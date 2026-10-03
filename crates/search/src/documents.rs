//! A list of works' documents, saved (tokenizer plan T1.3): a screening
//! set's, assembled from the corpus once — the slow part of a build — and
//! read back for every treatment's index, so that a variant costs its own
//! indexing and not another pass over the corpus.
//!
//! One zstd stream of lines, each JSON: a [`Header`], then a document a
//! line in the order the assembly gave them. A document's meaning is its
//! model's and its merge's, so a file of other versions is refused.

use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};

use merge::{Document, WorkId};
use serde::{Deserialize, Serialize};

use crate::error::Error;

/// The file's layout.
pub const FORMAT: u32 = 1;

/// zstd level, as the store's sets.
const LEVEL: i32 = 3;

/// What a file of documents says of itself, on its first line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Header {
    /// [`FORMAT`].
    pub documents_format: u32,
    #[allow(missing_docs)]
    pub model: u32,
    #[allow(missing_docs)]
    pub merge: u32,
    /// The journal batch the documents were assembled at: the batch of an
    /// index built from them.
    pub batch: u64,
    /// The list of works they are, as the assembly was told it.
    pub list: String,
    /// The list's file's BLAKE3, in hex.
    pub list_blake3: String,
    /// The documents that follow: one for every work the list names.
    pub works: u64,
}

fn io_at(path: &Path) -> impl Fn(io::Error) -> Error + '_ {
    |source| Error::Io {
        path: path.to_path_buf(),
        source,
    }
}

/// A file of documents being written: beside its place until every listed
/// work's document is in it, and only then in its place.
pub struct Writer {
    path: PathBuf,
    partial: PathBuf,
    out: zstd::stream::write::Encoder<'static, BufWriter<File>>,
    wanted: HashSet<WorkId>,
    written: u64,
}

impl std::fmt::Debug for Writer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Writer")
            .field("path", &self.path)
            .field("wanted", &self.wanted.len())
            .field("written", &self.written)
            .finish_non_exhaustive()
    }
}

impl Writer {
    /// A file at `path` for the documents of `wanted`, the works `list` —
    /// a file of that BLAKE3 — names, assembled at `batch`.
    ///
    /// # Errors
    ///
    /// I/O; a file already at `path` is one of them, since a set's
    /// documents are kept.
    pub fn create(
        path: &Path,
        list: String,
        list_blake3: String,
        batch: u64,
        wanted: HashSet<WorkId>,
    ) -> Result<Self, Error> {
        if path.exists() {
            return Err(io_at(path)(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "documents are kept, so name a new file",
            )));
        }
        let partial = PathBuf::from(format!("{}.partial", path.display()));
        let file = File::create(&partial).map_err(io_at(&partial))?;
        let mut out = zstd::stream::write::Encoder::new(BufWriter::new(file), LEVEL)
            .map_err(io_at(&partial))?;
        let header = Header {
            documents_format: FORMAT,
            model: model::VERSION,
            merge: merge::VERSION,
            batch,
            list,
            list_blake3,
            works: wanted.len() as u64,
        };
        line(&mut out, &header).map_err(io_at(&partial))?;
        Ok(Self {
            path: path.to_path_buf(),
            partial,
            out,
            wanted,
            written: 0,
        })
    }

    /// Saves those of `documents` the list names.
    ///
    /// # Errors
    ///
    /// I/O.
    pub fn add(&mut self, documents: &[Document]) -> Result<(), Error> {
        for d in documents.iter().filter(|d| self.wanted.contains(&d.id)) {
            line(&mut self.out, d).map_err(io_at(&self.partial))?;
            self.written += 1;
        }
        Ok(())
    }

    /// Puts the file in its place and gives its size in bytes.
    ///
    /// # Errors
    ///
    /// I/O, or a listed work no document was given for: the file is not
    /// kept.
    pub fn finish(self) -> Result<u64, Error> {
        let wanted = self.wanted.len() as u64;
        if self.written != wanted {
            let _ = fs::remove_file(&self.partial);
            return Err(Error::Mismatch(format!(
                "{}: the list names {wanted} works, and {} of them were among the documents",
                self.path.display(),
                self.written
            )));
        }
        let file = self
            .out
            .finish()
            .and_then(|buffered| {
                buffered
                    .into_inner()
                    .map_err(io::IntoInnerError::into_error)
            })
            .map_err(io_at(&self.partial))?;
        file.sync_all().map_err(io_at(&self.partial))?;
        fs::rename(&self.partial, &self.path).map_err(io_at(&self.path))?;
        Ok(fs::metadata(&self.path).map_err(io_at(&self.path))?.len())
    }
}

