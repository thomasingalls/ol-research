//! `survey journals` (citation fields plan §11, open item 3): whether what
//! NLM says of journals settles the pairs of records whose ISSNs do not
//! overlap, counted before the merge compares journals by an ISSN-L.
//!
//! The merge's rule (CF2's rule 4) is that two records disagree on the
//! journal when both give ISSNs and share none, and PubMed's side of that
//! already holds its `ISSNLinking`, NLM's copy of the ISSN-L. So a pair in
//! conflict is one whose Crossref record names neither PubMed's ISSN nor
//! its ISSN-L. This asks what else is known of such a pair:
//!
//! - NLM's table. Every PubMed record gives its journal's ISSN beside the
//!   journal's ISSN-L and NLM's own id for it; over the whole source that
//!   is a table from ISSN to ISSN-L, and to NLM id.
//! - Crossref's own pairings: the ISSNs one Crossref record names together.
//!
//! A pair is a PubMed record and the Crossref record of a DOI it names,
//! which is how nearly every pair of the merge is made; the merge's other
//! evidence is not read, so the counts are near the merge's and not its.
//! Every record is read at its speaking copy (merge plan §8). It writes
//! nothing but the list `--out` names: the ISSNs NLM gives no ISSN-L for,
//! the ones another table would be asked about.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt::{self, Write as _};
use std::fs::File;
use std::io::{self, BufWriter, Write as _};
use std::process::ExitCode;
use std::time::Instant;

use ingest::Store;
use merge::copies::Copies;
use merge::{Contributor, Name};
use model::{Interned, Issn, Pmid, Table};
use source_crossref::Crossref;
use source_crossref::normalize::Work as CrossrefWork;
use source_pubmed::Pubmed;
use source_pubmed::normalize::Citation;

use super::{each, read_copies};
use crate::{Options, Sets};

/// Journal pairs shown of each class, those of the most pairs first.
const SHOWN: usize = 15;
/// Examples shown of each oddity of NLM's table.
const ODD: usize = 5;
/// The list's first rows, counted by the unsettled pairs they cover.
const FIRST: [usize; 4] = [100, 1_000, 10_000, 100_000];

/// `survey journals [--store <root>] [--limit N] [--out <list.tsv>]`.
pub(crate) fn journals(opts: &Options) -> ExitCode {
    if !opts.paths.is_empty()
        || opts.workers.is_some()
        || opts.mailto.is_some()
        || opts.seed.is_some()
        || opts.by.is_some()
        || opts.survey
        || opts.related.is_some()
        || opts.walk.is_some()
        || opts.depth.is_some()
    {
        return crate::usage();
    }
    let what = "survey journals";
    let limit = opts.limit.map(|n| usize::try_from(n).unwrap_or(usize::MAX));
    // Before an hour's reading, not after it.
    let list = match &opts.out {
        Some(path) => match File::create(path) {
            Ok(file) => Some((path, BufWriter::new(file))),
            Err(e) => {
                eprintln!("{what}: {}: {e}", path.display());
                return ExitCode::FAILURE;
            }
        },
        None => None,
    };
    let (report, rows) = match survey(&opts.store(), limit) {
        Ok(found) => found,
        Err(e) => {
            eprintln!("{what}: {e}");
            return ExitCode::FAILURE;
        }
    };
    if let Some((path, mut file)) = list {
        let written = (|| -> io::Result<()> {
            writeln!(file, "issn\tunsettled pairs\tCrossref records")?;
            for row in &rows {
                writeln!(file, "{}\t{}\t{}", row.issn, row.pairs, row.records)?;
            }
            file.flush()
        })();
        if let Err(e) = written {
            eprintln!("{what}: {}: {e}", path.display());
            return ExitCode::FAILURE;
        }
    }
    let mut out = io::stdout().lock();
    match out.write_all(report.as_bytes()).and_then(|()| out.flush()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("stdout: {e}");
            ExitCode::FAILURE
        }
    }
}

fn survey(store: &Store, limit: Option<usize>) -> Result<(String, Vec<Row>), String> {
    let what = "survey journals";
    let started = Instant::now();
    let pubmed = Sets::find::<Pubmed>(store).map_err(|e| e.to_string())?;
    let crossref = Sets::find::<Crossref>(store).map_err(|e| e.to_string())?;
    let mut copies = Copies::new();
    read_copies::<Pubmed>(&mut copies, &pubmed, limit)?;
    read_copies::<Crossref>(&mut copies, &crossref, limit)?;
    copies.settle().map_err(|e| e.to_string())?;
    crate::phase(what, "copies read", started);
    let mut seen = Survey::default();
    for (path, set) in &pubmed.works {
        each::<Pubmed>(path, set, limit, &copies, |w, table| seen.pubmed(w, table))?;
    }
    crate::phase(
        what,
        &format!("PubMed read, {} DOIs named", seen.by_doi.len()),
        started,
    );
    for (path, set) in &crossref.works {
        each::<Crossref>(path, set, limit, &copies, |w, table| {
            seen.crossref(w, table)
        })?;
    }
    drop(copies);
    crate::phase(
        what,
        &format!("Crossref read, {} pairs", seen.counts.pairs),
        started,
    );
    let tables = Tables::of(&seen.journals, &seen.sets);
    let mut out = String::new();
    let mut rows = Vec::new();
    let written = (|| -> fmt::Result {
        writeln!(out, "survey journals: {}", store.root().display())?;
        if limit.is_some() {
            writeln!(out, "(--limit: each set read only in part)")?;
        }
        seen.report_pubmed(&tables, &mut out)?;
        seen.report_crossref(&tables, &mut out)?;
        rows = seen.report_pairs(&tables, &mut out)?;
        Ok(())
    })();
    written.map_err(|e| e.to_string())?;
    crate::phase(what, "counted", started);
    Ok((out, rows))
}

