//! D13 of the tokenizer plan (`docs/tokenizer-plan.md`): an answer key for
//! language identification that neither a detector nor a record's own tag
//! made, and the score of a detector's answers against it.
//!
//! The key is PubMed's, labelled by NLM (`docs/inbox/language-identification.md`):
//! `ArticleTitle` is English — NLM translates a title that is not, and
//! brackets the translation — and `VernacularTitle` is in the article's
//! language as NLM's indexers tag it; an `OtherAbstract` names its own
//! language. A text is labelled only where that reading is unambiguous: an
//! article in one language. Beside it, Crossref's titles by script,
//! unlabelled, for a person to read and for detectors to be compared where
//! no label exists.
//!
//! Every stratum — kind, language or script, length — keeps at most a
//! fixed number of texts, those whose seeded hash is smallest, so the key
//! is the same on every run over the same store; each stratum's
//! population is kept beside it, so a score can be weighted back to the
//! corpus.

use std::collections::{BTreeMap, BinaryHeap};
use std::fmt::Write as _;
use std::io::{self, Write as _};
use std::path::Path;
use std::process::ExitCode;
use std::time::Instant;

use ingest::Store;
use merge::copies::{Copies, native_of};
use merge::view::strip_markup;
use search::analysis::Script;
use source_crossref::Crossref;
use source_pubmed::Pubmed;

use super::Options;
use super::analysis::dominant;
use super::survey::{each, read_copies};

/// Texts kept of each labelled stratum.
const CAP: usize = 1_000;
/// Texts kept of each of Crossref's strata, for a person to read.
const CROSSREF_CAP: usize = 200;
/// What every text's place in its stratum is drawn with.
const SEED: &[u8] = b"ol-rht language key v1";

/// Lengths in characters: each bucket's upper bound, inclusive, and its
/// label. Characters, not words: a Chinese title is one run of letters.
const LENGTHS: [(usize, &str); 5] = [
    (20, "1-20"),
    (50, "21-50"),
    (100, "51-100"),
    (200, "101-200"),
    (usize::MAX, "201+"),
];

/// The key's file, and the account of how it was drawn.
pub(crate) const KEY: &str = "key.tsv";
const ABOUT: &str = "about.json";

/// What a text is, and so where its label comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Kind {
    /// `ArticleTitle` of an article in English alone.
    Title,
    /// `ArticleTitle` of an article in one other language: NLM's
    /// translation, brackets removed. English.
    TranslatedTitle,
    /// `VernacularTitle` of an article in one language.
    VernacularTitle,
    /// `Abstract` of an article in English alone.
    Abstract,
    /// `OtherAbstract`, in the language it names.
    OtherAbstract,
    /// Crossref's `title[0]`: no label.
    CrossrefTitle,
}

impl Kind {
    const fn name(self) -> &'static str {
        match self {
            Self::Title => "title",
            Self::TranslatedTitle => "translated-title",
            Self::VernacularTitle => "vernacular-title",
            Self::Abstract => "abstract",
            Self::OtherAbstract => "other-abstract",
            Self::CrossrefTitle => "crossref-title",
        }
    }
}

/// One text kept, ordered by its draw.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Kept {
    draw: u64,
    id: String,
    language: String,
    tagged: String,
    script: &'static str,
    chars: usize,
    text: String,
    /// A title's own work's abstract, cut to [`CONTEXT_CHARS`]; empty for
    /// anything else.
    context: String,
}

/// Characters of a title's abstract the key keeps beside it: enough for a
/// detector to be sure of its language.
const CONTEXT_CHARS: usize = 2_000;

/// A title's abstract as offered: already plain (PubMed's sections,
/// joined), or markup still in it (Crossref's JATS), stripped only if the
/// title is kept.
#[derive(Debug, Clone, Copy)]
enum Context<'a> {
    Plain(&'a str),
    Markup(&'a str),
}

impl Context<'_> {
    fn text(self) -> String {
        let text = match self {
            Self::Plain(t) => t.to_owned(),
            Self::Markup(t) => plain(t),
        };
        text.chars().take(CONTEXT_CHARS).collect()
    }
}

#[derive(Debug, Default)]
struct Stratum {
    population: u64,
    /// The smallest draws, the largest on top.
    kept: BinaryHeap<Kept>,
}

/// Where each text's place is drawn from: its id and the seed, nothing
/// the text says.
fn draw(id: &str) -> u64 {
    let mut h = blake3::Hasher::new();
    h.update(SEED);
    h.update(id.as_bytes());
    let bytes = h.finalize();
    let mut first = [0u8; 8];
    first.copy_from_slice(&bytes.as_bytes()[..8]);
    u64::from_le_bytes(first)
}

fn length(chars: usize) -> &'static str {
    LENGTHS
        .iter()
        .find(|(most, _)| chars <= *most)
        .map_or("201+", |(_, label)| label)
}

/// A text as the key holds it: markup stripped, on one line.
fn plain(raw: &str) -> String {
    strip_markup(raw)
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect::<String>()
        .trim()
        .to_owned()
}

