//! Building, opening and querying the index: a `.part` directory replaced
//! whole, a manifest of our own beside the engine's files, and a search
//! that returns ids, scores, stored titles and facet counts.

use std::fs;
use std::path::{Path, PathBuf};

use merge::Document;
use merge::WorkId;
use serde::{Deserialize, Serialize};
use tantivy::collector::{Count, FacetCollector, TopDocs};
use tantivy::query::{AllQuery, Query, QueryParser};
use tantivy::schema::{Facet, TantivyDocument, Value};
use tantivy::{Index, IndexReader, IndexWriter, ReloadPolicy, Term};

use crate::error::Error;
use crate::schema::{self, Fields};

/// Our manifest beside the engine's files.
pub const MANIFEST: &str = "ol-rht.json";

/// What a build records about itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    #[allow(missing_docs)]
    pub search: u32,
    #[allow(missing_docs)]
    pub model: u32,
    #[allow(missing_docs)]
    pub merge: u32,
    #[allow(missing_docs)]
    pub engine: String,
    #[allow(missing_docs)]
    pub built_at: String,
    /// The journal batch the build indexed; the batch the index has
    /// absorbed since is the engine's commit payload, read by
    /// [`Opened::batch`].
    pub batch: u64,
    /// Documents at build time.
    pub documents: u64,
    /// [`schema::DESCRIPTION`] at build time.
    pub mapping: String,
    #[allow(missing_docs)]
    pub segments: usize,
    /// The engine's files, in bytes.
    pub bytes: u64,
}

impl Manifest {
    fn path(dir: &Path) -> PathBuf {
        dir.join(MANIFEST)
    }

    /// Reads `dir`'s manifest.
    ///
    /// # Errors
    ///
    /// I/O, or a manifest that does not parse.
    pub fn read(dir: &Path) -> Result<Self, Error> {
        let path = Self::path(dir);
        let text = fs::read_to_string(&path).map_err(|e| Error::Io {
            path: path.clone(),
            source: e,
        })?;
        serde_json::from_str(&text).map_err(|source| Error::Manifest { path, source })
    }

    fn write(&self, dir: &Path) -> Result<(), Error> {
        let path = Self::path(dir);
        let mut text = serde_json::to_string_pretty(self).map_err(|source| Error::Manifest {
            path: path.clone(),
            source,
        })?;
        text.push('\n');
        fs::write(&path, text).map_err(|e| Error::Io { path, source: e })
    }

    fn check(&self) -> Result<(), Error> {
        if (self.search, self.model, self.merge) != (crate::VERSION, model::VERSION, merge::VERSION)
            || self.engine != crate::ENGINE
        {
            return Err(Error::Mismatch(format!(
                "built as {} search {} over model {} and merge {}, not this binary's {} {}",
                self.engine,
                self.search,
                self.model,
                self.merge,
                crate::ENGINE,
                crate::version()
            )));
        }
        Ok(())
    }
}

/// What a build left on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Built {
    #[allow(missing_docs)]
    pub dir: PathBuf,
    #[allow(missing_docs)]
    pub manifest: Manifest,
}

/// A document as the engine takes it.
fn document(fields: &Fields, d: &Document) -> Result<TantivyDocument, Error> {
    let mut doc = TantivyDocument::new();
    doc.add_u64(fields.id, u64::from(d.id.get()));
    if let Some(title) = &d.title {
        doc.add_text(fields.title, title);
    }
    if let Some(text) = &d.abstract_text {
        doc.add_text(fields.abstract_text, text);
    }
    for author in &d.authors {
        doc.add_text(fields.authors, author);
    }
    if let Some(venue) = &d.venue {
        doc.add_text(fields.venue, venue);
    }
    for path in schema::facets(d) {
        let facet = Facet::from_text(&path).map_err(|e| Error::Facet(format!("{path}: {e}")))?;
        doc.add_facet(fields.facet, facet);
    }
    Ok(doc)
}

/// Commits with the batch as the commit's payload: the one clock the
/// engine's own files carry.
fn commit(writer: &mut IndexWriter, batch: u64) -> Result<(), Error> {
    let mut prepared = writer.prepare_commit()?;
    prepared.set_payload(&batch.to_string());
    prepared.commit()?;
    Ok(())
}