/// A journal as one PubMed record gives it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Journal {
    /// `NlmUniqueID`, by its place in [`Survey::nlm_ids`].
    nlm: Option<u32>,
    /// `Journal/ISSN`, where it is one.
    issn: Option<Issn>,
    /// `ISSNLinking`, where it is one.
    linking: Option<Issn>,
}

impl Journal {
    /// The ISSNs the merge compares by: both.
    fn has(&self, issn: Issn) -> bool {
        self.issn == Some(issn) || self.linking == Some(issn)
    }
}

#[derive(Debug)]
struct Entry {
    journal: Journal,
    /// `MedlineTA` of the first record that gave the journal so.
    title: Box<str>,
    records: u64,
}

/// The PubMed record that names a DOI.
#[derive(Debug, Clone, Copy)]
struct Claimant {
    pmid: Pmid,
    /// Its journal, by its place in [`Survey::journals`].
    journal: u32,
    /// Another PubMed record names the DOI too.
    shared: bool,
}

/// The pairs of one PubMed journal and one set of Crossref's ISSNs that
/// share no ISSN.
#[derive(Debug)]
struct Apart {
    pairs: u64,
    /// The first pair.
    pmid: Pmid,
    doi: Box<str>,
    /// Crossref's first `container-title` there.
    title: Option<Box<str>>,
}

#[derive(Debug, Default)]
struct Counts {
    pubmed: u64,
    /// PubMed records naming a DOI.
    pubmed_doi: u64,
    /// DOIs more than one PubMed record names.
    shared_dois: u64,
    crossref: u64,
    pairs: u64,
    /// Pairs on a DOI more than one PubMed record names, not compared.
    shared: u64,
    neither: u64,
    pubmed_only: u64,
    crossref_only: u64,
    same: u64,
    apart: u64,
}

#[derive(Debug, Default)]
struct Survey {
    nlm_ids: Vec<Box<str>>,
    by_nlm_id: HashMap<Box<str>, u32>,
    journals: Vec<Entry>,
    by_journal: HashMap<Journal, u32>,
    by_doi: HashMap<Box<str>, Claimant>,
    /// Each set of ISSNs a Crossref record names, sorted, and the records
    /// naming exactly it.
    sets: Vec<(Box<[Issn]>, u64)>,
    by_set: HashMap<Box<[Issn]>, u32>,
    /// By a journal's place and a set's.
    apart: HashMap<(u32, u32), Apart>,
    counts: Counts,
}

fn index(len: usize, of: &str) -> Result<u32, String> {
    u32::try_from(len).map_err(|_| format!("over 4 billion {of}"))
}

/// A value's text: its own, or its symbol's in the set's table.
fn text<'s>(
    value: &'s Interned<'_>,
    table: Option<&'s Table>,
    field: &str,
) -> Result<&'s str, String> {
    match (value.as_text(), table) {
        (Some(text), _) => Ok(text),
        (None, Some(table)) => value
            .resolve(table)
            .ok_or_else(|| format!("{field}: a symbol beyond the set's table")),
        (None, None) => Err(format!("{field}: a symbol, and the set has no table")),
    }
}

