//! Command line for the pipeline.
//!
//! * `id <kind>` runs one identifier parser over stdin, one value per line.
//! * `<source> check <file>... [--limit N]` reads raw files through the
//!   source's strict native types and reports counts and throughput.
//! * `<source> normalize <file>... [--limit N]` parses and normalizes, and
//!   prints the source's stats report: what every field held.
//!
//! Sources are `pubmed`, `crossref` and `openalex`. Every one goes through
//! the same runner (`ingest::normalize`); what differs per source is one
//! registration line and its stats sink. All exit non-zero on the first
//! failure and say exactly where it was — source, file, record ordinal,
//! stage, native id, field, raw value — the seed of the "reproduce against
//! one record" command the failure report promises (plan §6).

use std::collections::BTreeMap;
use std::fmt::Display;
use std::io::{self, BufRead};
use std::process::ExitCode;

use ingest::{Failure, Sink, Source, Stage};
use model::{Claim, Doi, OpenAlexId, ParseError, Pmid};
use source_crossref::Crossref;
use source_openalex::OpenAlex;
use source_openalex::normalize::LocationId;
use source_pubmed::Pubmed;
use source_pubmed::normalize::{Citation, Reference};

const USAGE: &str = "usage:
  ol-rht id <pmid|pmid-url|openalex|openalex-url> < values.txt
  ol-rht <pubmed|crossref|openalex> check <file>... [--limit N]
  ol-rht <pubmed|crossref|openalex> normalize <file>... [--limit N]";

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
        [source, action, rest @ ..] => {
            let Some((paths, limit)) = split_limit(rest) else {
                return usage();
            };
            match (*source, *action) {
                ("pubmed", "check") => check::<Pubmed>(&paths, limit),
                ("pubmed", "normalize") => normalize::<Pubmed, PubmedStats>(&paths, limit),
                ("crossref", "check") => check::<Crossref>(&paths, limit),
                ("crossref", "normalize") => normalize::<Crossref, CrossrefStats>(&paths, limit),
                ("openalex", "check") => check::<OpenAlex>(&paths, limit),
                ("openalex", "normalize") => normalize::<OpenAlex, OpenAlexStats>(&paths, limit),
                _ => usage(),
            }
        }
        _ => usage(),
    }
}

/// Splits a trailing `--limit N` off the paths. `None` when there are no
/// paths or the flag is malformed.
fn split_limit<'a>(rest: &[&'a str]) -> Option<(Vec<&'a str>, Option<u64>)> {
    match rest {
        [] => None,
        [paths @ .., "--limit", n] => {
            (!paths.is_empty()).then_some((paths.to_vec(), Some(n.parse().ok()?)))
        }
        paths if paths.contains(&"--limit") => None,
        paths => Some((paths.to_vec(), None)),
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

/// A source's stats report (plan §6): what every normalized field held.
trait StatsReport: Default {
    fn report(&self);
}

fn check<S: Source>(paths: &[&str], limit: Option<u64>) -> ExitCode {
    match ingest::parse::<S>(paths, limit) {
        Ok(report) => {
            eprintln!("{}: {report}", S::NAME);
            ExitCode::SUCCESS
        }
        Err(failure) => failed(&failure),
    }
}

fn normalize<S, K>(paths: &[&str], limit: Option<u64>) -> ExitCode
where
    S: Source,
    K: StatsReport + for<'n> Sink<'n, S>,
{
    let mut stats = K::default();
    match ingest::normalize::<S, K>(paths, limit, &mut stats) {
        Ok(report) => {
            stats.report();
            eprintln!("{}: {report}", S::NAME);
            ExitCode::SUCCESS
        }
        Err(failure) => {
            stats.report();
            failed(&failure)
        }
    }
}

fn failed(failure: &Failure) -> ExitCode {
    eprintln!("{failure}");
    if failure.stage == Stage::Read {
        ExitCode::from(2)
    } else {
        ExitCode::FAILURE
    }
}

/// Keeps the first few malformed values for the report.
fn sample(samples: &mut Vec<String>, raw: &str, error: &impl Display) {
    if samples.len() < 8 {
        samples.push(format!("{raw:?}: {error}"));
    }
}

/// The PubMed stats report.
#[derive(Default)]
struct PubmedStats {
    citations: u64,
    deletions: u64,
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

impl PubmedStats {
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
                sample(&mut self.malformed_samples, raw, error);
            }
        }
    }
}

impl StatsReport for PubmedStats {
    fn report(&self) {
        eprintln!("citations {}  deletions {}", self.citations, self.deletions);
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

impl<'n> Sink<'n, Pubmed> for PubmedStats {
    fn work(&mut self, c: &Citation<'n>) {
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
                    sample(&mut self.malformed_samples, raw, error);
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

    fn reference(&mut self, _from: &Pmid, r: &Reference<'n>) {
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
            sample(&mut self.malformed_samples, raw, error);
        }
    }

    fn deletion(&mut self, _: &Pmid) {
        self.deletions += 1;
    }
}

/// The Crossref stats report.
#[derive(Default)]
struct CrossrefStats {
    works: u64,
    deletions: u64,
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

impl<'n> Sink<'n, Crossref> for CrossrefStats {
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
                sample(&mut self.malformed_samples, raw, error);
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

    fn deletion(&mut self, _: &Doi<'n>) {
        self.deletions += 1;
    }
}

impl StatsReport for CrossrefStats {
    fn report(&self) {
        eprintln!("works {}  deletions {}", self.works, self.deletions);
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

/// The OpenAlex stats report.
#[derive(Default)]
struct OpenAlexStats {
    works: u64,
    deletions: u64,
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

impl StatsReport for OpenAlexStats {
    fn report(&self) {
        eprintln!("works {}  deletions {}", self.works, self.deletions);
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

impl<'n> Sink<'n, OpenAlex> for OpenAlexStats {
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
                sample(&mut self.malformed_samples, raw, error);
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
                sample(&mut self.malformed_samples, raw, error);
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
                            sample(&mut self.malformed_samples, raw, error);
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

    fn reference(&mut self, from: &OpenAlexId, to: &OpenAlexId) {
        self.references += 1;
        self.self_references += u64::from(from == to);
    }

    fn deletion(&mut self, _: &OpenAlexId) {
        self.deletions += 1;
    }
}