#[derive(Debug, Default)]
struct Sampler {
    strata: BTreeMap<(Kind, String, &'static str), Stratum>,
    pubmed: u64,
    crossref: u64,
}

impl Sampler {
    /// Offers one text, with its work's abstract when it is a title. A
    /// labelled one is filed by its language, an unlabelled one by its
    /// script and whether it has an abstract beside it.
    fn offer(
        &mut self,
        kind: Kind,
        id: String,
        language: Option<String>,
        tagged: String,
        text: String,
        context: Option<Context<'_>>,
    ) {
        if !text.chars().any(char::is_alphanumeric) {
            return;
        }
        let context = context.filter(|c| match c {
            Context::Plain(t) | Context::Markup(t) => !t.trim().is_empty(),
        });
        let chars = text.chars().count();
        let script = dominant(&text).map_or("none", Script::name);
        let (label, cap) = match &language {
            Some(language) => (language.clone(), CAP),
            None if context.is_some() => (format!("{script}+abstract"), CROSSREF_CAP),
            None => (script.to_owned(), CROSSREF_CAP),
        };
        let stratum = self.strata.entry((kind, label, length(chars))).or_default();
        stratum.population += 1;
        let draw = draw(&id);
        if stratum.kept.len() == cap {
            if stratum.kept.peek().is_some_and(|top| draw >= top.draw) {
                return;
            }
            stratum.kept.pop();
        }
        stratum.kept.push(Kept {
            draw,
            id,
            language: language.unwrap_or_else(|| "-".to_owned()),
            tagged,
            script,
            chars,
            text,
            context: context.map(Context::text).unwrap_or_default(),
        });
    }

    fn pubmed(&mut self, c: &source_pubmed::normalize::Citation<'_>) {
        self.pubmed += 1;
        let [language] = c.languages.as_slice() else {
            return;
        };
        let language = language.to_string();
        if matches!(language.as_str(), "und" | "mul" | "zxx" | "mis") {
            return;
        }
        let tagged = language.clone();
        let id = format!("pmid:{}", c.pmid);
        let english = language == "eng";
        let title = plain(c.title);
        // The record's own abstract: the text a title stands beside in the
        // index, whatever language the article is in.
        let abstract_text: Option<String> = c.r#abstract.as_ref().map(|a| {
            a.sections
                .iter()
                .map(|s| plain(s.text))
                .collect::<Vec<_>>()
                .join(" ")
        });
        let context = abstract_text.as_deref().map(Context::Plain);
        if english {
            self.offer(
                Kind::Title,
                format!("{id}/title"),
                Some(language.clone()),
                tagged.clone(),
                title,
                context,
            );
            if let Some(text) = &abstract_text {
                self.offer(
                    Kind::Abstract,
                    format!("{id}/abstract"),
                    Some(language.clone()),
                    tagged.clone(),
                    text.clone(),
                    None,
                );
            }
        } else {
            if let Some(inner) = title
                .strip_prefix('[')
                .and_then(|t| t.strip_suffix("].").or_else(|| t.strip_suffix(']')))
            {
                self.offer(
                    Kind::TranslatedTitle,
                    format!("{id}/title"),
                    Some("eng".to_owned()),
                    tagged.clone(),
                    inner.trim().to_owned(),
                    context,
                );
            }
            // Not a title the index holds (it holds NLM's English one), but
            // a labelled title in another language beside what is most
            // often an English abstract: the case where borrowing the
            // abstract's language would go wrong.
            if let Some(v) = c.vernacular_title {
                self.offer(
                    Kind::VernacularTitle,
                    format!("{id}/vernacular"),
                    Some(language.clone()),
                    tagged.clone(),
                    plain(v),
                    context,
                );
            }
        }
        for (i, other) in c.other_abstracts.iter().enumerate() {
            let model::Claim::Valid(lang) = &other.language else {
                continue;
            };
            let own = lang.to_string();
            if matches!(own.as_str(), "und" | "mul" | "zxx" | "mis") {
                continue;
            }
            let text: Vec<String> = other.sections.iter().map(|s| plain(s.text)).collect();
            self.offer(
                Kind::OtherAbstract,
                format!("{id}/other-abstract/{i}"),
                Some(own),
                tagged.clone(),
                text.join(" "),
                None,
            );
        }
    }

    fn crossref(&mut self, w: &source_crossref::normalize::Work<'_>) {
        self.crossref += 1;
        let Some(title) = w.title else { return };
        let Some(id) = native_of::<Crossref>(w).map(|n| n.to_string()) else {
            return;
        };
        let tagged = w.language.map_or_else(|| "-".to_owned(), |l| l.to_string());
        self.offer(
            Kind::CrossrefTitle,
            format!("{id}/title"),
            None,
            tagged,
            plain(title),
            w.r#abstract.map(Context::Markup),
        );
    }
}