impl Survey {
    fn pubmed(&mut self, w: &Citation<'_>, table: Option<&Table>) -> Result<(), String> {
        self.counts.pubmed += 1;
        let nlm = match &w.journal.nlm_unique_id {
            Some(id) => {
                let id = text(id, table, "journal.nlm_unique_id")?;
                Some(if let Some(&ix) = self.by_nlm_id.get(id) {
                    ix
                } else {
                    let ix = index(self.nlm_ids.len(), "NLM ids")?;
                    self.nlm_ids.push(id.into());
                    self.by_nlm_id.insert(id.into(), ix);
                    ix
                })
            }
            None => None,
        };
        let journal = Journal {
            nlm,
            issn: w
                .journal
                .issn
                .as_ref()
                .and_then(|t| t.issn.valid().copied()),
            linking: w
                .journal
                .issn_linking
                .as_ref()
                .and_then(|c| c.valid().copied()),
        };
        let ix = if let Some(&ix) = self.by_journal.get(&journal) {
            ix
        } else {
            let ix = index(self.journals.len(), "journals")?;
            let title = text(&w.journal.medline_ta, table, "journal.medline_ta")?;
            self.journals.push(Entry {
                journal,
                title: title.into(),
                records: 0,
            });
            self.by_journal.insert(journal, ix);
            ix
        };
        self.journals[ix as usize].records += 1;
        let pmid = w.pmid;
        let (by_doi, shared) = (&mut self.by_doi, &mut self.counts.shared_dois);
        let mut named = false;
        Pubmed::names(w, &mut |a| {
            let Name::Doi(doi) = a.name else {
                return;
            };
            named = true;
            match by_doi.get_mut(doi) {
                None => {
                    let claimant = Claimant {
                        pmid,
                        journal: ix,
                        shared: false,
                    };
                    by_doi.insert(doi.into(), claimant);
                }
                Some(c) if c.pmid != pmid && !c.shared => {
                    c.shared = true;
                    *shared += 1;
                }
                Some(_) => {}
            }
        });
        self.counts.pubmed_doi += u64::from(named);
        Ok(())
    }

    fn crossref(&mut self, w: &CrossrefWork<'_>, table: Option<&Table>) -> Result<(), String> {
        self.counts.crossref += 1;
        // As the merge's view of the record gathers them: the valid ones.
        let mut issns: Vec<Issn> = w
            .issns
            .iter()
            .filter_map(|t| t.issn.valid().copied())
            .collect();
        issns.sort_unstable();
        issns.dedup();
        let set = if issns.is_empty() {
            None
        } else {
            Some(if let Some(&ix) = self.by_set.get(issns.as_slice()) {
                ix
            } else {
                let ix = index(self.sets.len(), "sets of ISSNs")?;
                self.sets.push((issns.as_slice().into(), 0));
                self.by_set.insert(issns.as_slice().into(), ix);
                ix
            })
        };
        if let Some(ix) = set {
            self.sets[ix as usize].1 += 1;
        }
        // The PubMed record naming this one, by any DOI it goes by.
        let by_doi = &self.by_doi;
        let mut found = None;
        Crossref::names(w, &mut |a| {
            if found.is_none()
                && let Name::Doi(doi) = a.name
            {
                found = by_doi.get(doi).copied();
            }
        });
        let Some(claimant) = found else {
            return Ok(());
        };
        self.counts.pairs += 1;
        if claimant.shared {
            self.counts.shared += 1;
            return Ok(());
        }
        let journal = self.journals[claimant.journal as usize].journal;
        let gives = journal.issn.is_some() || journal.linking.is_some();
        match (gives, set) {
            (false, None) => self.counts.neither += 1,
            (false, Some(_)) => self.counts.crossref_only += 1,
            (true, None) => self.counts.pubmed_only += 1,
            (true, Some(_)) if issns.iter().any(|&i| journal.has(i)) => self.counts.same += 1,
            (true, Some(set)) => {
                self.counts.apart += 1;
                if let Some(apart) = self.apart.get_mut(&(claimant.journal, set)) {
                    apart.pairs += 1;
                } else {
                    let title = match w.container_titles.iter().next() {
                        Some(title) => Some(text(title, table, "container_titles")?.into()),
                        None => None,
                    };
                    let apart = Apart {
                        pairs: 1,
                        pmid: claimant.pmid,
                        doi: w.doi.as_str().into(),
                        title,
                    };
                    self.apart.insert((claimant.journal, set), apart);
                }
            }
        }
        Ok(())
    }
}

/// What else is known of a pair whose records share no ISSN: the first of
/// these that holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Class {
    /// Another PubMed record gives one of Crossref's ISSNs beside this
    /// record's ISSN-L.
    Linking,
    /// Another PubMed record gives one of Crossref's ISSNs under this
    /// record's NLM id.
    Id,
    /// Another Crossref record names one of them beside an ISSN that is
    /// this journal's by NLM, and NLM has none of them.
    Beside,
    /// The same, but NLM has one of them as another journal's.
    BesideApart,
    /// NLM has one of Crossref's ISSNs, as another journal's.
    Other,
    /// No PubMed record names any of Crossref's ISSNs.
    Unknown,
}

impl Class {
    const fn name(self) -> &'static str {
        match self {
            Self::Linking => "NLM gives Crossref's ISSN this ISSN-L elsewhere",
            Self::Id => "NLM gives Crossref's ISSN this NLM id elsewhere",
            Self::Beside => "Crossref pairs them elsewhere; NLM has not Crossref's",
            Self::BesideApart => "Crossref pairs them elsewhere; NLM has another journal",
            Self::Other => "NLM has Crossref's ISSN as another journal's",
            Self::Unknown => "no PubMed record names Crossref's ISSN",
        }
    }

    /// NLM's own word settles it.
    const fn settled(self) -> bool {
        matches!(self, Self::Linking | Self::Id)
    }
}