/// Indexes every document into `dir` as of `batch`, replacing what was
/// there.
///
/// # Errors
///
/// I/O, the engine, or a facet path that is not one.
pub fn build(dir: &Path, documents: &[Document], batch: u64) -> Result<Built, Error> {
    let io = |path: &Path, e| Error::Io {
        path: path.to_path_buf(),
        source: e,
    };
    let part = part_of(dir);
    if part.exists() {
        fs::remove_dir_all(&part).map_err(|e| io(&part, e))?;
    }
    fs::create_dir_all(&part).map_err(|e| io(&part, e))?;
    let (schema, fields) = schema::provisional();
    let index = Index::create_in_dir(&part, schema)?;
    let mut writer: IndexWriter = index.writer(256 << 20)?;
    for d in documents {
        writer.add_document(document(&fields, d)?)?;
    }
    commit(&mut writer, batch)?;
    writer.wait_merging_threads()?;
    let segments = index.searchable_segment_metas()?.len();
    let bytes = bytes_under(&part).map_err(|e| io(&part, e))?;
    let manifest = Manifest {
        search: crate::VERSION,
        model: model::VERSION,
        merge: merge::VERSION,
        engine: crate::ENGINE.to_owned(),
        built_at: ingest::time::now(),
        batch,
        documents: documents.len() as u64,
        mapping: schema::DESCRIPTION.to_owned(),
        segments,
        bytes,
    };
    manifest.write(&part)?;
    drop(index);
    if dir.exists() {
        fs::remove_dir_all(dir).map_err(|e| io(dir, e))?;
    }
    fs::rename(&part, dir).map_err(|e| io(dir, e))?;
    Ok(Built {
        dir: dir.to_path_buf(),
        manifest,
    })
}

fn part_of(dir: &Path) -> PathBuf {
    let name = dir.file_name().and_then(|n| n.to_str()).unwrap_or("search");
    dir.with_file_name(format!("{name}.part"))
}

/// What absorbing a batch did.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Absorbed {
    /// Documents added, each replacing any earlier document of its work.
    pub put: u64,
    /// Works whose document was removed.
    pub removed: u64,
    /// Documents in the index after the commit.
    pub documents: u64,
}

/// Applies one batch in place (serving plan §5): every work in `puts`
/// gets its document, replacing any it had; every work in `removes`
/// loses its document; the commit carries `batch`. The manifest beside
/// the engine's files is not touched: it describes the build, and the
/// epoch digests it.
///
/// # Errors
///
/// I/O, the engine — another writer holding the index among them — or a
/// facet path that is not one.
pub fn absorb(
    dir: &Path,
    batch: u64,
    puts: &[Document],
    removes: &[WorkId],
) -> Result<Absorbed, Error> {
    let manifest = Manifest::read(dir)?;
    manifest.check()?;
    let index = Index::open_in_dir(dir)?;
    let fields = schema::fields(&index.schema())?;
    let mut writer: IndexWriter = index.writer(64 << 20)?;
    for id in removes.iter().chain(puts.iter().map(|d| &d.id)) {
        writer.delete_term(Term::from_field_u64(fields.id, u64::from(id.get())));
    }
    for d in puts {
        writer.add_document(document(&fields, d)?)?;
    }
    commit(&mut writer, batch)?;
    writer.wait_merging_threads()?;
    let reader = index
        .reader_builder()
        .reload_policy(ReloadPolicy::Manual)
        .try_into()?;
    Ok(Absorbed {
        put: puts.len() as u64,
        removed: removes.len() as u64,
        documents: reader.searcher().num_docs(),
    })
}

/// The batch an index's last commit carries.
///
/// # Errors
///
/// The engine, or a payload that is not a batch number.
pub fn batch_of(index: &Index) -> Result<u64, Error> {
    let payload = index.load_metas()?.payload;
    match payload {
        Some(text) => text.parse().map_err(|_| {
            Error::Mismatch(format!("the commit payload {text:?} is not a batch number"))
        }),
        None => Err(Error::Mismatch(
            "the index's commit carries no batch number".to_owned(),
        )),
    }
}

/// The engine's files under `dir`, in bytes, the manifest excluded.
fn bytes_under(dir: &Path) -> std::io::Result<u64> {
    let mut total = 0;
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        if entry.file_name() == MANIFEST {
            continue;
        }
        total += entry.metadata()?.len();
    }
    Ok(total)
}