/// `evaluate language-key --out <dir> [--store <root>] [--limit N]`.
pub(crate) fn key(opts: &Options) -> ExitCode {
    let what = "evaluate language-key";
    let Some(out) = opts.out.as_ref() else {
        return super::usage();
    };
    if !opts.paths.is_empty()
        || opts.workers.is_some()
        || opts.mailto.is_some()
        || opts.seed.is_some()
        || opts.by.is_some()
        || opts.related.is_some()
        || opts.walk.is_some()
        || opts.depth.is_some()
        || opts.survey
    {
        return super::usage();
    }
    if out.exists() {
        eprintln!(
            "{what}: {} exists; a key is kept, so name a new one",
            out.display()
        );
        return ExitCode::FAILURE;
    }
    let limit = opts.limit.map(|n| usize::try_from(n).unwrap_or(usize::MAX));
    match key_of(&opts.store(), out, limit) {
        Ok(summary) => {
            let mut stdout = io::stdout().lock();
            match stdout
                .write_all(summary.as_bytes())
                .and_then(|()| stdout.flush())
            {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("stdout: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        Err(e) => {
            eprintln!("{what}: {e}");
            ExitCode::FAILURE
        }
    }
}

fn key_of(store: &Store, out: &Path, limit: Option<usize>) -> Result<String, String> {
    let what = "evaluate language-key";
    let started = Instant::now();
    let pubmed = super::Sets::find::<Pubmed>(store).map_err(|e| e.to_string())?;
    let crossref = super::Sets::find::<Crossref>(store).map_err(|e| e.to_string())?;
    let mut copies = Copies::new();
    read_copies::<Pubmed>(&mut copies, &pubmed, limit)?;
    read_copies::<Crossref>(&mut copies, &crossref, limit)?;
    copies.settle().map_err(|e| e.to_string())?;
    super::phase(what, "copies read", started);
    let mut sampler = Sampler::default();
    for (path, set) in &pubmed.works {
        each::<Pubmed>(path, set, limit, &copies, |c| sampler.pubmed(c))?;
    }
    super::phase(what, "PubMed read", started);
    for (path, set) in &crossref.works {
        each::<Crossref>(path, set, limit, &copies, |w| sampler.crossref(w))?;
    }
    super::phase(what, "Crossref read", started);
    write(out, store, &sampler, limit).map_err(|e| format!("{}: {e}", out.display()))?;

    let mut summary = String::new();
    let _ = writeln!(
        summary,
        "{what}: {} PubMed and {} Crossref records read{}; {:.0} s",
        sampler.pubmed,
        sampler.crossref,
        if limit.is_some() {
            " (--limit: each set in part)"
        } else {
            ""
        },
        started.elapsed().as_secs_f64()
    );
    let mut by_kind: BTreeMap<Kind, (u64, usize)> = BTreeMap::new();
    for ((kind, _, _), s) in &sampler.strata {
        let e = by_kind.entry(*kind).or_default();
        e.0 += s.population;
        e.1 += s.kept.len();
    }
    for (kind, (population, kept)) in by_kind {
        let _ = writeln!(
            summary,
            "  {:<18} {population:>12} texts, {kept:>8} kept",
            kind.name()
        );
    }
    let _ = writeln!(summary, "  key: {}", out.join(KEY).display());
    Ok(summary)
}

/// The key and its account, through a `.partial` directory renamed once
/// both are whole.
fn write(out: &Path, store: &Store, sampler: &Sampler, limit: Option<usize>) -> io::Result<()> {
    let partial = std::path::PathBuf::from(format!("{}.partial", out.display()));
    if partial.exists() {
        std::fs::remove_dir_all(&partial)?;
    }
    std::fs::create_dir_all(&partial)?;
    let mut file = io::BufWriter::new(std::fs::File::create(partial.join(KEY))?);
    writeln!(
        file,
        "id\tkind\tlanguage\ttagged\tscript\tlength\tchars\ttext\tcontext"
    )?;
    let mut strata = Vec::new();
    for ((kind, label, length), s) in &sampler.strata {
        let mut kept: Vec<&Kept> = s.kept.iter().collect();
        kept.sort();
        for k in &kept {
            writeln!(
                file,
                "{}\t{}\t{}\t{}\t{}\t{length}\t{}\t{}\t{}",
                k.id,
                kind.name(),
                k.language,
                k.tagged,
                k.script,
                k.chars,
                k.text,
                k.context
            )?;
        }
        strata.push(serde_json::json!({
            "kind": kind.name(),
            "label": label,
            "length": length,
            "population": s.population,
            "kept": kept.len(),
        }));
    }
    file.into_inner()
        .map_err(io::IntoInnerError::into_error)?
        .sync_all()?;
    let about = serde_json::json!({
        "store": store.root().display().to_string(),
        "model": model::VERSION,
        "seed": String::from_utf8_lossy(SEED),
        "cap": CAP,
        "crossref_cap": CROSSREF_CAP,
        "limit": limit,
        "pubmed_records": sampler.pubmed,
        "crossref_records": sampler.crossref,
        "strata": strata,
        "ol_rht": env!("CARGO_PKG_VERSION"),
    });
    let mut text = serde_json::to_string_pretty(&about).map_err(io::Error::other)?;
    text.push('\n');
    std::fs::write(partial.join(ABOUT), text)?;
    std::fs::rename(&partial, out)
}

// --- the score -------------------------------------------------------------

/// The confidences an English call is read at.
const THRESHOLDS: [f64; 6] = [0.0, 0.5, 0.7, 0.9, 0.95, 0.99];
/// Confidence bins for calibration: each bin's lower bound.
const BINS: [f64; 6] = [0.0, 0.5, 0.7, 0.9, 0.95, 0.99];
/// Below this many texts a cell gets no interval, as a suite below the
/// evaluation's floor gets none.
const FLOOR: usize = evaluation::interval::MIN_QUERIES;

/// A language as the score compares it: an individual language folded
/// into its macrolanguage (`cmn` is `zho`), since PubMed's indexers tag
/// macrolanguages.
fn fold(code: &str) -> &str {
    use super::macrolanguages::MACROLANGUAGES;
    match MACROLANGUAGES.binary_search_by(|(individual, _)| (*individual).cmp(code)) {
        Ok(at) => MACROLANGUAGES[at].1,
        Err(_) => code,
    }
}

/// One text of the key.
#[derive(Debug)]
struct Row {
    id: String,
    kind: String,
    language: String,
    tagged: String,
    script: String,
    length: String,
    chars: usize,
    text: String,
    /// A title's own work's abstract; empty for anything else.
    context: String,
}

impl Row {
    fn labelled(&self) -> bool {
        self.kind != Kind::CrossrefTitle.name()
    }

    fn title(&self) -> bool {
        self.kind.ends_with("title")
    }
}

/// One detector's answer for a text: its language, folded, and its
/// confidence.
type Said = (String, f64);

/// One detector's answers, and what its run said of itself.
#[derive(Debug)]
struct Answers {
    name: String,
    about: serde_json::Value,
    by_id: std::collections::HashMap<String, Said>,
    /// Its answers for the titles' abstracts, by the title's id.
    context: std::collections::HashMap<String, Said>,
}

impl Answers {
    /// The folded answer for a row.
    fn get(&self, row: &Row) -> (&str, f64) {
        self.by_id
            .get(&row.id)
            .map_or(("-", 0.0), |(label, confidence)| (fold(label), *confidence))
    }

    /// The folded answer for a title's abstract, if it has one.
    fn get_context(&self, row: &Row) -> Option<(&str, f64)> {
        self.context
            .get(&row.id)
            .map(|(label, confidence)| (fold(label), *confidence))
    }

    fn english(&self, row: &Row, t: f64) -> bool {
        let (label, confidence) = self.get(row);
        label == "eng" && confidence >= t
    }

    /// A title's language under `rule`: its own answer, or its abstract's
    /// where the rule says the title alone is not enough.
    fn title_language(&self, row: &Row, rule: Borrow) -> (&str, f64) {
        let own = self.get(row);
        if !row.title() {
            return own;
        }
        let Some(abstract_says) = self.get_context(row) else {
            return own;
        };
        let short = row.chars <= rule.short;
        let unsure = own.1 < rule.unsure;
        if (short || unsure) && abstract_says.1 >= BORROW_CONFIDENCE {
            abstract_says
        } else {
            own
        }
    }
}

/// When a title takes its abstract's language: when it is `short`
/// characters or fewer, or its own answer is below `unsure`.
#[derive(Debug, Clone, Copy)]
struct Borrow {
    name: &'static str,
    short: usize,
    unsure: f64,
}

/// The rules tried. The abstract's own answer must be at least
/// [`BORROW_CONFIDENCE`] to be borrowed.
const BORROWS: [Borrow; 4] = [
    Borrow {
        name: "title of 20 characters or fewer",
        short: 20,
        unsure: 0.0,
    },
    Borrow {
        name: "title of 50 characters or fewer",
        short: 50,
        unsure: 0.0,
    },
    Borrow {
        name: "title's answer below 0.5",
        short: 0,
        unsure: 0.5,
    },
    Borrow {
        name: "50 characters or fewer, or below 0.5",
        short: 50,
        unsure: 0.5,
    },
];

/// An abstract's answer a title borrows only at this confidence or above.
const BORROW_CONFIDENCE: f64 = 0.5;

fn read_rows(dir: &Path) -> Result<Vec<Row>, String> {
    let path = dir.join(KEY);
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut rows = Vec::new();
    for (n, line) in text.lines().enumerate().skip(1) {
        let c: Vec<&str> = line.split('\t').collect();
        let [
            id,
            kind,
            language,
            tagged,
            script,
            length,
            chars,
            text,
            context,
        ] = c.as_slice()
        else {
            return Err(format!(
                "{}: line {}: not nine columns (a key drawn before the context column: draw a new one)",
                path.display(),
                n + 1
            ));
        };
        rows.push(Row {
            id: (*id).to_owned(),
            kind: (*kind).to_owned(),
            language: fold(language).to_owned(),
            tagged: (*tagged).to_owned(),
            script: (*script).to_owned(),
            length: (*length).to_owned(),
            chars: chars
                .parse()
                .map_err(|e| format!("{}: line {}: {e}", path.display(), n + 1))?,
            text: (*text).to_owned(),
            context: (*context).to_owned(),
        });
    }
    Ok(rows)
}

/// Each labelled stratum's weight: its population over what the key kept
/// of it, so a share can be read as PubMed's rather than the key's.
fn weights(dir: &Path) -> Result<std::collections::HashMap<(String, String, String), f64>, String> {
    let path = dir.join(ABOUT);
    let about: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?,
    )
    .map_err(|e| format!("{}: {e}", path.display()))?;
    let strata = about["strata"]
        .as_array()
        .ok_or_else(|| format!("{}: no strata", path.display()))?;
    let mut out = std::collections::HashMap::new();
    for s in strata {
        let (Some(kind), Some(label), Some(length), Some(population), Some(kept)) = (
            s["kind"].as_str(),
            s["label"].as_str(),
            s["length"].as_str(),
            s["population"].as_u64(),
            s["kept"].as_u64(),
        ) else {
            return Err(format!("{}: a stratum without its counts", path.display()));
        };
        #[allow(clippy::cast_precision_loss)]
        let weight = population as f64 / kept.max(1) as f64;
        out.insert(
            (kind.to_owned(), fold(label).to_owned(), length.to_owned()),
            weight,
        );
    }
    Ok(out)
}

fn read_answers(path: &Path, key_digest: &str) -> Result<Answers, String> {
    let about_path = std::path::PathBuf::from(format!("{}.about", path.display()));
    let about: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(&about_path)
            .map_err(|e| format!("{}: {e}", about_path.display()))?,
    )
    .map_err(|e| format!("{}: {e}", about_path.display()))?;
    if about["key_blake3"].as_str() != Some(key_digest) {
        return Err(format!(
            "{}: answers to another key than this one",
            path.display()
        ));
    }
    let name = about["detector"]
        .as_str()
        .ok_or_else(|| format!("{}: no detector named", about_path.display()))?
        .to_owned();
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut by_id = std::collections::HashMap::new();
    let mut context = std::collections::HashMap::new();
    let number = |s: &str, n: usize| -> Result<f64, String> {
        s.parse()
            .map_err(|e| format!("{}: line {}: {e}", path.display(), n + 1))
    };
    for (n, line) in text.lines().enumerate().skip(1) {
        let c: Vec<&str> = line.split('\t').collect();
        let [id, label, confidence, _, context_label, context_confidence] = c.as_slice() else {
            return Err(format!(
                "{}: line {}: not six columns (answers from before the context column)",
                path.display(),
                n + 1
            ));
        };
        by_id.insert(
            (*id).to_owned(),
            ((*label).to_owned(), number(confidence, n)?),
        );
        if !context_label.is_empty() {
            context.insert(
                (*id).to_owned(),
                ((*context_label).to_owned(), number(context_confidence, n)?),
            );
        }
    }
    Ok(Answers {
        name,
        about,
        by_id,
        context,
    })
}