fn line(out: &mut impl Write, value: &impl Serialize) -> io::Result<()> {
    serde_json::to_writer(&mut *out, value).map_err(io::Error::other)?;
    out.write_all(b"\n")
}

/// A file of documents, opened: its header read and checked, its
/// documents still to come.
pub struct Saved {
    path: PathBuf,
    header: Header,
    lines: io::Lines<BufReader<zstd::stream::read::Decoder<'static, BufReader<File>>>>,
}

impl std::fmt::Debug for Saved {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Saved")
            .field("path", &self.path)
            .field("header", &self.header)
            .finish_non_exhaustive()
    }
}

impl Saved {
    /// Opens a file of documents.
    ///
    /// # Errors
    ///
    /// I/O, or a file of another layout, model or merge.
    pub fn open(path: &Path) -> Result<Self, Error> {
        let wrong = |what: String| Error::Mismatch(format!("{}: {what}", path.display()));
        let file = File::open(path).map_err(io_at(path))?;
        let decoder = zstd::stream::read::Decoder::new(file).map_err(io_at(path))?;
        let mut lines = BufReader::new(decoder).lines();
        let first = lines
            .next()
            .ok_or_else(|| wrong("empty".to_owned()))?
            .map_err(io_at(path))?;
        let header: Header =
            serde_json::from_str(&first).map_err(|e| wrong(format!("its first line: {e}")))?;
        if (header.documents_format, header.model, header.merge)
            != (FORMAT, model::VERSION, merge::VERSION)
        {
            return Err(wrong(format!(
                "documents of layout {}, model {} and merge {}, not this binary's {FORMAT}, {} \
                 and {}",
                header.documents_format,
                header.model,
                header.merge,
                model::VERSION,
                merge::VERSION
            )));
        }
        Ok(Self {
            path: path.to_path_buf(),
            header,
            lines,
        })
    }

    #[allow(missing_docs)]
    #[must_use]
    pub const fn header(&self) -> &Header {
        &self.header
    }

    /// Every document, given to `each` in the file's order.
    ///
    /// # Errors
    ///
    /// I/O; a line that is not a document; other than the number of
    /// documents the header says; or whatever `each` returns.
    pub fn each(self, mut each: impl FnMut(Document) -> Result<(), Error>) -> Result<(), Error> {
        let wrong = |what: String| Error::Mismatch(format!("{}: {what}", self.path.display()));
        let mut read = 0u64;
        for text in self.lines {
            let text = text.map_err(io_at(&self.path))?;
            let document = serde_json::from_str(&text)
                .map_err(|e| wrong(format!("line {}: {e}", read + 2)))?;
            each(document)?;
            read += 1;
        }
        if read != self.header.works {
            return Err(wrong(format!(
                "{read} documents, and its header says {}",
                self.header.works
            )));
        }
        Ok(())
    }
}

/// Reads a file of documents, each given to `each` in the file's order,
/// and gives its header: [`Saved::open`] and [`Saved::each`].
///
/// # Errors
///
/// I/O; a file of another layout, model or merge; a line that is not a
/// document; other than the number of documents its header says; or
/// whatever `each` returns.
pub fn read(path: &Path, each: impl FnMut(Document) -> Result<(), Error>) -> Result<Header, Error> {
    let saved = Saved::open(path)?;
    let header = saved.header.clone();
    saved.each(each)?;
    Ok(header)
}

#[cfg(test)]
mod tests {
    use merge::kind::{Form, Genre};
    use merge::{SourceId, Sources};

    use super::*;

    fn document(id: u32, title: Option<&str>) -> Document {
        Document {
            id: WorkId::new(id).unwrap(),
            title: title.map(str::to_owned),
            abstract_text: id
                .is_multiple_of(2)
                .then(|| "A line.\nAnd \"another\", with a\ttab.".to_owned()),
            authors: vec!["Ada Lovelace".to_owned(), "Søren Ångström".to_owned()],
            venue: id.is_multiple_of(3).then(|| "Journal of Tests".to_owned()),
            year: Some(2020),
            form: Some(Form::JournalArticle),
            genre: (id % 2 == 1).then_some(Genre::Article),
            language: None,
            retracted: id == 2,
            corrected: false,
            concern: false,
            access: id.is_multiple_of(2).then_some(true),
            held_by: [SourceId::Pubmed, SourceId::Crossref]
                .into_iter()
                .collect::<Sources>(),
        }
    }