/// One hit.
#[derive(Debug, Clone, PartialEq)]
pub struct Hit {
    #[allow(missing_docs)]
    pub score: f32,
    #[allow(missing_docs)]
    pub id: WorkId,
    /// The stored title.
    pub title: Option<String>,
}

/// Per facet root, the top values and their counts.
pub type FacetCounts = Vec<(String, Vec<(String, u64)>)>;

/// What a search returns.
#[derive(Debug, Clone, PartialEq)]
pub struct Hits {
    /// Documents matching, before the limit.
    pub total: usize,
    #[allow(missing_docs)]
    pub hits: Vec<Hit>,
    /// Over every match.
    pub facets: FacetCounts,
}

/// An open index.
pub struct Opened {
    manifest: Manifest,
    index: Index,
    fields: Fields,
    reader: IndexReader,
}

impl std::fmt::Debug for Opened {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Opened")
            .field("manifest", &self.manifest)
            .finish_non_exhaustive()
    }
}

impl Opened {
    /// Opens the index under `dir`, checking the manifest's versions and
    /// the schema's fields.
    ///
    /// # Errors
    ///
    /// I/O, the engine, another version's index.
    pub fn open(dir: &Path) -> Result<Self, Error> {
        let manifest = Manifest::read(dir)?;
        manifest.check()?;
        let index = Index::open_in_dir(dir)?;
        let fields = schema::fields(&index.schema())?;
        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::Manual)
            .try_into()?;
        Ok(Self {
            manifest,
            index,
            fields,
            reader,
        })
    }

    #[allow(missing_docs)]
    #[must_use]
    pub const fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    /// Documents in the index.
    #[must_use]
    pub fn documents(&self) -> u64 {
        self.reader.searcher().num_docs()
    }

    /// The batch the index has absorbed: its last commit's payload.
    ///
    /// # Errors
    ///
    /// The engine, or a payload that is not a batch number.
    pub fn batch(&self) -> Result<u64, Error> {
        batch_of(&self.index)
    }

    /// Picks up commits made since the index was opened or last reloaded,
    /// which is how a batch absorbed by the merge reaches a searcher.
    ///
    /// # Errors
    ///
    /// The engine.
    pub fn reload(&self) -> Result<(), Error> {
        self.reader.reload()?;
        Ok(())
    }

    fn parse(&self, text: &str) -> Result<Box<dyn Query>, Error> {
        QueryParser::for_index(&self.index, self.fields.searched())
            .parse_query(text)
            .map_err(Error::Query)
    }

    /// The top `limit` hits for `text` over the searched fields, and the
    /// top `top` values of every facet root over every match.
    ///
    /// # Errors
    ///
    /// A query the parser refuses, or the engine.
    pub fn search(&self, text: &str, limit: usize, top: usize) -> Result<Hits, Error> {
        let query = self.parse(text)?;
        let searcher = self.reader.searcher();
        let by_score = TopDocs::with_limit(limit.max(1)).order_by_score();
        let (docs, total) = searcher.search(&query, &(by_score, Count))?;
        let mut hits = Vec::with_capacity(docs.len());
        for (score, address) in docs {
            let doc: TantivyDocument = searcher.doc(address)?;
            let id = doc
                .get_first(self.fields.id)
                .and_then(|v| v.as_u64())
                .and_then(|n| u32::try_from(n).ok())
                .and_then(WorkId::new)
                .ok_or_else(|| Error::Mismatch("a document without an id".to_owned()))?;
            let title = doc
                .get_first(self.fields.title)
                .and_then(|v| v.as_str())
                .map(str::to_owned);
            hits.push(Hit { score, id, title });
        }
        let facets = self.facets(&*query, top)?;
        Ok(Hits {
            total,
            hits,
            facets,
        })
    }

    /// The top `top` values of every facet root over every document.
    ///
    /// # Errors
    ///
    /// The engine.
    pub fn facets_of_all(&self, top: usize) -> Result<FacetCounts, Error> {
        self.facets(&AllQuery, top)
    }

    fn facets(&self, query: &dyn Query, top: usize) -> Result<FacetCounts, Error> {
        let mut collector = FacetCollector::for_field("facet");
        for root in schema::ROOTS {
            collector.add_facet(root);
        }
        let counts = self.reader.searcher().search(query, &collector)?;
        let mut out = Vec::with_capacity(schema::ROOTS.len());
        for root in schema::ROOTS {
            let values = counts
                .top_k(root, top)
                .into_iter()
                .map(|(facet, n)| {
                    let path = facet.to_path_string();
                    let value = path.rsplit('/').next().unwrap_or("").to_owned();
                    (value, n)
                })
                .collect();
            out.push((root.to_owned(), values));
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use merge::Document;
    use merge::kind::{Form, Genre};
    use merge::{SourceId, Sources, WorkId};

    use super::{Opened, build};

    fn document(id: u32, title: &str, abstract_text: &str, year: u16) -> Document {
        Document {
            id: WorkId::new(id).unwrap(),
            title: Some(title.to_owned()),
            abstract_text: Some(abstract_text.to_owned()),
            authors: vec!["Ada Lovelace".to_owned()],
            venue: Some("Journal of Tests".to_owned()),
            year: Some(year),
            form: Some(Form::JournalArticle),
            genre: Some(Genre::Article),
            language: None,
            retracted: id == 2,
            corrected: false,
            concern: false,
            access: Some(true),
            held_by: [SourceId::Pubmed].into_iter().collect::<Sources>(),
        }
    }

    #[test]
    fn documents_are_found_with_their_facets() {
        let root = std::env::temp_dir().join(format!("ol-rht-search-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let dir = root.join(crate::version());
        let docs = vec![
            document(
                1,
                "Randomized trial of aspirin",
                "A trial of aspirin in adults.",
                2020,
            ),
            document(
                2,
                "Aspirin and headache",
                "Retracted for data problems.",
                2021,
            ),
            document(3, "Something else entirely", "Unrelated text.", 2021),
        ];
        let built = build(&dir, &docs, 7).unwrap();
        assert_eq!(built.manifest.documents, 3);
        let opened = Opened::open(&dir).unwrap();
        assert_eq!(opened.documents(), 3);
        assert_eq!(opened.batch().unwrap(), 7);
        let hits = opened.search("aspirin", 10, 5).unwrap();
        assert_eq!(hits.total, 2);
        let ids: Vec<u32> = hits.hits.iter().map(|h| h.id.get()).collect();
        assert!(ids.contains(&1) && ids.contains(&2), "{ids:?}");
        assert!(hits.hits.iter().all(|h| h.title.is_some()));
        let years = hits
            .facets
            .iter()
            .find(|(root, _)| root == "/year")
            .unwrap();
        assert_eq!(years.1.len(), 2);
        let status = hits
            .facets
            .iter()
            .find(|(root, _)| root == "/status")
            .unwrap();
        assert_eq!(status.1, vec![("retracted".to_owned(), 1)]);
        let all = opened.facets_of_all(10).unwrap();
        let held = all.iter().find(|(root, _)| root == "/held_by").unwrap();
        assert_eq!(held.1, vec![("pubmed".to_owned(), 3)]);
        assert_eq!(opened.search("lovelace", 10, 5).unwrap().total, 3);
        assert_eq!(opened.search("\"of tests\"", 10, 5).unwrap().total, 3);
        // A batch absorbed: one document replaced, one removed, one added.
        let absorbed = super::absorb(
            &dir,
            8,
            &[
                document(2, "Aspirin and headache, revised", "Now fine.", 2022),
                document(4, "A fourth work on aspirin", "New.", 2023),
            ],
            &[WorkId::new(3).unwrap()],
        )
        .unwrap();
        assert_eq!(
            (absorbed.put, absorbed.removed, absorbed.documents),
            (2, 1, 3)
        );
        assert_eq!(opened.documents(), 3, "the searcher lags until reloaded");
        opened.reload().unwrap();
        assert_eq!(opened.batch().unwrap(), 8);
        let hits = opened.search("aspirin", 10, 5).unwrap();
        let mut ids: Vec<u32> = hits.hits.iter().map(|h| h.id.get()).collect();
        ids.sort_unstable();
        assert_eq!(ids, [1, 2, 4]);
        assert_eq!(opened.search("revised", 10, 5).unwrap().total, 1);
        assert_eq!(
            opened.search("entirely", 10, 5).unwrap().total,
            0,
            "work 3 is gone"
        );
        let status = hits
            .facets
            .iter()
            .find(|(root, _)| root == "/status")
            .unwrap();
        assert_eq!(status.1, vec![("retracted".to_owned(), 1)]);
        std::fs::remove_dir_all(&root).unwrap();
    }
}
