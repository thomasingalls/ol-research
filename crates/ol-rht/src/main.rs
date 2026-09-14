//! Command line for the pipeline.
//!
//! * `id <kind>` runs one identifier parser over stdin, one value per line.
//! * `pubmed check <file.xml.gz> [--limit N]` parses a baseline or update
//!   file with the strict PubMed parser and reports counts and throughput.
//! * `pubmed normalize <file.xml.gz> [--limit N]` parses and normalizes,
//!   and prints the first stats report: what every field held.
//! * `crossref check <file>...` parses API envelope pages (`.json`) or
//!   public-data-file lines (`.jsonl`, `.jsonl.gz`) with the strict Crossref
//!   types and reports counts and throughput.
//! * `crossref normalize <file>...` parses and normalizes, and prints the
//!   stats report.
//! * `openalex check <part.gz>...` parses snapshot part files with the strict
//!   OpenAlex types and reports counts and throughput.
//! * `openalex normalize <part.gz>...` parses and normalizes, and prints the
//!   stats report.
//!
//! All exit non-zero on the first failure and say exactly where it was: the
//! seed of the "reproduce against one record" command the failure report
//! promises (plan §6), and the quickest way to let a real file vote.

use std::collections::BTreeMap;
use std::fmt::Display;
use std::io::{self, BufRead};
use std::path::Path;
use std::process::ExitCode;
use std::time::Instant;

use model::{Claim, Doi, OpenAlexId, ParseError, Pmid};
use source_crossref::normalize::Emit as CrossrefEmit;
use source_openalex::normalize::{Emit as OpenAlexEmit, LocationId};
use source_pubmed::ArticleSet;
use source_pubmed::normalize::{Citation, Emit, Reference, normalize};
use source_pubmed::record::Record;

const USAGE: &str = "usage:
  ol-rht id <pmid|pmid-url|openalex|openalex-url> < values.txt
  ol-rht pubmed check <file.xml.gz> [--limit N]
  ol-rht pubmed normalize <file.xml.gz> [--limit N]
  ol-rht crossref check <file>...
  ol-rht crossref normalize <file>...
  ol-rht openalex check <part.gz>...
  ol-rht openalex normalize <part.gz>...";

type Parser = fn(&str) -> Result<(), ParseError>;

fn parser(kind: &str) -> Option<Parser> {
    Some(match kind {
        "pmid" => |s| Pmid::from_digits(s).map(drop),
        "pmid-url" => |s| Pmid::from_url(s).map(drop),
        "openalex" => |s| OpenAlexId::from_bare(s).map(drop),
        "openalex-url" => |s| OpenAlexId::from_url(s).map(drop),
        _ => return None,
    })
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    match args.as_slice() {
        ["id", kind] => match parser(kind) {
            Some(parse) => ids(parse),
            None => usage(),
        },
        ["pubmed", "check", path] => pubmed_check(Path::new(path), None),
        ["pubmed", "check", path, "--limit", n] => match n.parse() {
            Ok(limit) => pubmed_check(Path::new(path), Some(limit)),
            Err(_) => usage(),
        },
        ["pubmed", "normalize", path] => pubmed_normalize(Path::new(path), None),
        ["pubmed", "normalize", path, "--limit", n] => match n.parse() {
            Ok(limit) => pubmed_normalize(Path::new(path), Some(limit)),
            Err(_) => usage(),
        },
        ["crossref", "check", paths @ ..] if !paths.is_empty() => crossref_check(paths),
        ["crossref", "normalize", paths @ ..] if !paths.is_empty() => crossref_normalize(paths),
        ["openalex", "check", paths @ ..] if !paths.is_empty() => openalex_check(paths),
        ["openalex", "normalize", paths @ ..] if !paths.is_empty() => openalex_normalize(paths),
        _ => usage(),
    }
}

fn usage() -> ExitCode {
    eprintln!("{USAGE}");
    ExitCode::from(2)
}