/// What the two sources say of ISSNs over all their records.
#[derive(Debug, Default)]
struct Tables {
    /// NLM's table: an ISSN PubMed gives, to each ISSN-L given beside it;
    /// an ISSN-L to itself.
    linking: HashMap<Issn, Vec<Issn>>,
    /// An ISSN PubMed gives, as ISSN or ISSN-L, to each NLM id given
    /// beside it: empty where none ever is.
    ids: HashMap<Issn, Vec<u32>>,
    /// An ISSN to each other one a Crossref record names with it.
    beside: HashMap<Issn, Vec<Issn>>,
    /// An ISSN to the Crossref records naming it.
    records: HashMap<Issn, u64>,
}

fn add<T: PartialEq>(to: &mut Vec<T>, value: T) {
    if !to.contains(&value) {
        to.push(value);
    }
}

impl Tables {
    fn of(journals: &[Entry], sets: &[(Box<[Issn]>, u64)]) -> Self {
        let mut tables = Self::default();
        for entry in journals {
            let j = &entry.journal;
            if let Some(l) = j.linking {
                add(tables.linking.entry(l).or_default(), l);
                if let Some(i) = j.issn {
                    add(tables.linking.entry(i).or_default(), l);
                }
            }
            for issn in [j.issn, j.linking].into_iter().flatten() {
                let ids = tables.ids.entry(issn).or_default();
                if let Some(id) = j.nlm {
                    add(ids, id);
                }
            }
        }
        for (set, records) in sets {
            for &a in set {
                *tables.records.entry(a).or_default() += records;
                for &b in set {
                    if a != b {
                        add(tables.beside.entry(a).or_default(), b);
                    }
                }
            }
        }
        tables
    }

    /// NLM gives `issn` an ISSN-L.
    fn linked(&self, issn: Issn) -> bool {
        self.linking.contains_key(&issn)
    }

    /// Some PubMed record names `issn`.
    fn named(&self, issn: Issn) -> bool {
        self.ids.contains_key(&issn)
    }

    /// `issn` is the journal's by NLM's ISSN-L.
    fn by_linking(&self, journal: &Journal, issn: Issn) -> bool {
        journal
            .linking
            .is_some_and(|l| self.linking.get(&issn).is_some_and(|ls| ls.contains(&l)))
    }

    /// `issn` is the journal's by NLM's id.
    fn by_id(&self, journal: &Journal, issn: Issn) -> bool {
        journal
            .nlm
            .is_some_and(|id| self.ids.get(&issn).is_some_and(|ids| ids.contains(&id)))
    }

    fn class(&self, journal: &Journal, theirs: &[Issn]) -> Class {
        if theirs.iter().any(|&c| self.by_linking(journal, c)) {
            return Class::Linking;
        }
        if theirs.iter().any(|&c| self.by_id(journal, c)) {
            return Class::Id;
        }
        let ours =
            |y: Issn| journal.has(y) || self.by_linking(journal, y) || self.by_id(journal, y);
        let paired = theirs.iter().any(|c| {
            self.beside
                .get(c)
                .is_some_and(|with| with.iter().any(|&y| ours(y)))
        });
        let named = theirs.iter().any(|&c| self.named(c));
        match (paired, named) {
            (true, false) => Class::Beside,
            (true, true) => Class::BesideApart,
            (false, true) => Class::Other,
            (false, false) => Class::Unknown,
        }
    }
}

/// A row of the list: an ISSN NLM gives no ISSN-L for.
#[derive(Debug, PartialEq, Eq)]
struct Row {
    issn: Issn,
    /// The pairs NLM does not settle that name it, on either side.
    pairs: u64,
    /// The Crossref records naming it.
    records: u64,
}

/// `n` of `of` as a percentage to one place.
fn percent(n: u64, of: u64) -> String {
    if of == 0 {
        return "-".to_owned();
    }
    let tenths = (u128::from(n) * 1000 + u128::from(of) / 2) / u128::from(of);
    format!("{}.{}%", tenths / 10, tenths % 10)
}

/// One oddity of NLM's table: how many, and the first few.
fn odd(out: &mut String, what: &str, count: usize, shown: &[String]) -> fmt::Result {
    if shown.is_empty() {
        writeln!(out, "    {what}: {count}")
    } else {
        writeln!(out, "    {what}: {count}, as {}", shown.join("; "))
    }
}

fn issns(set: &[Issn]) -> String {
    set.iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(" ")
}

impl Survey {
    fn nlm_id(&self, ix: u32) -> &str {
        &self.nlm_ids[ix as usize]
    }

