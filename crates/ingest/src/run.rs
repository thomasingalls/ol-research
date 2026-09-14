//! The runner: every record of every file through a [`Source`] and into a
//! [`Sink`], stopping at the first failure with enough context to reproduce
//! it (plan §6): the source, the file, the record's ordinal, the stage, and
//! the source's own error with its native id and field.

use std::error::Error;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::gz;
use crate::source::{Sink, Source};

/// Reads one raw file into memory: through the multi-member gzip decoder
/// when its extension is `gz`, as is otherwise.
///
/// # Errors
///
/// Any I/O or gzip error, or bytes that are not UTF-8.
pub fn read_raw(path: &Path) -> io::Result<String> {
    if path.extension().is_some_and(|e| e == "gz") {
        gz::read_to_string(path)
    } else {
        fs::read_to_string(path)
    }
}

/// The stage a run failed at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// Reading or inflating the file.
    Read,
    /// The strict native parse.
    Parse,
    /// The strict value-level normalization.
    Normalize,
}

impl fmt::Display for Stage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Read => "read",
            Self::Parse => "parse",
            Self::Normalize => "normalize",
        })
    }
}

/// Why a run stopped, and where.
#[derive(Debug)]
pub struct Failure {
    /// [`Source::NAME`].
    pub source: &'static str,
    /// The raw file.
    pub path: PathBuf,
    /// The record's ordinal in the file, from one; zero when the file
    /// itself could not be read.
    pub record: u64,
    #[allow(missing_docs)]
    pub stage: Stage,
    /// The source's error: what, and the native id where it was readable.
    pub error: Box<dyn Error + Send + Sync>,
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}: {}", self.source, self.stage, self.path.display())?;
        if self.record > 0 {
            write!(f, ": record {}", self.record)?;
        }
        write!(f, ": {}", self.error)
    }
}

impl Error for Failure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.error.as_ref())
    }
}

/// What a run did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Report {
    /// Files read.
    pub files: usize,
    /// Records parsed (and normalized, when normalizing).
    pub records: u64,
    /// Bytes of decompressed input.
    pub bytes: u64,
    /// Wall time for reading, parsing and normalizing.
    pub elapsed: Duration,
}

#[allow(clippy::cast_precision_loss)] // reporting only
impl Report {
    /// Records per second of wall time.
    #[must_use]
    pub fn records_per_second(&self) -> f64 {
        self.records as f64 / self.elapsed.as_secs_f64()
    }

    /// Decompressed mebibytes per second of wall time.
    #[must_use]
    pub fn mib_per_second(&self) -> f64 {
        self.bytes as f64 / (1024.0 * 1024.0) / self.elapsed.as_secs_f64()
    }
}

impl fmt::Display for Report {
    #[allow(clippy::cast_precision_loss)] // reporting only
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} records from {} files, {:.1} MiB in {:.2}s = {:.0} records/s, {:.0} MiB/s",
            self.records,
            self.files,
            self.bytes as f64 / (1024.0 * 1024.0),
            self.elapsed.as_secs_f64(),
            self.records_per_second(),
            self.mib_per_second()
        )
    }
}

/// Parses every record of every file and normalizes none of them: the
/// structural check alone. `limit` stops the run after that many records
/// across all files.
///
/// # Errors
///
/// The first file that cannot be read or record that does not parse.
pub fn parse<S: Source>(paths: &[impl AsRef<Path>], limit: Option<u64>) -> Result<Report, Failure> {
    each::<S, _>(paths, limit, |_| Ok(()))
}

/// Parses and normalizes every record of every file into `sink`. `limit`
/// stops the run after that many records across all files.
///
/// # Errors
///
/// The first file that cannot be read, record that does not parse, or value
/// that does not fit the model. The sink has received everything before it.
pub fn normalize<S, K>(
    paths: &[impl AsRef<Path>],
    limit: Option<u64>,
    sink: &mut K,
) -> Result<Report, Failure>
where
    S: Source,
    K: for<'n> Sink<'n, S>,
{
    each::<S, _>(paths, limit, |native| S::normalize(native, sink))
}

fn each<S: Source, F>(
    paths: &[impl AsRef<Path>],
    limit: Option<u64>,
    mut f: F,
) -> Result<Report, Failure>
where
    F: FnMut(&S::Native<'_>) -> Result<(), S::NormalizeError>,
{
    let started = Instant::now();
    let mut report = Report::default();
    'files: for path in paths {
        let path = path.as_ref();
        let input = read_raw(path).map_err(|e| Failure {
            source: S::NAME,
            path: path.to_path_buf(),
            record: 0,
            stage: Stage::Read,
            error: Box::new(e),
        })?;
        report.files += 1;
        report.bytes += input.len() as u64;
        for (index, record) in S::records(&input).enumerate() {
            if limit.is_some_and(|n| report.records >= n) {
                break 'files;
            }
            let fail = |stage: Stage, error: Box<dyn Error + Send + Sync>| Failure {
                source: S::NAME,
                path: path.to_path_buf(),
                record: index as u64 + 1,
                stage,
                error,
            };
            let native = record.map_err(|e| fail(Stage::Parse, Box::new(e)))?;
            f(&native).map_err(|e| fail(Stage::Normalize, Box::new(e)))?;
            report.records += 1;
        }
    }
    report.elapsed = started.elapsed();
    Ok(report)
}