    /// The listed works' documents are saved and read back as they were,
    /// in the order given; a list naming a work with no document keeps no
    /// file; and a file is never written over.
    #[test]
    fn documents_are_read_back_as_they_were() {
        let dir = std::env::temp_dir().join(format!("ol-rht-documents-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let all: Vec<Document> = [
            (1, Some("IL-6 and TNF-α in sepsis")),
            (2, Some("新型冠状病毒肺炎患者的临床特征分析")),
            (3, None),
            (4, Some("Étude de l'efficacité")),
            (5, Some("A title\u{2028}with a line separator")),
            (6, Some("")),
        ]
        .into_iter()
        .map(|(id, title)| document(id, title))
        .collect();
        let works = |ids: &[u32]| -> HashSet<WorkId> {
            ids.iter().map(|id| WorkId::new(*id).unwrap()).collect()
        };
        let path = dir.join("documents.jsonl.zst");
        let mut writer = Writer::create(
            &path,
            "works.tsv".into(),
            "abc".into(),
            7,
            works(&[1, 2, 3, 5, 6]),
        )
        .unwrap();
        for range in all.chunks(4) {
            writer.add(range).unwrap();
        }
        assert!(!path.exists(), "not in its place until it is whole");
        assert!(writer.finish().unwrap() > 0);
        let mut back = Vec::new();
        let header = read(&path, |d| {
            back.push(d);
            Ok(())
        })
        .unwrap();
        assert_eq!(
            header,
            Header {
                documents_format: FORMAT,
                model: model::VERSION,
                merge: merge::VERSION,
                batch: 7,
                list: "works.tsv".into(),
                list_blake3: "abc".into(),
                works: 5,
            }
        );
        let kept: Vec<Document> = all.iter().filter(|d| d.id.get() != 4).cloned().collect();
        assert_eq!(back, kept);

        assert!(
            Writer::create(&path, "works.tsv".into(), "abc".into(), 7, works(&[1])).is_err(),
            "documents are kept"
        );
        let short = dir.join("short.jsonl.zst");
        let mut writer =
            Writer::create(&short, "works.tsv".into(), "abc".into(), 7, works(&[1, 70])).unwrap();
        writer.add(&all).unwrap();
        let refused = writer.finish().unwrap_err();
        assert!(matches!(refused, Error::Mismatch(_)), "{refused}");
        assert!(!short.exists());
        assert_eq!(
            fs::read_dir(&dir).unwrap().count(),
            1,
            "and nothing beside it"
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    /// A file of another version, or cut short, is refused.
    #[test]
    fn a_file_is_read_only_as_it_was_written() {
        let dir = std::env::temp_dir().join(format!("ol-rht-documents-bad-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let write = |name: &str, header: &Header, documents: &[Document]| {
            let path = dir.join(name);
            let mut out =
                zstd::stream::write::Encoder::new(File::create(&path).unwrap(), LEVEL).unwrap();
            line(&mut out, header).unwrap();
            for d in documents {
                line(&mut out, d).unwrap();
            }
            out.finish().unwrap();
            path
        };
        let header = Header {
            documents_format: FORMAT,
            model: model::VERSION,
            merge: merge::VERSION,
            batch: 1,
            list: "works.tsv".into(),
            list_blake3: "abc".into(),
            works: 2,
        };
        let documents = [document(1, Some("one")), document(2, Some("two"))];
        let count = |path: &Path| {
            let mut n = 0;
            read(path, |_| {
                n += 1;
                Ok(())
            })
            .map(|_| n)
        };
        assert_eq!(count(&write("whole", &header, &documents)).unwrap(), 2);
        for (name, header, documents) in [
            ("short", header.clone(), &documents[..1]),
            (
                "model",
                Header {
                    model: model::VERSION + 1,
                    ..header.clone()
                },
                &documents[..],
            ),
            (
                "merge",
                Header {
                    merge: merge::VERSION + 1,
                    ..header.clone()
                },
                &documents[..],
            ),
            (
                "layout",
                Header {
                    documents_format: FORMAT + 1,
                    ..header
                },
                &documents[..],
            ),
        ] {
            let refused = count(&write(name, &header, documents)).unwrap_err();
            assert!(matches!(refused, Error::Mismatch(_)), "{name}: {refused}");
        }
        fs::remove_dir_all(&dir).unwrap();
    }
}