    fn report_pubmed(&self, tables: &Tables, out: &mut String) -> fmt::Result {
        let c = &self.counts;
        let sum = |f: &dyn Fn(&Journal) -> bool| -> u64 {
            self.journals
                .iter()
                .filter(|e| f(&e.journal))
                .map(|e| e.records)
                .sum()
        };
        writeln!(out)?;
        writeln!(out, "PubMed: {} records", c.pubmed)?;
        writeln!(
            out,
            "  naming a DOI {}; DOIs more than one record names {}",
            c.pubmed_doi, c.shared_dois
        )?;
        writeln!(
            out,
            "  giving an ISSN {}; an ISSN-L {}; an ISSN and no ISSN-L {}; neither {}",
            sum(&|j| j.issn.is_some()),
            sum(&|j| j.linking.is_some()),
            sum(&|j| j.issn.is_some() && j.linking.is_none()),
            sum(&|j| j.issn.is_none() && j.linking.is_none())
        )?;
        writeln!(
            out,
            "  NLM ids {}; journals as a record gives one (NLM id, ISSN, ISSN-L) {}",
            self.nlm_ids.len(),
            self.journals.len()
        )?;
        let ls: BTreeSet<Issn> = self
            .journals
            .iter()
            .filter_map(|e| e.journal.linking)
            .collect();
        writeln!(
            out,
            "  NLM's table: {} ISSNs under {} ISSN-Ls; ISSNs given with no ISSN-L anywhere {}",
            tables.linking.len(),
            ls.len(),
            tables.ids.keys().filter(|&&i| !tables.linked(i)).count()
        )?;
        // Where the table is not one: an ISSN under two ISSN-Ls.
        let mut two: Vec<(Issn, &Vec<Issn>)> = tables
            .linking
            .iter()
            .filter(|(_, ls)| ls.len() > 1)
            .map(|(&i, ls)| (i, ls))
            .collect();
        two.sort_unstable_by_key(|&(i, _)| i);
        let shown: Vec<String> = two
            .iter()
            .take(ODD)
            .map(|(i, ls)| format!("{i} under {}", issns(ls)))
            .collect();
        odd(out, "an ISSN under more than one ISSN-L", two.len(), &shown)?;
        let mut by_id: BTreeMap<u32, BTreeSet<Issn>> = BTreeMap::new();
        let mut by_l: BTreeMap<Issn, BTreeSet<u32>> = BTreeMap::new();
        for e in &self.journals {
            if let (Some(id), Some(l)) = (e.journal.nlm, e.journal.linking) {
                by_id.entry(id).or_default().insert(l);
                by_l.entry(l).or_default().insert(id);
            }
        }
        let shown: Vec<String> = by_id
            .iter()
            .filter(|(_, ls)| ls.len() > 1)
            .take(ODD)
            .map(|(&id, ls)| {
                let ls: Vec<Issn> = ls.iter().copied().collect();
                format!("{} with {}", self.nlm_id(id), issns(&ls))
            })
            .collect();
        let count = by_id.values().filter(|ls| ls.len() > 1).count();
        odd(out, "an NLM id with more than one ISSN-L", count, &shown)?;
        let shown: Vec<String> = by_l
            .iter()
            .filter(|(_, ids)| ids.len() > 1)
            .take(ODD)
            .map(|(l, ids)| {
                let ids: Vec<&str> = ids.iter().map(|&id| self.nlm_id(id)).collect();
                format!("{l} under {}", ids.join(" "))
            })
            .collect();
        let count = by_l.values().filter(|ids| ids.len() > 1).count();
        odd(out, "an ISSN-L under more than one NLM id", count, &shown)
    }

    fn report_crossref(&self, tables: &Tables, out: &mut String) -> fmt::Result {
        let with: u64 = self.sets.iter().map(|s| s.1).sum();
        let records = |f: &dyn Fn(Issn) -> bool| -> u64 {
            self.sets
                .iter()
                .filter(|(set, _)| set.iter().any(|&i| f(i)))
                .map(|s| s.1)
                .sum()
        };
        let count = |f: &dyn Fn(Issn) -> bool| tables.records.keys().filter(|&&i| f(i)).count();
        // An ISSN no PubMed record names, beside one that one does.
        let beside = |i: Issn| {
            !tables.named(i)
                && tables
                    .beside
                    .get(&i)
                    .is_some_and(|with| with.iter().any(|&y| tables.named(y)))
        };
        writeln!(out)?;
        writeln!(
            out,
            "Crossref: {} records; giving an ISSN {with}; distinct sets of ISSNs {}; distinct ISSNs {}",
            self.counts.crossref,
            self.sets.len(),
            tables.records.len()
        )?;
        writeln!(
            out,
            "  ISSNs NLM gives an ISSN-L: {}; records naming one {} ({} of those giving an ISSN)",
            count(&|i| tables.linked(i)),
            records(&|i| tables.linked(i)),
            percent(records(&|i| tables.linked(i)), with)
        )?;
        writeln!(
            out,
            "  ISSNs PubMed names with no ISSN-L: {}",
            count(&|i| tables.named(i) && !tables.linked(i))
        )?;
        writeln!(
            out,
            "  ISSNs no PubMed record names: {}; of them, beside one PubMed names, in some record: {}",
            count(&|i| !tables.named(i)),
            count(&beside)
        )?;
        let none = with - records(&|i| tables.named(i));
        writeln!(
            out,
            "  records naming only ISSNs no PubMed record names: {none} ({})",
            percent(none, with)
        )
    }

