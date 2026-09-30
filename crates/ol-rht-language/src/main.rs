//! `ol-rht-language`: language identification for the tokenizer plan's
//! D13 (`docs/tokenizer-plan.md`), measured before it chooses anything. It
//! runs one detector over the answer key `ol-rht evaluate language-key`
//! made and writes each text's answer; `ol-rht evaluate language-score`
//! scores them. The detectors live here alone, so that no other binary
//! carries their models (`docs/inbox/language-identification.md`).
//!
//! An answer is a language as ISO 639-3 writes it — the detector's own
//! code, an individual language where it names one (GlotLID's `cmn`), so
//! the score folds macrolanguages — or `-` where the detector names none;
//! its confidence, from 0 to 1; and the microseconds it took. A title's
//! work's abstract, when the key holds one beside it, is answered too, in
//! two more columns. Beside the answers, `<out>.about`: the detector, its
//! model's digest, its load time, its throughput (every text it answered,
//! the abstracts beside titles included) and its memory.

use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

const USAGE: &str = "usage:
  ol-rht-language detect <key.tsv> --detector <lingua | lingua-low | glotlid> --out <answers.tsv> [--model <file>] [--workers N]";

/// Which detector, and how it is built.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Which {
    /// lingua, every language it has, high-accuracy mode, models preloaded.
    Lingua,
    /// lingua's low-accuracy mode: fewer models, weaker under 120
    /// characters.
    LinguaLow,
    /// GlotLID, a fastText model read from `--model`.
    GlotLid,
}

impl Which {
    const fn name(self) -> &'static str {
        match self {
            Self::Lingua => "lingua",
            Self::LinguaLow => "lingua-low",
            Self::GlotLid => "glotlid",
        }
    }
}

#[derive(Debug)]
struct Args {
    key: PathBuf,
    which: Which,
    out: PathBuf,
    model: Option<PathBuf>,
    workers: usize,
}

fn parse(args: &[String]) -> Option<Args> {
    let [command, rest @ ..] = args else {
        return None;
    };
    if command != "detect" {
        return None;
    }
    let (mut key, mut which, mut out, mut model, mut workers) = (None, None, None, None, None);
    let mut rest = rest.iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--detector" => {
                which = Some(match rest.next()?.as_str() {
                    "lingua" => Which::Lingua,
                    "lingua-low" => Which::LinguaLow,
                    "glotlid" => Which::GlotLid,
                    _ => return None,
                });
            }
            "--out" => out = Some(PathBuf::from(rest.next()?)),
            "--model" => model = Some(PathBuf::from(rest.next()?)),
            "--workers" => workers = Some(rest.next()?.parse::<usize>().ok().filter(|n| *n > 0)?),
            flag if flag.starts_with("--") => return None,
            path if key.is_none() => key = Some(PathBuf::from(path)),
            _ => return None,
        }
    }
    let which = which?;
    // A model file is GlotLID's and nobody else's.
    if (which == Which::GlotLid) != model.is_some() {
        return None;
    }
    Some(Args {
        key: key?,
        which,
        out: out?,
        model,
        workers: workers.unwrap_or_else(|| {
            std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get)
        }),
    })
}

/// One detector: a text's language and the detector's confidence in it.
trait Detect: Sync {
    fn detect(&self, text: &str) -> Result<(String, f64), String>;
}

struct Lingua(lingua::LanguageDetector);

impl Detect for Lingua {
    fn detect(&self, text: &str) -> Result<(String, f64), String> {
        let values = self.0.compute_language_confidence_values(text);
        Ok(match values.first() {
            Some((language, confidence)) if *confidence > 0.0 => {
                (language.iso_code_639_3().to_string(), *confidence)
            }
            _ => ("-".to_owned(), 0.0),
        })
    }
}

struct GlotLid(fasttext_pure_rs::FastText);

impl Detect for GlotLid {
    fn detect(&self, text: &str) -> Result<(String, f64), String> {
        let predictions = self.0.predict(text, 1, 0.0).map_err(|e| e.to_string())?;
        Ok(match predictions.first() {
            Some(p) => (label_of(&p.label), f64::from(p.probability)),
            None => ("-".to_owned(), 0.0),
        })
    }
}

/// A fastText label as ISO 639-3: `__label__eng_Latn` is `eng`; lid.176's
/// two-letter `__label__en` is `eng` too, through ISO 639-1.
fn label_of(label: &str) -> String {
    let bare = label.strip_prefix("__label__").unwrap_or(label);
    let code = bare.split('_').next().unwrap_or(bare).to_ascii_lowercase();
    if code.len() == 2 {
        return model::Lang::from_alpha_2(&code).map_or(code, |l| l.to_string());
    }
    code
}