fn ids(parse: Parser) -> ExitCode {
    let (mut ok, mut failed) = (0u64, 0u64);
    for (index, line) in io::stdin().lock().lines().enumerate() {
        let line = match line {
            Ok(line) => line,
            Err(err) => {
                eprintln!("stdin: {err}");
                return ExitCode::from(2);
            }
        };
        match parse(&line) {
            Ok(()) => ok += 1,
            Err(err) => {
                failed += 1;
                eprintln!("line {}: {line:?}: {err}", index + 1);
            }
        }
    }
    eprintln!("ok={ok} failed={failed}");
    if failed == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn read(path: &Path) -> Option<String> {
    match source_pubmed::gz::read_to_string(path) {
        Ok(input) => Some(input),
        Err(err) => {
            eprintln!("{}: {err}", path.display());
            None
        }
    }
}

#[allow(clippy::cast_precision_loss)] // reporting only
fn pubmed_check(path: &Path, limit: Option<u64>) -> ExitCode {
    let started = Instant::now();
    let Some(input) = read(path) else {
        return ExitCode::from(2);
    };
    let inflated = started.elapsed();
    let bytes = input.len();

    let parsing = Instant::now();
    let mut set = ArticleSet::new(&input);
    let (mut articles, mut deletes, mut references, mut article_ids) = (0u64, 0u64, 0u64, 0u64);
    loop {
        if limit.is_some_and(|n| articles + deletes >= n) {
            break;
        }
        match set.next_record() {
            Ok(Some(Record::Article(a))) => {
                articles += 1;
                if let Some(d) = &a.pubmed_data {
                    article_ids += d.article_ids.len() as u64;
                    references += count_references(&d.reference_lists);
                }
            }
            Ok(Some(Record::DeleteCitation(d))) => deletes += d.pmids.len() as u64,
            Ok(None) => break,
            Err(err) => {
                eprintln!("{}: {err}", path.display());
                eprintln!("after {articles} articles");
                return ExitCode::FAILURE;
            }
        }
    }
    let parsed = parsing.elapsed();
    let mib = bytes as f64 / (1024.0 * 1024.0);
    eprintln!(
        "{}: {articles} articles, {deletes} deletions, {article_ids} article ids, {references} references",
        path.display()
    );
    eprintln!(
        "inflate {:.2}s ({mib:.1} MiB); parse {:.2}s = {:.0} records/s, {:.0} MiB/s",
        inflated.as_secs_f64(),
        parsed.as_secs_f64(),
        (articles + deletes) as f64 / parsed.as_secs_f64(),
        mib / parsed.as_secs_f64()
    );
    ExitCode::SUCCESS
}

fn count_references(lists: &[source_pubmed::record::ReferenceList<'_>]) -> u64 {
    lists
        .iter()
        .map(|l| l.references.len() as u64 + count_references(&l.sub_lists))
        .sum()
}

/// The PubMed stats report (plan §6): what every normalized field held.
#[derive(Default)]
struct Stats {
    citations: u64,
    doi: [u64; 3],
    elocation_doi: [u64; 3],
    doi_lowercased: u64,
    pii: u64,
    pmc: u64,
    published: BTreeMap<&'static str, u64>,
    electronic: u64,
    revised: u64,
    completed: u64,
    status: BTreeMap<&'static str, u64>,
    languages: BTreeMap<&'static str, u64>,
    publication_types: BTreeMap<String, u64>,
    title: [u64; 3],
    abstracts: u64,
    sections: u64,
    sections_markup: u64,
    sections_math: u64,
    other_abstracts: u64,
    journal_issn: u64,
    journal_issn_linking: u64,
    persons: u64,
    collectives: u64,
    orcid: [u64; 3],
    affiliations: u64,
    affiliation_ids: BTreeMap<&'static str, u64>,
    investigators: u64,
    mesh_headings: u64,
    mesh_major: u64,
    chemicals: u64,
    keywords: u64,
    grants: u64,
    grants_with_country: u64,
    data_banks: u64,
    history: BTreeMap<&'static str, u64>,
    publication_status: BTreeMap<&'static str, u64>,
    relations: BTreeMap<&'static str, u64>,
    references: u64,
    reference_ids: BTreeMap<&'static str, u64>,
    reference_doi_malformed: u64,
    malformed_samples: Vec<String>,
}

impl Stats {
    /// Counts one DOI claim as valid / malformed / absent.
    fn doi_claim<E: Display>(&mut self, slot: usize, claim: Option<&Claim<'_, Doi<'_>, E>>) {
        let counters = if slot == 0 {
            &mut self.doi
        } else {
            &mut self.elocation_doi
        };
        match claim {
            None => counters[2] += 1,
            Some(Claim::Valid(doi)) => {
                counters[0] += 1;
                if !doi.is_borrowed() {
                    self.doi_lowercased += 1;
                }
            }
            Some(Claim::Malformed { raw, error }) => {
                counters[1] += 1;
                if self.malformed_samples.len() < 8 {
                    self.malformed_samples.push(format!("{raw:?}: {error}"));
                }
            }
        }
    }

    fn report(&self) {
        eprintln!("citations {}", self.citations);
        eprintln!(
            "  doi           valid {} malformed {} none {}",
            self.doi[0], self.doi[1], self.doi[2]
        );
        eprintln!(
            "  elocation doi valid {} malformed {} none {}",
            self.elocation_doi[0], self.elocation_doi[1], self.elocation_doi[2]
        );
        eprintln!("  doi lowercased (allocations) {}", self.doi_lowercased);
        eprintln!("  pii {}  pmc {}", self.pii, self.pmc);
        eprintln!("  published {:?}", self.published);
        eprintln!(
            "  electronic {}  revised {}  completed {}",
            self.electronic, self.revised, self.completed
        );
        eprintln!("  status {:?}", self.status);
        eprintln!("  languages {:?}", self.languages);
        eprintln!(
            "  publication types: {} distinct",
            self.publication_types.len()
        );
        eprintln!(
            "  title plain {} markup {} empty {}",
            self.title[0], self.title[1], self.title[2]
        );
        eprintln!(
            "  abstracts {} sections {} (markup {} math {})  other abstracts {}",
            self.abstracts,
            self.sections,
            self.sections_markup,
            self.sections_math,
            self.other_abstracts
        );
        eprintln!(
            "  journal issn {}  issn linking {}",
            self.journal_issn, self.journal_issn_linking
        );
        eprintln!(
            "  authors persons {} collectives {}  orcid valid {} malformed {} none {}",
            self.persons, self.collectives, self.orcid[0], self.orcid[1], self.orcid[2]
        );
        eprintln!(
            "  affiliations {}  ids {:?}  investigators {}",
            self.affiliations, self.affiliation_ids, self.investigators
        );
        eprintln!(
            "  mesh headings {} (major {})  chemicals {}  keywords {}  grants {} (with country {})  data banks {}",
            self.mesh_headings,
            self.mesh_major,
            self.chemicals,
            self.keywords,
            self.grants,
            self.grants_with_country,
            self.data_banks
        );
        eprintln!("  history {:?}", self.history);
        eprintln!("  publication status {:?}", self.publication_status);
        eprintln!("  relations {:?}", self.relations);
        eprintln!("references {}", self.references);
        eprintln!("  ids {:?}", self.reference_ids);
        eprintln!("  doi malformed {}", self.reference_doi_malformed);
        if !self.malformed_samples.is_empty() {
            eprintln!("  malformed samples:");
            for sample in &self.malformed_samples {
                eprintln!("    {sample}");
            }
        }
    }
}

impl<'n> Emit<'n> for Stats {
    fn citation(&mut self, c: &Citation<'n>) {
        use source_pubmed::normalize::AuthorName;
        self.citations += 1;
        self.doi_claim(0, c.ids.doi.as_ref());
        self.doi_claim(1, c.ids.elocation_doi.as_ref());
        self.pii += u64::from(c.ids.pii.is_some());
        self.pmc += u64::from(c.ids.pmc.is_some());
        *self
            .published
            .entry(c.dates.published.precision())
            .or_default() += 1;
        self.electronic += u64::from(c.dates.electronic.is_some());
        self.revised += u64::from(c.dates.revised.is_some());
        self.completed += u64::from(c.dates.completed.is_some());
        *self.status.entry(c.status.token()).or_default() += 1;
        for l in &c.languages {
            *self.languages.entry(l.alpha_3()).or_default() += 1;
        }
        for pt in &c.publication_types {
            *self
                .publication_types
                .entry(format!("{} {}", pt.ui, pt.name))
                .or_default() += 1;
        }
        if c.title.is_empty() {
            self.title[2] += 1;
        } else if c.title.contains('<') {
            self.title[1] += 1;
        } else {
            self.title[0] += 1;
        }
        if let Some(a) = &c.r#abstract {
            self.abstracts += 1;
            for s in &a.sections {
                self.sections += 1;
                self.sections_markup += u64::from(s.text.contains('<'));
                self.sections_math += u64::from(s.text.contains("<mml:"));
            }
        }
        self.other_abstracts += c.other_abstracts.len() as u64;
        self.journal_issn += u64::from(c.journal.issn.is_some());
        self.journal_issn_linking += u64::from(c.journal.issn_linking.is_some());
        for a in &c.authors {
            match a.name {
                AuthorName::Person(_) => self.persons += 1,
                AuthorName::Collective { .. } => self.collectives += 1,
            }
            match &a.orcid {
                None => self.orcid[2] += 1,
                Some(Claim::Valid(_)) => self.orcid[0] += 1,
                Some(Claim::Malformed { raw, error }) => {
                    self.orcid[1] += 1;
                    if self.malformed_samples.len() < 8 {
                        self.malformed_samples.push(format!("{raw:?}: {error}"));
                    }
                }
            }
            self.affiliations += a.affiliations.len() as u64;
            for af in &a.affiliations {
                for id in &af.ids {
                    *self.affiliation_ids.entry(id.source.token()).or_default() += 1;
                }
            }
        }
        self.investigators += c
            .investigator_lists
            .iter()
            .map(|l| l.investigators.len() as u64)
            .sum::<u64>();
        self.mesh_headings += c.mesh.len() as u64;
        self.mesh_major += c.mesh.iter().filter(|h| h.major).count() as u64;
        self.chemicals += c.chemicals.len() as u64;
        self.keywords += c.keywords.len() as u64;
        self.grants += c.grants.len() as u64;
        self.grants_with_country += c.grants.iter().filter(|g| g.country.is_some()).count() as u64;
        self.data_banks += c.data_banks.len() as u64;
        for h in &c.history {
            *self.history.entry(h.status.token()).or_default() += 1;
        }
        if let Some(ps) = c.publication_status {
            *self.publication_status.entry(ps.token()).or_default() += 1;
        }
        for r in &c.relations {
            *self.relations.entry(r.kind.token()).or_default() += 1;
        }
    }

    fn reference(&mut self, r: &Reference<'n>) {
        self.references += 1;
        let key = match (r.pmid.is_some(), r.doi.is_some(), r.pmc.is_some()) {
            (false, false, false) => "none",
            (true, false, false) => "pmid",
            (false, true, false) => "doi",
            (false, false, true) => "pmc",
            (true, true, false) => "pmid+doi",
            (true, false, true) => "pmid+pmc",
            (false, true, true) => "doi+pmc",
            (true, true, true) => "pmid+doi+pmc",
        };
        *self.reference_ids.entry(key).or_default() += 1;
        if let Some(Claim::Malformed { raw, error }) = &r.doi {
            self.reference_doi_malformed += 1;
            if self.malformed_samples.len() < 8 {
                self.malformed_samples.push(format!("{raw:?}: {error}"));
            }
        }
    }
}

#[allow(clippy::cast_precision_loss)] // reporting only
fn pubmed_normalize(path: &Path, limit: Option<u64>) -> ExitCode {
    let Some(input) = read(path) else {
        return ExitCode::from(2);
    };
    let started = Instant::now();
    let mut set = ArticleSet::new(&input);
    let mut stats = Stats::default();
    let mut records = 0u64;
    loop {
        if limit.is_some_and(|n| records >= n) {
            break;
        }
        match set.next_record() {
            Ok(Some(Record::Article(a))) => {
                records += 1;
                if let Err(err) = normalize(&a, &mut stats) {
                    eprintln!("{}: {err}", path.display());
                    eprintln!("after {records} records");
                    stats.report();
                    return ExitCode::FAILURE;
                }
            }
            Ok(Some(Record::DeleteCitation(_))) => records += 1,
            Ok(None) => break,
            Err(err) => {
                eprintln!("{}: {err}", path.display());
                return ExitCode::FAILURE;
            }
        }
    }
    let elapsed = started.elapsed();
    stats.report();
    eprintln!(
        "parse+normalize {:.2}s = {:.0} records/s",
        elapsed.as_secs_f64(),
        records as f64 / elapsed.as_secs_f64()
    );
    ExitCode::SUCCESS
}

/// Runs `f` on every work of one container.
fn crossref_drive<'a>(
    works: impl Iterator<Item = Result<source_crossref::Work<'a>, source_crossref::Error>>,
    f: &mut impl FnMut(&source_crossref::Work<'_>) -> Result<(), String>,
) -> Result<(), String> {
    for w in works {
        f(&w.map_err(|e| e.to_string())?)?;
    }
    Ok(())
}

/// Reads a Crossref file of either container and runs `f` on every work;
/// returns the decompressed size.
fn crossref_each(
    path: &Path,
    f: &mut impl FnMut(&source_crossref::Work<'_>) -> Result<(), String>,
) -> Result<usize, String> {
    let input = if path.extension().is_some_and(|e| e == "gz") {
        source_crossref::gz::read_to_string(path)
    } else {
        std::fs::read_to_string(path)
    }
    .map_err(|e| e.to_string())?;
    if input.trim_start().starts_with("{\"status\"") {
        let page = source_crossref::Page::parse(&input).map_err(|e| e.to_string())?;
        crossref_drive(page.works(), f)?;
    } else {
        crossref_drive(source_crossref::jsonl(&input), f)?;
    }
    Ok(input.len())
}

#[allow(clippy::cast_precision_loss)] // reporting only
fn crossref_check(paths: &[&str]) -> ExitCode {
    let started = Instant::now();
    let (mut works, mut references, mut authors, mut bytes) = (0u64, 0u64, 0u64, 0usize);
    let mut types: BTreeMap<String, u64> = BTreeMap::new();
    for path in paths {
        let path = Path::new(path);
        let result = crossref_each(path, &mut |w| {
            works += 1;
            references += w.reference.as_ref().map_or(0, Vec::len) as u64;
            authors += w.author.as_ref().map_or(0, Vec::len) as u64;
            *types.entry(format!("{:?}", w.r#type)).or_default() += 1;
            Ok(())
        });
        match result {
            Ok(n) => bytes += n,
            Err(err) => {
                eprintln!("{}: {err}", path.display());
                eprintln!("after {works} works");
                return ExitCode::FAILURE;
            }
        }
    }
    let elapsed = started.elapsed();
    let mib = bytes as f64 / (1024.0 * 1024.0);
    eprintln!(
        "{works} works, {references} references, {authors} authors from {} files",
        paths.len()
    );
    eprintln!("types {types:?}");
    eprintln!(
        "read+parse {:.2}s = {:.0} works/s, {:.0} MiB/s",
        elapsed.as_secs_f64(),
        works as f64 / elapsed.as_secs_f64(),
        mib / elapsed.as_secs_f64()
    );
    ExitCode::SUCCESS
}

/// The Crossref stats report: what every normalized field held.
#[derive(Default)]
struct CrossrefStats {
    works: u64,
    types: BTreeMap<String, u64>,
    doi_lowercased: u64,
    owner_differs: u64,
    issued: BTreeMap<&'static str, u64>,
    title: [u64; 3],
    alternate_titles: u64,
    subtitle_empty: u64,
    abstracts: [u64; 3],
    persons: u64,
    orgs: u64,
    with_orcid: u64,
    affiliations_with_ror: u64,
    issns: u64,
    isbns: u64,
    languages: BTreeMap<&'static str, u64>,
    references: u64,
    reference_doi: [u64; 3],
    reference_empty: u64,
    updates: BTreeMap<String, u64>,
    relations: BTreeMap<String, u64>,
    licenses: BTreeMap<String, u64>,
    malformed_samples: Vec<String>,
}

impl<'n> CrossrefEmit<'n> for CrossrefStats {
    fn work(&mut self, w: &source_crossref::normalize::Work<'n>) {
        self.works += 1;
        *self.types.entry(format!("{:?}", w.kind)).or_default() += 1;
        self.doi_lowercased += u64::from(!w.doi.is_borrowed());
        self.owner_differs += u64::from(w.owner != w.doi.registrant());
        *self
            .issued
            .entry(w.issued.map_or("none", |d| d.precision()))
            .or_default() += 1;
        match w.title {
            None => self.title[2] += 1,
            Some(t) if t.contains('<') => self.title[1] += 1,
            Some(_) => self.title[0] += 1,
        }
        self.alternate_titles += w.alternate_titles.len() as u64;
        self.subtitle_empty += u64::from(w.subtitle.is_some_and(str::is_empty));
        match w.r#abstract {
            None => self.abstracts[2] += 1,
            Some(a) if a.trim_start().starts_with("<jats:") => self.abstracts[0] += 1,
            Some(_) => self.abstracts[1] += 1,
        }
        for a in &w.authors {
            match a.name {
                source_crossref::normalize::AuthorName::Person { .. } => self.persons += 1,
                source_crossref::normalize::AuthorName::Org(_) => self.orgs += 1,
            }
            self.with_orcid += u64::from(a.orcid.is_some());
            self.affiliations_with_ror +=
                a.affiliations.iter().filter(|x| x.ror.is_some()).count() as u64;
        }
        self.issns += w.issns.len() as u64;
        self.isbns += w.isbns.len() as u64;
        if let Some(l) = w.language {
            *self.languages.entry(l.alpha_3()).or_default() += 1;
        }
        for u in w.updates_to.iter().chain(&w.updated_by) {
            *self.updates.entry(format!("{:?}", u.kind)).or_default() += 1;
        }
        for r in &w.relations {
            *self.relations.entry(r.kind.to_owned()).or_default() += 1;
        }
        for l in &w.licenses {
            *self.licenses.entry(format!("{:?}", l.version)).or_default() += 1;
        }
    }

    fn reference(&mut self, _from: &Doi<'n>, r: &source_crossref::normalize::Reference<'n>) {
        self.references += 1;
        match &r.doi {
            None => self.reference_doi[2] += 1,
            Some(Claim::Valid(_)) => self.reference_doi[0] += 1,
            Some(Claim::Malformed { raw, error }) => {
                self.reference_doi[1] += 1;
                if self.malformed_samples.len() < 6 {
                    self.malformed_samples.push(format!("{raw:?}: {error}"));
                }
            }
        }
        if r.doi.is_none()
            && r.unstructured.is_none()
            && r.article_title.is_none()
            && r.journal_title.is_none()
            && r.volume_title.is_none()
            && r.author.is_none()
        {
            self.reference_empty += 1;
        }
    }
}

impl CrossrefStats {
    fn report(&self) {
        eprintln!("works {}", self.works);
        eprintln!("  types {:?}", self.types);
        eprintln!(
            "  doi lowercased (allocations) {}  owner prefix != registrant {}",
            self.doi_lowercased, self.owner_differs
        );
        eprintln!("  issued {:?}", self.issued);
        eprintln!(
            "  title plain {} markup {} none {}  alternate titles {}  empty subtitles {}",
            self.title[0], self.title[1], self.title[2], self.alternate_titles, self.subtitle_empty
        );
        eprintln!(
            "  abstract jats {} other {} none {}",
            self.abstracts[0], self.abstracts[1], self.abstracts[2]
        );
        eprintln!(
            "  authors persons {} orgs {} with orcid {}  affiliations with ror {}",
            self.persons, self.orgs, self.with_orcid, self.affiliations_with_ror
        );
        eprintln!("  issns {}  isbns {}", self.issns, self.isbns);
        eprintln!("  languages {:?}", self.languages);
        eprintln!("  updates {:?}", self.updates);
        eprintln!("  relations {:?}", self.relations);
        eprintln!("  licenses {:?}", self.licenses);
        eprintln!("references {}", self.references);
        eprintln!(
            "  doi valid {} malformed {} none {}  empty {}",
            self.reference_doi[0],
            self.reference_doi[1],
            self.reference_doi[2],
            self.reference_empty
        );
        if !self.malformed_samples.is_empty() {
            eprintln!("  malformed samples:");
            for s in &self.malformed_samples {
                eprintln!("    {s}");
            }
        }
    }
}

#[allow(clippy::cast_precision_loss)] // reporting only
fn crossref_normalize(paths: &[&str]) -> ExitCode {
    let started = Instant::now();
    let mut stats = CrossrefStats::default();
    for path in paths {
        let path = Path::new(path);
        let result = crossref_each(path, &mut |w| {
            source_crossref::normalize::normalize(w, &mut stats).map_err(|e| e.to_string())
        });
        if let Err(err) = result {
            eprintln!("{}: {err}", path.display());
            eprintln!("after {} works", stats.works);
            stats.report();
            return ExitCode::FAILURE;
        }
    }
    let elapsed = started.elapsed();
    stats.report();
    eprintln!(
        "read+parse+normalize {:.2}s = {:.0} works/s",
        elapsed.as_secs_f64(),
        stats.works as f64 / elapsed.as_secs_f64()
    );
    ExitCode::SUCCESS
}

/// Reads a part file, gzipped or not.
fn openalex_read(path: &Path) -> Option<String> {
    let input = if path.extension().is_some_and(|e| e == "gz") {
        source_openalex::gz::read_to_string(path)
    } else {
        std::fs::read_to_string(path)
    };
    match input {
        Ok(input) => Some(input),
        Err(err) => {
            eprintln!("{}: {err}", path.display());
            None
        }
    }
}

#[allow(clippy::cast_precision_loss)] // reporting only
fn openalex_check(paths: &[&str]) -> ExitCode {
    let started = Instant::now();
    let (mut works, mut references, mut authorships, mut abstracts, mut bytes) =
        (0u64, 0u64, 0u64, 0u64, 0usize);
    let mut types: BTreeMap<String, u64> = BTreeMap::new();
    let mut oa: BTreeMap<String, u64> = BTreeMap::new();
    for path in paths {
        let path = Path::new(path);
        let Some(input) = openalex_read(path) else {
            return ExitCode::from(2);
        };
        bytes += input.len();
        for w in source_openalex::jsonl(&input) {
            match w {
                Ok(w) => {
                    works += 1;
                    references += w.referenced_works.len() as u64;
                    authorships += w.authorships.len() as u64;
                    abstracts += u64::from(w.abstract_inverted_index.is_some());
                    *types.entry(format!("{:?}", w.r#type)).or_default() += 1;
                    *oa.entry(format!("{:?}", w.open_access.oa_status))
                        .or_default() += 1;
                }
                Err(err) => {
                    eprintln!("{}: {err}", path.display());
                    eprintln!("after {works} works");
                    return ExitCode::FAILURE;
                }
            }
        }
    }
    let elapsed = started.elapsed();
    let mib = bytes as f64 / (1024.0 * 1024.0);
    eprintln!(
        "{works} works, {references} referenced works, {authorships} authorships, {abstracts} abstracts from {} files",
        paths.len()
    );
    eprintln!("types {types:?}");
    eprintln!("oa status {oa:?}");
    eprintln!(
        "read+parse {:.2}s = {:.0} works/s, {:.0} MiB/s",
        elapsed.as_secs_f64(),
        works as f64 / elapsed.as_secs_f64(),
        mib / elapsed.as_secs_f64()
    );
    ExitCode::SUCCESS
}

/// The OpenAlex stats report: what every normalized field held.
#[derive(Default)]
struct OpenAlexStats {
    works: u64,
    types: BTreeMap<String, u64>,
    doi: [u64; 3],
    doi_lowercased: u64,
    pmid: u64,
    mag: [u64; 2],
    languages: BTreeMap<&'static str, u64>,
    language_malformed: u64,
    title: [u64; 3],
    oa_status: BTreeMap<String, u64>,
    oa_url: u64,
    with_best_oa: u64,
    authorships: u64,
    authors_with_id: u64,
    with_orcid: u64,
    with_raw_orcid: u64,
    orcid_conflicts: u64,
    corresponding: u64,
    institutions: u64,
    institution_countries: u64,
    authorship_countries: u64,
    affiliations: u64,
    locations: u64,
    provenance: BTreeMap<String, u64>,
    location_ids: BTreeMap<&'static str, u64>,
    with_source: u64,
    issn_l: u64,
    issn: [u64; 2],
    host_lineage: u64,
    licenses: BTreeMap<String, u64>,
    versions: BTreeMap<String, u64>,
    topics: u64,
    keywords: u64,
    concepts: u64,
    mesh: u64,
    sdgs: u64,
    awards: u64,
    funders: u64,
    funders_with_ror: u64,
    apc: [u64; 2],
    abstracts: u64,
    counts_by_year: u64,
    indexed_in: BTreeMap<String, u64>,
    flags: [u64; 3],
    references: u64,
    self_references: u64,
    related: u64,
    malformed_samples: Vec<String>,
}

impl OpenAlexStats {
    fn sample(&mut self, raw: &str, error: &impl Display) {
        if self.malformed_samples.len() < 8 {
            self.malformed_samples.push(format!("{raw:?}: {error}"));
        }
    }

    fn report(&self) {
        eprintln!("works {}", self.works);
        eprintln!("  types {:?}", self.types);
        eprintln!(
            "  doi valid {} malformed {} none {}  lowercased (allocations) {}",
            self.doi[0], self.doi[1], self.doi[2], self.doi_lowercased
        );
        eprintln!(
            "  pmid {}  mag {} (not the W number {})",
            self.pmid, self.mag[0], self.mag[1]
        );
        eprintln!(
            "  languages {:?}  malformed {}",
            self.languages, self.language_malformed
        );
        eprintln!(
            "  title plain {} markup {} empty {}",
            self.title[0], self.title[1], self.title[2]
        );
        eprintln!(
            "  oa status {:?}  oa_url {}  with best oa location {}",
            self.oa_status, self.oa_url, self.with_best_oa
        );
        eprintln!(
            "  authorships {}  with author id {}  orcid {} raw orcid {} conflicting {}  corresponding {}",
            self.authorships,
            self.authors_with_id,
            self.with_orcid,
            self.with_raw_orcid,
            self.orcid_conflicts,
            self.corresponding
        );
        eprintln!(
            "  institutions {} (with country {})  authorship countries {}  affiliations {}",
            self.institutions,
            self.institution_countries,
            self.authorship_countries,
            self.affiliations
        );
        eprintln!(
            "  locations {}  provenance {:?}  ids {:?}",
            self.locations, self.provenance, self.location_ids
        );
        eprintln!(
            "  sources {}  issn_l {}  issn valid {} malformed {}  host lineage entries {}",
            self.with_source, self.issn_l, self.issn[0], self.issn[1], self.host_lineage
        );
        eprintln!("  licenses {:?}", self.licenses);
        eprintln!("  versions {:?}", self.versions);
        eprintln!(
            "  topics {}  keywords {}  concepts {}  mesh {}  sdgs {}",
            self.topics, self.keywords, self.concepts, self.mesh, self.sdgs
        );
        eprintln!(
            "  awards {}  funders {} (with ror {})  apc list {} paid {}",
            self.awards, self.funders, self.funders_with_ror, self.apc[0], self.apc[1]
        );
        eprintln!(
            "  abstracts {}  counts_by_year entries {}  indexed_in {:?}",
            self.abstracts, self.counts_by_year, self.indexed_in
        );
        eprintln!(
            "  retracted {}  xpac {}  has_fulltext {}  related works {}",
            self.flags[0], self.flags[1], self.flags[2], self.related
        );
        eprintln!(
            "references {} (self references {})",
            self.references, self.self_references
        );
        if !self.malformed_samples.is_empty() {
            eprintln!("  malformed samples:");
            for s in &self.malformed_samples {
                eprintln!("    {s}");
            }
        }
    }
}

impl<'n> OpenAlexEmit<'n> for OpenAlexStats {
    #[allow(clippy::too_many_lines)] // one counter per field group
    fn work(&mut self, w: &source_openalex::normalize::Work<'n>) {
        self.works += 1;
        *self.types.entry(format!("{:?}", w.kind)).or_default() += 1;
        match &w.doi {
            None => self.doi[2] += 1,
            Some(Claim::Valid(d)) => {
                self.doi[0] += 1;
                self.doi_lowercased += u64::from(!d.is_borrowed());
            }
            Some(Claim::Malformed { raw, error }) => {
                self.doi[1] += 1;
                self.sample(raw, error);
            }
        }
        self.pmid += u64::from(w.pmid.is_some());
        if let Some(m) = w.mag {
            self.mag[0] += 1;
            self.mag[1] += u64::from(u64::from(m.get()) != w.id.get());
        }
        match &w.language {
            None => {}
            Some(Claim::Valid(l)) => *self.languages.entry(l.alpha_3()).or_default() += 1,
            Some(Claim::Malformed { raw, error }) => {
                self.language_malformed += 1;
                self.sample(raw, error);
            }
        }
        if w.title.is_empty() {
            self.title[2] += 1;
        } else if w.title.contains('<') {
            self.title[1] += 1;
        } else {
            self.title[0] += 1;
        }
        *self
            .oa_status
            .entry(format!("{:?}", w.open_access.status))
            .or_default() += 1;
        self.oa_url += u64::from(w.open_access.oa_url.is_some());
        self.with_best_oa += u64::from(w.best_oa_location().is_some());
        for a in &w.authorships {
            self.authorships += 1;
            self.authors_with_id += u64::from(a.author.id.is_some());
            self.with_orcid += u64::from(a.author.orcid.is_some());
            self.with_raw_orcid += u64::from(a.raw_orcid.is_some());
            self.orcid_conflicts += u64::from(matches!(
                (a.author.orcid, a.raw_orcid),
                (Some(x), Some(y)) if x != y
            ));
            self.corresponding += u64::from(a.is_corresponding);
            self.institutions += a.institutions.len() as u64;
            self.institution_countries += a
                .institutions
                .iter()
                .filter(|i| i.country.is_some())
                .count() as u64;
            self.authorship_countries += a.countries.len() as u64;
            self.affiliations += a.affiliations.len() as u64;
        }
        for l in &w.locations {
            self.locations += 1;
            *self
                .provenance
                .entry(format!("{:?}", l.provenance))
                .or_default() += 1;
            let kind = match l.id {
                LocationId::Mag(_) => "mag",
                LocationId::Pmid(_) => "pmid",
                LocationId::Doi(_) => "doi",
                LocationId::Repo(_) => "repo",
            };
            *self.location_ids.entry(kind).or_default() += 1;
            if let Some(s) = &l.source {
                self.with_source += 1;
                self.issn_l += u64::from(s.issn_l.is_some());
                for c in &s.issns {
                    match c {
                        Claim::Valid(_) => self.issn[0] += 1,
                        Claim::Malformed { raw, error } => {
                            self.issn[1] += 1;
                            self.sample(raw, error);
                        }
                    }
                }
                self.host_lineage += s.host_organization_lineage.len() as u64;
            }
            if let Some(license) = l.license {
                *self.licenses.entry(license.token().to_owned()).or_default() += 1;
            }
            if let Some(v) = l.version {
                *self.versions.entry(format!("{v:?}")).or_default() += 1;
            }
        }
        self.topics += w.topics.len() as u64;
        self.keywords += w.keywords.len() as u64;
        self.concepts += w.concepts.len() as u64;
        self.mesh += w.mesh.len() as u64;
        self.sdgs += w.sustainable_development_goals.len() as u64;
        self.awards += w.awards.len() as u64;
        self.funders += w.funders.len() as u64;
        self.funders_with_ror += w.funders.iter().filter(|f| f.ror.is_some()).count() as u64;
        self.apc[0] += u64::from(w.apc_list.is_some());
        self.apc[1] += u64::from(w.apc_paid.is_some());
        self.abstracts += u64::from(w.abstract_inverted_index.is_some());
        self.counts_by_year += w.counts_by_year.len() as u64;
        for i in w.indexed_in.iter() {
            *self.indexed_in.entry(format!("{i:?}")).or_default() += 1;
        }
        self.flags[0] += u64::from(w.is_retracted);
        self.flags[1] += u64::from(w.is_xpac);
        self.flags[2] += u64::from(w.has_fulltext);
        self.related += w.related_works.len() as u64;
    }

    fn reference(&mut self, from: OpenAlexId, to: OpenAlexId) {
        self.references += 1;
        self.self_references += u64::from(from == to);
    }
}

#[allow(clippy::cast_precision_loss)] // reporting only
fn openalex_normalize(paths: &[&str]) -> ExitCode {
    let started = Instant::now();
    let mut stats = OpenAlexStats::default();
    let mut bytes = 0usize;
    for path in paths {
        let path = Path::new(path);
        let Some(input) = openalex_read(path) else {
            return ExitCode::from(2);
        };
        bytes += input.len();
        for w in source_openalex::jsonl(&input) {
            let result = w.map_err(|e| e.to_string()).and_then(|w| {
                source_openalex::normalize::normalize(&w, &mut stats).map_err(|e| e.to_string())
            });
            if let Err(err) = result {
                eprintln!("{}: {err}", path.display());
                eprintln!("after {} works", stats.works);
                stats.report();
                return ExitCode::FAILURE;
            }
        }
    }
    let elapsed = started.elapsed();
    let mib = bytes as f64 / (1024.0 * 1024.0);
    stats.report();
    eprintln!(
        "read+parse+normalize {:.2}s = {:.0} works/s, {:.0} MiB/s",
        elapsed.as_secs_f64(),
        stats.works as f64 / elapsed.as_secs_f64(),
        mib / elapsed.as_secs_f64()
    );
    ExitCode::SUCCESS
}