/// A share of texts with its interval: `values` are 0 and 1.
fn cell(values: &[f64]) -> String {
    let n = values.len();
    if n == 0 {
        return "-".to_owned();
    }
    #[allow(clippy::cast_precision_loss)]
    let mean = values.iter().sum::<f64>() / n as f64;
    if n < FLOOR {
        return format!("{mean:.3} (n {n}, too few for an interval)");
    }
    let [interval] = evaluation::interval::intervals(&[values.to_vec()], &vec![0; n])[..] else {
        return format!("{mean:.3} (n {n})");
    };
    format!(
        "{mean:.3} ({:.3}–{:.3}, n {n})",
        interval.low, interval.high
    )
}

fn ones(it: impl Iterator<Item = bool>) -> Vec<f64> {
    it.map(|b| if b { 1.0 } else { 0.0 }).collect()
}

/// `evaluate language-score <key dir> <answers>... [--out <hand-check.tsv>]`.
pub(crate) fn score(opts: &Options) -> ExitCode {
    let what = "evaluate language-score";
    let [key, answers @ ..] = opts.paths.as_slice() else {
        return super::usage();
    };
    if answers.is_empty()
        || opts.limit.is_some()
        || opts.workers.is_some()
        || opts.store.is_some()
        || opts.mailto.is_some()
        || opts.seed.is_some()
        || opts.by.is_some()
        || opts.related.is_some()
        || opts.walk.is_some()
        || opts.depth.is_some()
        || opts.survey
    {
        return super::usage();
    }
    match score_of(key, answers, opts.out.as_deref()) {
        Ok(report) => {
            let mut stdout = io::stdout().lock();
            match stdout
                .write_all(report.as_bytes())
                .and_then(|()| stdout.flush())
            {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("stdout: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        Err(e) => {
            eprintln!("{what}: {e}");
            ExitCode::FAILURE
        }
    }
}

#[allow(clippy::too_many_lines)]
fn score_of(
    key: &Path,
    answer_paths: &[std::path::PathBuf],
    hand_check: Option<&Path>,
) -> Result<String, String> {
    let key_file = key.join(KEY);
    let key_digest = {
        let bytes = std::fs::read(&key_file).map_err(|e| format!("{}: {e}", key_file.display()))?;
        blake3::hash(&bytes).to_hex().to_string()
    };
    let rows = read_rows(key)?;
    let weights = weights(key)?;
    let detectors = answer_paths
        .iter()
        .map(|p| read_answers(p, &key_digest))
        .collect::<Result<Vec<_>, _>>()?;
    for d in &detectors {
        if let Some(row) = rows.iter().find(|r| !d.by_id.contains_key(&r.id)) {
            return Err(format!("{} has no answer for {}", d.name, row.id));
        }
    }
    let labelled: Vec<&Row> = rows.iter().filter(|r| r.labelled()).collect();
    let english: Vec<&Row> = labelled
        .iter()
        .copied()
        .filter(|r| r.language == "eng")
        .collect();
    let other: Vec<&Row> = labelled
        .iter()
        .copied()
        .filter(|r| r.language != "eng")
        .collect();
    let weight = |r: &Row| {
        weights
            .get(&(r.kind.clone(), r.language.clone(), r.length.clone()))
            .copied()
            .unwrap_or(1.0)
    };

    let mut out = String::new();
    let _ = writeln!(
        out,
        "evaluate language-score: {} ({} labelled texts, {} of them English; {} Crossref titles unlabelled)",
        key.display(),
        labelled.len(),
        english.len(),
        rows.len() - labelled.len()
    );
    let _ = writeln!(
        out,
        "  languages folded to ISO 639-3 macrolanguages (SIL's mappings); a share's interval is a 95% bootstrap over texts, none under {FLOOR}"
    );
    for d in &detectors {
        let a = &d.about;
        let _ = writeln!(
            out,
            "\n== {}: {:.0} texts a second on {} threads, loaded in {:.1} s, peak {}; model {}",
            d.name,
            a["texts_per_second"].as_f64().unwrap_or(0.0),
            a["workers"],
            a["load_seconds"].as_f64().unwrap_or(0.0),
            a["peak_rss_kib"].as_u64().map_or_else(
                || "not measured".to_owned(),
                |k| format!("{} MiB", k / 1024)
            ),
            a["model"]["blake3"].as_str().unwrap_or("built in")
        );

        let _ = writeln!(
            out,
            "English texts called English (top answer, any confidence), by kind and length:"
        );
        let mut cells: BTreeMap<(&str, &str), Vec<&Row>> = BTreeMap::new();
        for r in &english {
            cells
                .entry((r.kind.as_str(), r.length.as_str()))
                .or_default()
                .push(r);
        }
        for ((kind, length), rs) in &cells {
            let _ = writeln!(
                out,
                "  {kind:<18} {length:<8} {}",
                cell(&ones(rs.iter().map(|r| d.english(r, 0.0))))
            );
        }

        let _ = writeln!(
            out,
            "By language and the text's script (NLM writes some original titles in Latin letters), every labelled text: answered right; called English; the commonest wrong answer:"
        );
        let mut by_language: BTreeMap<(&str, &str), Vec<&Row>> = BTreeMap::new();
        for r in &labelled {
            by_language
                .entry((r.language.as_str(), r.script.as_str()))
                .or_default()
                .push(r);
        }
        let mut languages: Vec<_> = by_language.into_iter().collect();
        languages.sort_by(|(la, a), (lb, b)| b.len().cmp(&a.len()).then(la.cmp(lb)));
        for ((language, script), rs) in &languages {
            let language = &format!("{language}/{script}");
            let english_texts = language.starts_with("eng/");
            let right = ones(rs.iter().map(|r| d.get(r).0 == r.language));
            let called = ones(rs.iter().map(|r| d.english(r, 0.0)));
            let mut wrong: BTreeMap<&str, usize> = BTreeMap::new();
            for r in rs {
                let (label, _) = d.get(r);
                if label != r.language {
                    *wrong.entry(label).or_default() += 1;
                }
            }
            let commonest = wrong
                .iter()
                .max_by_key(|(label, n)| (**n, std::cmp::Reverse(**label)))
                .map_or_else(|| "-".to_owned(), |(label, n)| format!("{label} ×{n}"));
            let _ = writeln!(
                out,
                "  {language:<16} right {}; English {}; {commonest}",
                cell(&right),
                if english_texts {
                    "-".to_owned()
                } else {
                    cell(&called)
                }
            );
        }

        let _ = writeln!(
            out,
            "Calibration, every labelled text: confidence, answered right:"
        );
        for (i, low) in BINS.iter().enumerate() {
            let high = BINS.get(i + 1).copied().unwrap_or(f64::INFINITY);
            let rs: Vec<&&Row> = labelled
                .iter()
                .filter(|r| {
                    let c = d.get(r).1;
                    c >= *low && c < high
                })
                .collect();
            let _ = writeln!(
                out,
                "  {low:.2}{}  {}",
                if high.is_finite() {
                    format!("–{high:.2}")
                } else {
                    "+    ".to_owned()
                },
                cell(&ones(rs.iter().map(|r| d.get(r).0 == r.language)))
            );
        }
        english_at(
            &mut out,
            "English at a confidence",
            &english,
            &other,
            &weight,
            &|r, t| d.english(r, t),
            &THRESHOLDS,
        );
        english_at(
            &mut out,
            "English unless another language at a confidence",
            &english,
            &other,
            &weight,
            &|r, t| {
                let (label, confidence) = d.get(r);
                label == "eng" || confidence < t
            },
            &THRESHOLDS[1..],
        );
        borrowing(&mut out, d, &labelled);
        misses(&mut out, d, &labelled);
    }

    if let [first, second, ..] = detectors.as_slice() {
        let _ = writeln!(
            out,
            "\n== {} and {} both English, each at the same confidence:",
            first.name, second.name
        );
        english_at(
            &mut out,
            "English at a confidence",
            &english,
            &other,
            &weight,
            &|r, t| first.english(r, t) && second.english(r, t),
            &THRESHOLDS,
        );
    }

    let crossref: Vec<&Row> = rows.iter().filter(|r| !r.labelled()).collect();
    if !crossref.is_empty() {
        let _ = writeln!(
            out,
            "\n== Crossref titles, unlabelled, by script: each detector's commonest answers"
        );
        let mut by_script: BTreeMap<&str, Vec<&Row>> = BTreeMap::new();
        for r in &crossref {
            by_script.entry(r.script.as_str()).or_default().push(r);
        }
        for (script, rs) in &by_script {
            let _ = write!(out, "  {script:<12} n {:<5}", rs.len());
            for d in &detectors {
                let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
                for r in rs {
                    *counts.entry(d.get(r).0).or_default() += 1;
                }
                let mut counts: Vec<_> = counts.into_iter().collect();
                counts.sort_by(|(la, a), (lb, b)| b.cmp(a).then(la.cmp(lb)));
                let top: Vec<String> = counts
                    .iter()
                    .take(3)
                    .map(|(l, n)| format!("{l} {n}"))
                    .collect();
                let _ = write!(out, "  {}: {}", d.name, top.join(", "));
            }
            if let [first, second, ..] = detectors.as_slice() {
                let agree = rs
                    .iter()
                    .filter(|r| first.get(r).0 == second.get(r).0)
                    .count();
                let _ = write!(out, "  agree {agree}");
            }
            let _ = writeln!(out);
        }
    }

    if let Some(path) = hand_check {
        // Each detector's answer for the title and for its abstract, and
        // the first detector's under the widest borrowing rule; a `person`
        // column to fill; the title; the abstract's first characters.
        const CONTEXT_SHOWN: usize = 300;
        let rule = BORROWS[BORROWS.len() - 1];
        let mut text = String::from("id\tscript\ttagged");
        for d in &detectors {
            let _ = write!(text, "\t{0}\t{0}:abstract", d.name);
        }
        if let Some(first) = detectors.first() {
            let _ = write!(text, "\t{}:borrowing", first.name);
        }
        text.push_str("\tperson\ttext\tabstract\n");
        for r in &crossref {
            let _ = write!(text, "{}\t{}\t{}", r.id, r.script, r.tagged);
            for d in &detectors {
                let _ = write!(
                    text,
                    "\t{}\t{}",
                    d.get(r).0,
                    d.get_context(r).map_or("", |c| c.0)
                );
            }
            if let Some(first) = detectors.first() {
                let _ = write!(text, "\t{}", first.title_language(r, rule).0);
            }
            let context: String = r.context.chars().take(CONTEXT_SHOWN).collect();
            let _ = writeln!(text, "\t\t{}\t{context}", r.text);
        }
        if path.exists() {
            return Err(format!("{} exists; name a new one", path.display()));
        }
        std::fs::write(path, text).map_err(|e| format!("{}: {e}", path.display()))?;
        let _ = writeln!(
            out,
            "\nhand-check sheet (a `person` column to fill): {}",
            path.display()
        );
    }
    Ok(out)
}

/// A share weighted back to PubMed's populations: each text counts as
/// many as its stratum's population over what the key kept of it. No
/// interval: it is a ratio (§7 item 9).
fn weighted(rows: &[&Row], weight: &dyn Fn(&Row) -> f64, yes: &dyn Fn(&Row) -> bool) -> f64 {
    let (mut hit, mut all) = (0.0, 0.0);
    for r in rows {
        all += weight(r);
        if yes(r) {
            hit += weight(r);
        }
    }
    if all > 0.0 { hit / all } else { 0.0 }
}

/// English calls at each threshold: how many English titles and abstracts
/// are called English — as the key drew them, each length alike, and
/// weighted to how often each length occurs — how many texts in other
/// languages are too, and the precision of a call as PubMed's populations
/// would make it.
fn english_at(
    out: &mut String,
    heading: &str,
    english: &[&Row],
    other: &[&Row],
    weight: &dyn Fn(&Row) -> f64,
    calls: &dyn Fn(&Row, f64) -> bool,
    thresholds: &[f64],
) {
    let _ = writeln!(
        out,
        "{heading}: English titles called English (weighted by length's share, no interval); English abstracts (weighted); other languages' texts called English; PubMed-weighted precision (no interval)"
    );
    let titles: Vec<&Row> = english.iter().copied().filter(|r| r.title()).collect();
    let abstracts: Vec<&Row> = english.iter().copied().filter(|r| !r.title()).collect();
    for &t in thresholds {
        let title_calls = ones(titles.iter().map(|r| calls(r, t)));
        let abstract_calls = ones(abstracts.iter().map(|r| calls(r, t)));
        let false_english = ones(other.iter().map(|r| calls(r, t)));
        let (mut right, mut called) = (0.0, 0.0);
        for r in english.iter().chain(other) {
            if calls(r, t) {
                called += weight(r);
                if r.language == "eng" {
                    right += weight(r);
                }
            }
        }
        let precision = if called > 0.0 { right / called } else { 0.0 };
        let _ = writeln!(
            out,
            "  ≥{t:.2}  titles {} ({:.3}); abstracts {} ({:.3}); others {}; precision {precision:.4}",
            cell(&title_calls),
            weighted(&titles, weight, &|r| calls(r, t)),
            cell(&abstract_calls),
            weighted(&abstracts, weight, &|r| calls(r, t)),
            cell(&false_english)
        );
    }
}

/// Whether a title borrowing its abstract's language helps: for each rule,
/// English titles called English, titles in another language called
/// English (the abstract beside them most often English), titles in
/// another language answered right, and how many titles the rule changed.
fn borrowing(out: &mut String, d: &Answers, labelled: &[&Row]) {
    let titles: Vec<&Row> = labelled
        .iter()
        .copied()
        .filter(|r| r.title() && !r.context.is_empty())
        .collect();
    let english: Vec<&Row> = titles
        .iter()
        .copied()
        .filter(|r| r.language == "eng")
        .collect();
    let other: Vec<&Row> = titles
        .iter()
        .copied()
        .filter(|r| r.language != "eng")
        .collect();
    let _ = writeln!(
        out,
        "A title taking its abstract's language (the abstract's answer at {BORROW_CONFIDENCE} or above), over the {} labelled titles with an abstract beside them: English titles called English; other titles called English; other titles answered right; titles changed",
        titles.len()
    );
    let none = Borrow {
        name: "the title alone",
        short: 0,
        unsure: 0.0,
    };
    for rule in std::iter::once(none).chain(BORROWS) {
        let called = |r: &Row| d.title_language(r, rule).0 == "eng";
        let changed = titles
            .iter()
            .filter(|r| d.title_language(r, rule).0 != d.get(r).0)
            .count();
        let _ = writeln!(
            out,
            "  {:<38} English {}; others English {}; others right {}; changed {changed}",
            rule.name,
            cell(&ones(english.iter().map(|r| called(r)))),
            cell(&ones(other.iter().map(|r| called(r)))),
            cell(&ones(
                other
                    .iter()
                    .map(|r| d.title_language(r, rule).0 == r.language)
            ))
        );
    }
}

/// Texts to read: English titles a detector called something else, a few
/// of each length, and texts in other languages it called English, one of
/// a language at a time.
fn misses(out: &mut String, d: &Answers, labelled: &[&Row]) {
    const PER_LENGTH: usize = 5;
    const OTHERS: usize = 15;
    const SHOWN: usize = 100;
    let show = |r: &Row| -> String {
        let (label, confidence) = d.get(r);
        let cut: String = r.text.chars().take(SHOWN).collect();
        format!("{label} {confidence:.2}  {}  `{cut}`", r.id)
    };
    let _ = writeln!(
        out,
        "To read — English titles called something else ({PER_LENGTH} of each length):"
    );
    for (_, length) in LENGTHS {
        for r in labelled
            .iter()
            .filter(|r| r.title() && r.language == "eng" && r.length == length)
            .filter(|r| d.get(r).0 != "eng")
            .take(PER_LENGTH)
        {
            let _ = writeln!(out, "  {length:<8} {}", show(r));
        }
    }
    let _ = writeln!(
        out,
        "To read — texts in another language called English (one a language, {OTHERS} at most):"
    );
    let mut seen = std::collections::HashSet::new();
    for r in labelled
        .iter()
        .filter(|r| r.language != "eng" && d.get(r).0 == "eng")
    {
        if seen.len() == OTHERS {
            break;
        }
        if seen.insert(r.language.as_str()) {
            let _ = writeln!(out, "  {:<8} {}", r.language, show(r));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stratum_keeps_the_smallest_draws_whatever_the_order() {
        let offer = |order: &[u32]| {
            let mut s = Sampler::default();
            for n in order {
                s.offer(
                    Kind::Title,
                    format!("pmid:{n}/title"),
                    Some("eng".to_owned()),
                    "eng".to_owned(),
                    "A title".to_owned(),
                    None,
                );
            }
            s
        };
        let many: Vec<u32> = (1..=u32::try_from(CAP).unwrap() + 50).collect();
        let reversed: Vec<u32> = many.iter().rev().copied().collect();
        let (a, b) = (offer(&many), offer(&reversed));
        let ids = |s: &Sampler| {
            let mut v: Vec<String> = s
                .strata
                .values()
                .flat_map(|st| st.kept.iter().map(|k| k.id.clone()))
                .collect();
            v.sort();
            v
        };
        assert_eq!(ids(&a), ids(&b));
        let stratum = a.strata.values().next().unwrap();
        assert_eq!(
            (stratum.population, stratum.kept.len()),
            (many.len() as u64, CAP)
        );
    }

    #[test]
    fn texts_are_filed_by_language_or_by_script_and_length() {
        let mut s = Sampler::default();
        s.offer(
            Kind::VernacularTitle,
            "a".into(),
            Some("zho".into()),
            "zho".into(),
            "肺癌基因治疗".into(),
            Some(Context::Plain("An abstract in English.")),
        );
        s.offer(
            Kind::CrossrefTitle,
            "b".into(),
            None,
            "-".into(),
            "Разработка метода".into(),
            None,
        );
        s.offer(
            Kind::CrossrefTitle,
            "c".into(),
            None,
            "-".into(),
            "  --  ".into(),
            None,
        );
        s.offer(
            Kind::CrossrefTitle,
            "d".into(),
            None,
            "-".into(),
            "Разработка метода".into(),
            Some(Context::Markup("<jats:p>Аннотация &amp; текст</jats:p>")),
        );
        s.offer(
            Kind::CrossrefTitle,
            "e".into(),
            None,
            "-".into(),
            "Разработка метода".into(),
            Some(Context::Markup("   ")),
        );
        let keys: Vec<_> = s
            .strata
            .keys()
            .map(|(k, l, n)| (k.name(), l.as_str(), *n))
            .collect();
        assert_eq!(
            keys,
            vec![
                ("vernacular-title", "zho", "1-20"),
                ("crossref-title", "Cyrillic", "1-20"),
                ("crossref-title", "Cyrillic+abstract", "1-20"),
            ]
        );
        let kept = |stratum: &str| {
            s.strata
                .iter()
                .find(|((_, l, _), _)| l == stratum)
                .map(|(_, st)| {
                    st.kept
                        .iter()
                        .map(|k| k.context.clone())
                        .collect::<Vec<_>>()
                })
                .unwrap()
        };
        assert_eq!(kept("zho"), vec!["An abstract in English."]);
        assert_eq!(kept("Cyrillic+abstract"), vec!["Аннотация & текст"]);
        // A blank abstract is no abstract: filed without, kept without.
        assert_eq!(kept("Cyrillic"), vec!["", ""]);
    }

    #[test]
    fn a_translated_title_loses_its_brackets() {
        let title = plain("[Diagnosis of <i>tuberculosis</i> in children].");
        let inner = title
            .strip_prefix('[')
            .and_then(|t| t.strip_suffix("].").or_else(|| t.strip_suffix(']')));
        assert_eq!(inner, Some("Diagnosis of tuberculosis in children"));
    }

    #[test]
    fn individual_languages_fold_into_their_macrolanguage() {
        assert_eq!(fold("cmn"), "zho");
        assert_eq!(fold("arb"), "ara");
        assert_eq!(fold("nob"), "nor");
        assert_eq!(fold("eng"), "eng");
        assert_eq!(fold("-"), "-");
    }

    /// The table is SIL's, sorted, so a binary search finds every entry.
    #[test]
    fn the_macrolanguage_table_is_sorted_and_whole() {
        use super::super::macrolanguages::MACROLANGUAGES;
        assert!(MACROLANGUAGES.windows(2).all(|w| w[0].0 < w[1].0));
        for (individual, macrolanguage) in MACROLANGUAGES {
            assert_eq!(fold(individual), macrolanguage);
        }
    }

    #[test]
    fn a_cell_gives_no_interval_under_the_floor() {
        assert_eq!(cell(&[]), "-");
        assert!(cell(&[1.0; 10]).contains("too few"));
        let many: Vec<f64> = (0..100).map(|i| f64::from(u8::from(i % 4 == 0))).collect();
        let text = cell(&many);
        assert!(
            text.starts_with("0.250 (") && text.contains("n 100"),
            "{text}"
        );
    }

    #[test]
    fn a_title_borrows_its_abstracts_language_only_as_the_rule_says() {
        let row = |id: &str, kind: &str, chars: usize| Row {
            id: id.to_owned(),
            kind: kind.to_owned(),
            language: "eng".to_owned(),
            tagged: "eng".to_owned(),
            script: "Latin".to_owned(),
            length: length(chars).to_owned(),
            chars,
            text: String::new(),
            context: "an abstract".to_owned(),
        };
        let said = |pairs: &[(&str, &str, f64)]| {
            pairs
                .iter()
                .map(|(id, l, c)| ((*id).to_owned(), ((*l).to_owned(), *c)))
                .collect()
        };
        let d = Answers {
            name: "test".to_owned(),
            about: serde_json::Value::Null,
            by_id: said(&[
                ("short", "nld", 0.9),
                ("long", "nld", 0.9),
                ("unsure", "nld", 0.3),
                ("doubtful-abstract", "nld", 0.3),
                ("abstract", "nld", 0.3),
            ]),
            context: said(&[
                ("short", "eng", 0.99),
                ("long", "eng", 0.99),
                ("unsure", "eng", 0.99),
                ("doubtful-abstract", "eng", 0.4),
                ("abstract", "eng", 0.99),
            ]),
        };
        let [short, _, unsure, either] = BORROWS;
        assert_eq!(d.title_language(&row("short", "title", 15), short).0, "eng");
        assert_eq!(d.title_language(&row("long", "title", 80), short).0, "nld");
        assert_eq!(d.title_language(&row("long", "title", 80), unsure).0, "nld");
        assert_eq!(
            d.title_language(&row("unsure", "title", 80), unsure).0,
            "eng"
        );
        assert_eq!(
            d.title_language(&row("unsure", "title", 80), either).0,
            "eng"
        );
        // An abstract the detector is unsure of lends nothing.
        assert_eq!(
            d.title_language(&row("doubtful-abstract", "title", 10), either)
                .0,
            "nld"
        );
        // Only titles borrow.
        assert_eq!(
            d.title_language(&row("abstract", "abstract", 10), either).0,
            "nld"
        );
    }
}