fn build(args: &Args) -> Result<Box<dyn Detect>, String> {
    use lingua::LanguageDetectorBuilder;
    Ok(match args.which {
        Which::Lingua => Box::new(Lingua(
            LanguageDetectorBuilder::from_all_languages()
                .with_preloaded_language_models()
                .build(),
        )),
        Which::LinguaLow => Box::new(Lingua(
            LanguageDetectorBuilder::from_all_languages()
                .with_low_accuracy_mode()
                .with_preloaded_language_models()
                .build(),
        )),
        Which::GlotLid => {
            let path = args.model.as_deref().ok_or("glotlid needs --model")?;
            Box::new(GlotLid(
                fasttext_pure_rs::FastText::load(path)
                    .map_err(|e| format!("{}: {e}", path.display()))?,
            ))
        }
    })
}

/// One text of the key: its id, the text, and — for a title — its work's
/// abstract, empty otherwise.
struct Row {
    id: String,
    text: String,
    context: String,
}

/// The key's rows, in the key's order.
fn read_key(path: &Path) -> Result<Vec<Row>, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut rows = Vec::new();
    for (n, line) in io::BufReader::new(file).lines().enumerate() {
        let line = line.map_err(|e| format!("{}: {e}", path.display()))?;
        if n == 0 {
            if !line.starts_with("id\t") {
                return Err(format!("{}: not a language key", path.display()));
            }
            continue;
        }
        let columns: Vec<&str> = line.split('\t').collect();
        let [id, _, _, _, _, _, _, text, context] = columns.as_slice() else {
            return Err(format!(
                "{}: line {}: not nine columns (a key drawn before the context column)",
                path.display(),
                n + 1
            ));
        };
        rows.push(Row {
            id: (*id).to_owned(),
            text: (*text).to_owned(),
            context: (*context).to_owned(),
        });
    }
    Ok(rows)
}

/// A text's answer: its language, the detector's confidence, the
/// microseconds the text took, and the answer for its abstract when it has
/// one beside it.
type Answer = (String, f64, u64, Option<(String, f64)>);

/// Every text on `workers` threads, each taking the next as it finishes
/// one; the answers in the key's order.
fn detect_all(detector: &dyn Detect, rows: &[Row], workers: usize) -> Result<Vec<Answer>, String> {
    let next = AtomicUsize::new(0);
    let mut answered: Vec<(usize, Result<Answer, String>)> = std::thread::scope(|scope| {
        let threads: Vec<_> = (0..workers)
            .map(|_| {
                scope.spawn(|| {
                    let mut mine = Vec::new();
                    loop {
                        let i = next.fetch_add(1, Ordering::Relaxed);
                        let Some(row) = rows.get(i) else {
                            break mine;
                        };
                        let answer = (|| {
                            let started = Instant::now();
                            let (label, confidence) = detector.detect(&row.text)?;
                            let micros =
                                u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX);
                            let context = if row.context.is_empty() {
                                None
                            } else {
                                Some(detector.detect(&row.context)?)
                            };
                            Ok((label, confidence, micros, context))
                        })()
                        .map_err(|e: String| format!("{}: {e}", row.id));
                        mine.push((i, answer));
                    }
                })
            })
            .collect();
        threads
            .into_iter()
            .flat_map(|t| t.join().expect("a detecting thread panicked"))
            .collect()
    });
    answered.sort_unstable_by_key(|(i, _)| *i);
    answered.into_iter().map(|(_, a)| a).collect()
}

/// The process's peak resident memory, where the system says (Linux).
fn peak_rss_kib() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    status
        .lines()
        .find_map(|l| l.strip_prefix("VmHWM:"))
        .and_then(|v| v.trim().trim_end_matches("kB").trim().parse().ok())
}

fn digest(path: &Path) -> Result<String, String> {
    let mut hasher = blake3::Hasher::new();
    let mut file = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    io::copy(&mut file, &mut hasher).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(hasher.finalize().to_hex().to_string())
}