    fn pair(&self, journal: u32, set: u32, apart: &Apart) -> String {
        let e = &self.journals[journal as usize];
        let nlm = e.journal.nlm.map_or("none", |id| self.nlm_id(id));
        let own = |i: Option<Issn>| i.map_or_else(|| "none".to_owned(), |i| i.to_string());
        format!(
            "{:>9}  {:?} NLM id {nlm}, ISSN {}, ISSN-L {} / {} {:?}  pmid:{} doi:{}",
            apart.pairs,
            e.title,
            own(e.journal.issn),
            own(e.journal.linking),
            issns(&self.sets[set as usize].0),
            apart.title.as_deref().unwrap_or(""),
            apart.pmid,
            apart.doi
        )
    }

    /// The pairs, and the list: every ISSN of Crossref's or of an unsettled
    /// pair that NLM gives no ISSN-L, those in the most pairs first.
    fn report_pairs(&self, tables: &Tables, out: &mut String) -> Result<Vec<Row>, fmt::Error> {
        let c = &self.counts;
        let both = c.same + c.apart;
        writeln!(out)?;
        writeln!(
            out,
            "Pairs, a PubMed record and the Crossref record of a DOI it names: {}",
            c.pairs
        )?;
        writeln!(
            out,
            "  the DOI named by more than one PubMed record, not compared: {}",
            c.shared
        )?;
        writeln!(out, "  neither gives an ISSN: {}", c.neither)?;
        writeln!(out, "  only PubMed gives one: {}", c.pubmed_only)?;
        writeln!(out, "  only Crossref gives one: {}", c.crossref_only)?;
        writeln!(out, "  both give ISSNs: {both}")?;
        writeln!(
            out,
            "    sharing one: {} ({})",
            c.same,
            percent(c.same, both)
        )?;
        writeln!(
            out,
            "    sharing none, the merge's conflict on the journal: {} ({}), over {} pairs of a journal and a set of ISSNs",
            c.apart,
            percent(c.apart, both),
            self.apart.len()
        )?;
        let mut classes: BTreeMap<Class, Vec<(u32, u32, &Apart)>> = BTreeMap::new();
        let mut unsettled: HashMap<Issn, u64> = HashMap::new();
        let mut asked: Vec<(u64, Vec<Issn>)> = Vec::new();
        for (&(journal, set), apart) in &self.apart {
            let j = &self.journals[journal as usize].journal;
            let theirs = &self.sets[set as usize].0;
            let class = tables.class(j, theirs);
            classes
                .entry(class)
                .or_default()
                .push((journal, set, apart));
            if !class.settled() {
                let ours = j.issn.filter(|_| j.linking.is_none());
                let unlinked: Vec<Issn> = theirs
                    .iter()
                    .copied()
                    .chain(ours)
                    .filter(|&issn| !tables.linked(issn))
                    .collect();
                for &issn in &unlinked {
                    *unsettled.entry(issn).or_default() += apart.pairs;
                }
                asked.push((apart.pairs, unlinked));
            }
        }
        for (class, found) in &mut classes {
            found.sort_unstable_by(|a, b| {
                b.2.pairs
                    .cmp(&a.2.pairs)
                    .then_with(|| (a.0, a.1).cmp(&(b.0, b.1)))
            });
            let pairs: u64 = found.iter().map(|f| f.2.pairs).sum();
            writeln!(
                out,
                "      {:<56} {pairs:>9} ({}), {} journal pairs",
                class.name(),
                percent(pairs, c.apart),
                found.len()
            )?;
        }
        let settled: u64 = classes
            .iter()
            .filter(|(class, _)| class.settled())
            .flat_map(|(_, found)| found.iter().map(|f| f.2.pairs))
            .sum();
        writeln!(
            out,
            "    settled by NLM's own word: {settled} ({}); not: {} ({})",
            percent(settled, c.apart),
            c.apart - settled,
            percent(c.apart - settled, c.apart)
        )?;
        writeln!(
            out,
            "  each class's journal pairs of the most pairs; PubMed's journal / Crossref's ISSNs and title, and the first pair:"
        )?;
        for (class, found) in &classes {
            writeln!(out, "    {}:", class.name())?;
            for &(journal, set, apart) in found.iter().take(SHOWN) {
                writeln!(out, "      {}", self.pair(journal, set, apart))?;
            }
        }
        let rows = Self::rows(tables, &unsettled);
        report_list(&rows, &asked, c.apart - settled, out)?;
        Ok(rows)
    }