#[cfg(test)]
mod tests {
    use std::fmt;
    use std::io::Write;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};

    use super::{Stage, normalize, parse};
    use crate::source::{Count, Sink, Source};

    /// A toy source: one record per line. `3: 4 5` is work 3 citing 4 and
    /// 5; `del 7` withdraws 7; a line starting with `#` fails to parse; a
    /// line that is not a number fails to normalize.
    struct Lines;

    #[derive(Debug)]
    struct BadLine(String);

    impl fmt::Display for BadLine {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "bad line {:?}", self.0)
        }
    }

    impl std::error::Error for BadLine {}

    impl Source for Lines {
        const NAME: &'static str = "lines";
        type Native<'a> = &'a str;
        type Id<'n> = u32;
        type Work<'n> = u32;
        type Reference<'n> = u32;
        type ParseError = BadLine;
        type NormalizeError = BadLine;

        fn records(input: &str) -> impl Iterator<Item = Result<&str, BadLine>> + '_ {
            input.lines().map(|line| {
                if line.starts_with('#') {
                    Err(BadLine(line.to_owned()))
                } else {
                    Ok(line)
                }
            })
        }

        fn normalize<'n, K: Sink<'n, Self>>(line: &'n &str, sink: &mut K) -> Result<(), BadLine> {
            let bad = || BadLine((*line).to_owned());
            if let Some(id) = line.strip_prefix("del ") {
                sink.deletion(&id.parse().map_err(|_| bad())?);
                return Ok(());
            }
            let (from, targets) = line.split_once(':').ok_or_else(bad)?;
            let from: u32 = from.parse().map_err(|_| bad())?;
            sink.work(&from);
            for to in targets.split_whitespace() {
                sink.reference(&from, &to.parse().map_err(|_| bad())?);
            }
            Ok(())
        }
    }

    static NEXT: AtomicU32 = AtomicU32::new(0);

    /// Writes `text` to a fresh file in the temp dir, gzipped when asked.
    fn file(text: &str, gz: bool) -> PathBuf {
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let name = format!(
            "ol-rht-ingest-{}-{n}.{}",
            std::process::id(),
            if gz { "txt.gz" } else { "txt" }
        );
        let path = std::env::temp_dir().join(name);
        let mut out = std::fs::File::create(&path).unwrap();
        if gz {
            let mut enc = flate2::write::GzEncoder::new(out, flate2::Compression::fast());
            enc.write_all(text.as_bytes()).unwrap();
            enc.finish().unwrap();
        } else {
            out.write_all(text.as_bytes()).unwrap();
        }
        path
    }

    #[test]
    fn counts_works_edges_and_deletions_across_plain_and_gzipped_files() {
        const PLAIN: &str = "1: 2 3\n2:\n";
        const GZ: &str = "3: 1\ndel 9\n";
        let plain = file(PLAIN, false);
        let gz = file(GZ, true);
        let mut count = Count::default();
        let report = normalize::<Lines, _>(&[&plain, &gz], None, &mut count).unwrap();
        assert_eq!(report.files, 2);
        assert_eq!(report.records, 4);
        assert_eq!(report.bytes, (PLAIN.len() + GZ.len()) as u64);
        assert_eq!(
            count,
            Count {
                works: 3,
                references: 3,
                deletions: 1
            }
        );
        assert!(report.to_string().contains("4 records from 2 files"));
        let report = parse::<Lines>(&[&plain, &gz], None).unwrap();
        assert_eq!(report.records, 4);
        std::fs::remove_file(plain).unwrap();
        std::fs::remove_file(gz).unwrap();
    }

    #[test]
    fn limit_stops_across_files_before_parsing_the_next_record() {
        let a = file("1: 2\n2: 3\n", false);
        let b = file("#never read\n", false);
        let mut count = Count::default();
        let report = normalize::<Lines, _>(&[&a, &b], Some(2), &mut count).unwrap();
        assert_eq!(report.records, 2);
        assert_eq!(
            report.files, 2,
            "the second file is opened, then the limit stops it"
        );
        assert_eq!(count.works, 2);
        let mut count = Count::default();
        let report = normalize::<Lines, _>(&[&a, &b], Some(1), &mut count).unwrap();
        assert_eq!((report.records, report.files), (1, 1));
        std::fs::remove_file(a).unwrap();
        std::fs::remove_file(b).unwrap();
    }

    #[test]
    fn failures_say_which_file_record_and_stage() {
        let path = file("1: 2\n#nope\n3: 4\n", false);
        let err = parse::<Lines>(&[&path], None).unwrap_err();
        assert_eq!((err.stage, err.record), (Stage::Parse, 2));
        assert_eq!(err.source, "lines");
        assert_eq!(err.path, path);
        assert!(
            err.to_string().contains("record 2: bad line \"#nope\""),
            "{err}"
        );
        std::fs::remove_file(&path).unwrap();

        let path = file("1: 2\n2: x\n", false);
        let mut count = Count::default();
        let err = normalize::<Lines, _>(&[&path], None, &mut count).unwrap_err();
        assert_eq!((err.stage, err.record), (Stage::Normalize, 2));
        assert_eq!(
            count.works, 2,
            "the sink had the work before its edge failed"
        );
        assert!(err.to_string().starts_with("lines normalize: "), "{err}");
        std::fs::remove_file(&path).unwrap();

        let missing = std::env::temp_dir().join("ol-rht-ingest-does-not-exist.txt");
        let err = parse::<Lines>(&[&missing], None).unwrap_err();
        assert_eq!((err.stage, err.record), (Stage::Read, 0));
        assert!(!err.to_string().contains("record"), "{err}");
    }
}