fn run(args: &Args) -> Result<String, String> {
    let about_path = PathBuf::from(format!("{}.about", args.out.display()));
    for path in [&args.out, &about_path] {
        if path.exists() {
            return Err(format!(
                "{} exists; answers are kept, so name new ones",
                path.display()
            ));
        }
    }
    let rows = read_key(&args.key)?;
    let started = Instant::now();
    let detector = build(args)?;
    let loaded = started.elapsed();
    let detecting = Instant::now();
    let answers = detect_all(detector.as_ref(), &rows, args.workers)?;
    let elapsed = detecting.elapsed();

    let partial = PathBuf::from(format!("{}.partial", args.out.display()));
    let written = (|| -> io::Result<()> {
        let mut file = io::BufWriter::new(std::fs::File::create(&partial)?);
        writeln!(
            file,
            "id\tlabel\tconfidence\tmicros\tcontext_label\tcontext_confidence"
        )?;
        for (row, (label, confidence, micros, context)) in rows.iter().zip(&answers) {
            let (context_label, context_confidence) = match context {
                Some((l, c)) => (l.as_str(), format!("{c:.6}")),
                None => ("", String::new()),
            };
            writeln!(
                file,
                "{}\t{label}\t{confidence:.6}\t{micros}\t{context_label}\t{context_confidence}",
                row.id
            )?;
        }
        file.into_inner()
            .map_err(io::IntoInnerError::into_error)?
            .sync_all()?;
        std::fs::rename(&partial, &args.out)
    })();
    if let Err(e) = written {
        let _ = std::fs::remove_file(&partial);
        return Err(format!("{}: {e}", args.out.display()));
    }
    let model = match &args.model {
        Some(path) => {
            serde_json::json!({ "path": path.display().to_string(), "blake3": digest(path)? })
        }
        None => serde_json::Value::Null,
    };
    // Every text detected, the abstracts beside titles included.
    let abstracts = answers.iter().filter(|a| a.3.is_some()).count();
    let detections = rows.len() + abstracts;
    let per_second = |d: Duration| {
        #[allow(clippy::cast_precision_loss)]
        let n = detections as f64;
        n / d.as_secs_f64().max(f64::MIN_POSITIVE)
    };
    let about = serde_json::json!({
        "detector": args.which.name(),
        "model": model,
        "key": args.key.display().to_string(),
        "key_blake3": digest(&args.key)?,
        "texts": rows.len(),
        "abstracts_beside_titles": abstracts,
        "workers": args.workers,
        "load_seconds": loaded.as_secs_f64(),
        "detect_seconds": elapsed.as_secs_f64(),
        "texts_per_second": per_second(elapsed),
        "peak_rss_kib": peak_rss_kib(),
        "ol_rht_language": env!("CARGO_PKG_VERSION"),
    });
    let mut text = serde_json::to_string_pretty(&about).map_err(|e| e.to_string())?;
    text.push('\n');
    std::fs::write(&about_path, text).map_err(|e| format!("{}: {e}", about_path.display()))?;
    Ok(format!(
        "ol-rht-language detect: {} texts and {abstracts} abstracts beside titles by {} on {} threads in {:.1} s ({:.0} a second), loaded in {:.1} s; answers: {}\n",
        rows.len(),
        args.which.name(),
        args.workers,
        elapsed.as_secs_f64(),
        per_second(elapsed),
        loaded.as_secs_f64(),
        args.out.display()
    ))
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(args) = parse(&args) else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    match run(&args) {
        Ok(summary) => {
            print!("{summary}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("ol-rht-language detect: {e}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fasttext_labels_become_iso_639_3() {
        assert_eq!(label_of("__label__eng_Latn"), "eng");
        assert_eq!(label_of("__label__cmn_Hani"), "cmn");
        assert_eq!(label_of("__label__en"), "eng");
        assert_eq!(label_of("__label__de"), "deu");
    }

    #[test]
    fn a_model_file_goes_with_glotlid_alone() {
        let args = |s: &str| s.split(' ').map(str::to_owned).collect::<Vec<_>>();
        assert!(parse(&args("detect k.tsv --detector lingua --out a.tsv")).is_some());
        assert!(
            parse(&args(
                "detect k.tsv --detector lingua --out a.tsv --model m.bin"
            ))
            .is_none()
        );
        assert!(parse(&args("detect k.tsv --detector glotlid --out a.tsv")).is_none());
        assert!(
            parse(&args(
                "detect k.tsv --detector glotlid --out a.tsv --model m.bin --workers 4"
            ))
            .is_some()
        );
    }

    #[test]
    fn lingua_names_english() {
        let detector = Lingua(lingua::LanguageDetectorBuilder::from_all_languages().build());
        let (label, confidence) = detector
            .detect("Clinical features of patients with pneumonia in the intensive care unit")
            .unwrap();
        assert_eq!(label, "eng");
        assert!(confidence > 0.5, "{confidence}");
    }
}