    fn rows(tables: &Tables, unsettled: &HashMap<Issn, u64>) -> Vec<Row> {
        let mut rows: Vec<Row> = tables
            .records
            .iter()
            .filter(|&(&issn, _)| !tables.linked(issn))
            .map(|(&issn, &records)| Row {
                issn,
                pairs: unsettled.get(&issn).copied().unwrap_or(0),
                records,
            })
            .collect();
        // PubMed's own, given with no ISSN-L, that no Crossref record names.
        rows.extend(
            unsettled
                .iter()
                .filter(|(issn, _)| !tables.records.contains_key(issn))
                .map(|(&issn, &pairs)| Row {
                    issn,
                    pairs,
                    records: 0,
                }),
        );
        rows.sort_unstable_by(|a, b| {
            b.pairs
                .cmp(&a.pairs)
                .then_with(|| b.records.cmp(&a.records))
                .then_with(|| a.issn.cmp(&b.issn))
        });
        rows
    }
}

/// How far down the list the unsettled pairs' ISSNs reach: the pairs
/// naming none of the list, and, for each of [`FIRST`], the pairs naming
/// some of it and all of those within that many rows.
fn reach(rows: &[Row], asked: &[(u64, Vec<Issn>)]) -> (u64, [u64; FIRST.len()]) {
    let rank: HashMap<Issn, usize> = rows
        .iter()
        .enumerate()
        .map(|(at, row)| (row.issn, at))
        .collect();
    let mut none = 0;
    let mut within = [0; FIRST.len()];
    for (pairs, issns) in asked {
        let last = issns
            .iter()
            .map(|issn| rank.get(issn).copied().unwrap_or(usize::MAX))
            .max();
        let Some(last) = last else {
            none += pairs;
            continue;
        };
        for (n, &first) in FIRST.iter().enumerate() {
            if last < first {
                within[n] += pairs;
            }
        }
    }
    (none, within)
}

fn report_list(
    rows: &[Row],
    asked: &[(u64, Vec<Issn>)],
    unsettled: u64,
    out: &mut String,
) -> fmt::Result {
    let in_pairs = rows.iter().filter(|r| r.pairs > 0).count();
    let (none, within) = reach(rows, asked);
    writeln!(out)?;
    writeln!(
        out,
        "ISSNs NLM gives no ISSN-L (the list): {} in all of Crossref's records and the unsettled pairs; {in_pairs} of them in an unsettled pair",
        rows.len()
    )?;
    writeln!(
        out,
        "  unsettled pairs naming none of them, NLM giving each side an ISSN-L: {none} ({})",
        percent(none, unsettled)
    )?;
    writeln!(
        out,
        "  unsettled pairs naming some, all of them within the list's first rows:"
    )?;
    for (first, pairs) in FIRST.into_iter().zip(within) {
        writeln!(
            out,
            "    the first {first:>7}: {pairs:>9} ({})",
            percent(pairs, unsettled)
        )?;
        if first >= in_pairs {
            break;
        }
    }
    writeln!(
        out,
        "  the first {SHOWN}, with the Crossref records naming each:"
    )?;
    for row in rows.iter().take(SHOWN) {
        writeln!(
            out,
            "    {}  {:>9} pairs  {:>9} records",
            row.issn, row.pairs, row.records
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn issn(s: &str) -> Issn {
        Issn::parse(s).unwrap()
    }

    fn entry(nlm: Option<u32>, issn_: Option<&str>, linking: Option<&str>) -> Entry {
        Entry {
            journal: Journal {
                nlm,
                issn: issn_.map(issn),
                linking: linking.map(issn),
            },
            title: "".into(),
            records: 1,
        }
    }

    fn set(issns: &[&str]) -> (Box<[Issn]>, u64) {
        let mut set: Vec<Issn> = issns.iter().map(|s| issn(s)).collect();
        set.sort_unstable();
        (set.into(), 1)
    }

    // Nature: 0028-0836 in print and as ISSN-L, 1476-4687 online. Science:
    // 0036-8075 and 1095-9203. The Lancet: 0140-6736 and 1474-547X. JAMA:
    // 0098-7484 and 1538-3598.

    #[test]
    fn another_record_gives_the_issn_this_issn_l() {
        let journals = [
            entry(Some(0), Some("0028-0836"), Some("0028-0836")),
            entry(Some(0), Some("1476-4687"), Some("0028-0836")),
        ];
        let sets = [set(&["1476-4687"])];
        let tables = Tables::of(&journals, &sets);
        assert_eq!(
            tables.class(&journals[0].journal, &sets[0].0),
            Class::Linking
        );
    }

    #[test]
    fn another_record_gives_the_issn_this_nlm_id() {
        let journals = [
            entry(Some(7), Some("0098-7484"), None),
            entry(Some(7), Some("1538-3598"), None),
        ];
        let sets = [set(&["1538-3598"])];
        let tables = Tables::of(&journals, &sets);
        assert_eq!(tables.class(&journals[0].journal, &sets[0].0), Class::Id);
    }

    #[test]
    fn crossref_pairs_them_on_another_record() {
        let journals = [entry(Some(1), Some("0036-8075"), Some("0036-8075"))];
        let sets = [set(&["1095-9203"]), set(&["0036-8075", "1095-9203"])];
        let tables = Tables::of(&journals, &sets);
        assert_eq!(
            tables.class(&journals[0].journal, &sets[0].0),
            Class::Beside
        );
    }

    #[test]
    fn crossref_pairs_them_through_an_issn_nlm_links() {
        // Crossref never writes Nature's online ISSN beside its print one,
        // but beside a third that NLM gives Nature's ISSN-L.
        let journals = [
            entry(Some(0), Some("0028-0836"), Some("0028-0836")),
            entry(Some(0), Some("1476-4687"), Some("0028-0836")),
        ];
        let sets = [set(&["2045-2322"]), set(&["1476-4687", "2045-2322"])];
        let tables = Tables::of(&journals, &sets);
        assert_eq!(
            tables.class(&journals[0].journal, &sets[0].0),
            Class::Beside
        );
    }

    #[test]
    fn crossref_pairs_them_and_nlm_has_another_journal() {
        let journals = [
            entry(Some(2), Some("0140-6736"), Some("0140-6736")),
            entry(Some(0), Some("1476-4687"), Some("0028-0836")),
        ];
        let sets = [set(&["1476-4687"]), set(&["0140-6736", "1476-4687"])];
        let tables = Tables::of(&journals, &sets);
        assert_eq!(
            tables.class(&journals[0].journal, &sets[0].0),
            Class::BesideApart
        );
    }

    #[test]
    fn nlm_has_the_issn_as_another_journal() {
        let journals = [
            entry(Some(2), Some("0140-6736"), Some("0140-6736")),
            entry(Some(0), Some("1476-4687"), Some("0028-0836")),
        ];
        let sets = [set(&["1476-4687"])];
        let tables = Tables::of(&journals, &sets);
        assert_eq!(tables.class(&journals[0].journal, &sets[0].0), Class::Other);
        // Named by PubMed with no ISSN-L and no id is still named.
        let journals = [
            entry(Some(2), Some("0140-6736"), Some("0140-6736")),
            entry(None, Some("1476-4687"), None),
        ];
        let tables = Tables::of(&journals, &sets);
        assert_eq!(tables.class(&journals[0].journal, &sets[0].0), Class::Other);
    }

    #[test]
    fn no_pubmed_record_names_the_issn() {
        let journals = [entry(Some(2), Some("0140-6736"), Some("0140-6736"))];
        let sets = [set(&["2045-2322"]), set(&["1474-547X", "2045-2322"])];
        let tables = Tables::of(&journals, &sets);
        assert_eq!(
            tables.class(&journals[0].journal, &sets[0].0),
            Class::Unknown
        );
    }

    #[test]
    fn the_list_is_what_nlm_gives_no_issn_l() {
        let journals = [
            entry(Some(0), Some("0028-0836"), Some("0028-0836")),
            entry(Some(3), Some("0098-7484"), None),
        ];
        let sets = [
            set(&["0028-0836", "1476-4687"]),
            set(&["2045-2322"]),
            (set(&["1474-547X"]).0, 5),
        ];
        let tables = Tables::of(&journals, &sets);
        let unsettled = HashMap::from([(issn("2045-2322"), 4), (issn("0098-7484"), 4)]);
        let rows = Survey::rows(&tables, &unsettled);
        let row = |s: &str, pairs, records| Row {
            issn: issn(s),
            pairs,
            records,
        };
        // Nature's print ISSN has its ISSN-L and is not listed; JAMA's,
        // which PubMed gives none and Crossref never names, is.
        assert_eq!(
            rows,
            [
                row("2045-2322", 4, 1),
                row("0098-7484", 4, 0),
                row("1474-547X", 0, 5),
                row("1476-4687", 0, 1),
            ]
        );
    }

    #[test]
    fn the_reach_of_the_unsettled_pairs_down_the_list() {
        let row = |s: &str, pairs| Row {
            issn: issn(s),
            pairs,
            records: 0,
        };
        let rows: Vec<Row> = std::iter::once(row("2045-2322", 9))
            .chain((0..FIRST[0] - 1).map(|_| row("0000-0000", 0)))
            .chain([row("1474-547X", 2)])
            .collect();
        let asked = [
            // Both sides have their ISSN-L: nothing to ask.
            (5, vec![]),
            (7, vec![issn("2045-2322")]),
            // The second of its two is the list's row 101.
            (2, vec![issn("2045-2322"), issn("1474-547X")]),
        ];
        assert_eq!(reach(&rows, &asked), (5, [7, 9, 9, 9]));
    }

    #[test]
    fn percentages() {
        assert_eq!(percent(1, 3), "33.3%");
        assert_eq!(percent(2, 3), "66.7%");
        assert_eq!(percent(0, 0), "-");
        assert_eq!(percent(993_260, 30_662_791), "3.2%");
    }
}
