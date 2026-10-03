//! Building, opening and querying the index: a `.part` directory replaced
//! whole, a manifest of our own beside the engine's files, and a search
//! that returns ids, scores, stored titles and facet counts.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use merge::Document;
use merge::WorkId;
use serde::{Deserialize, Serialize};
use tantivy::collector::{Count, FacetCollector, FacetCounts as Counted, TopDocs};
use tantivy::query::{AllQuery, Query};
use tantivy::schema::{Facet, TantivyDocument, Value};
use tantivy::{DocAddress, Index, IndexReader, IndexWriter, ReloadPolicy, Searcher, Term};

use crate::analyzer::{Analyzer, Switches};
use crate::error::Error;
use crate::schema::{self, Fields};
use crate::translate;
use crate::until::Until;

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
    /// [`schema::description`] at build time.
    pub mapping: String,
    #[allow(missing_docs)]
    pub segments: usize,
    /// The engine's files, in bytes.
    pub bytes: u64,
    /// The analyzer a treatment's index was built with (tokenizer plan
    /// T1.2), registered again whenever the index is opened, so a query is
    /// split as its text was. The published index names none — its text
    /// is through Tantivy's `default` — and its manifest reads and writes
    /// as it did before there were treatments.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub analyzer: Option<Switches>,
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
pub(crate) fn document(fields: &Fields, d: &Document) -> Result<TantivyDocument, Error> {
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

/// The engine's index under `dir`, opened as `manifest` says it was
/// built: the analyzer it names registered, and every text field split by
/// that analyzer and no other — refused here, not at the first query.
fn engine(dir: &Path, manifest: &Manifest) -> Result<(Index, Fields), Error> {
    let index = Index::open_in_dir(dir)?;
    if let Some(switches) = manifest.analyzer {
        Analyzer::register(&index, switches);
    }
    let schema = index.schema();
    let fields = schema::fields(&schema)?;
    let named = manifest
        .analyzer
        .map_or_else(|| "default".to_owned(), |switches| switches.name());
    for field in fields.searched() {
        let split_by = schema::analyzer_of(&schema, field);
        if split_by != Some(named.as_str()) {
            return Err(Error::Mismatch(format!(
                "{}: the field {} is split by {}, and the manifest names {named}",
                dir.display(),
                schema.get_field_name(field),
                split_by.unwrap_or("nothing")
            )));
        }
    }
    Ok((index, fields))
}

/// Indexes every document into `dir` as of `batch`, replacing what was
/// there: the published mapping, with no analyzer named.
///
/// # Errors
///
/// I/O, the engine, or a facet path that is not one.
pub fn build(dir: &Path, documents: &[Document], batch: u64) -> Result<Built, Error> {
    let mut build = Build::start(dir, None)?;
    build.add(documents)?;
    build.finish(batch)
}

/// A build of the index taken a batch of documents at a time, so that an
/// assembly given out a range of ids at a time never has to hold them all
/// (the serving layer's memory, 2026-09-24): [`build`], in three steps.
pub struct Build {
    dir: PathBuf,
    part: PathBuf,
    index: Index,
    writer: IndexWriter,
    fields: Fields,
    analyzer: Option<Switches>,
    documents: u64,
}

impl std::fmt::Debug for Build {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Build")
            .field("dir", &self.dir)
            .field("documents", &self.documents)
            .finish_non_exhaustive()
    }
}

impl Build {
    /// An empty index beside `dir`, which takes its place at
    /// [`Build::finish`]: the published mapping, or with `analyzer` a
    /// treatment's, its text split by those switches.
    ///
    /// # Errors
    ///
    /// I/O, or the engine.
    pub fn start(dir: &Path, analyzer: Option<Switches>) -> Result<Self, Error> {
        let io = |path: &Path, e| Error::Io {
            path: path.to_path_buf(),
            source: e,
        };
        let part = part_of(dir);
        if part.exists() {
            fs::remove_dir_all(&part).map_err(|e| io(&part, e))?;
        }
        fs::create_dir_all(&part).map_err(|e| io(&part, e))?;
        let (schema, fields) = schema::provisional(analyzer);
        let index = Index::create_in_dir(&part, schema)?;
        if let Some(switches) = analyzer {
            Analyzer::register(&index, switches);
        }
        let writer: IndexWriter = index.writer(256 << 20)?;
        Ok(Self {
            dir: dir.to_path_buf(),
            part,
            index,
            writer,
            fields,
            analyzer,
            documents: 0,
        })
    }

    /// Indexes `documents`.
    ///
    /// # Errors
    ///
    /// The engine, or a facet path that is not one.
    pub fn add(&mut self, documents: &[Document]) -> Result<(), Error> {
        for d in documents {
            self.writer.add_document(document(&self.fields, d)?)?;
            self.documents += 1;
        }
        Ok(())
    }

    /// Commits at `batch`, writes the manifest, and puts the index in its
    /// place.
    ///
    /// # Errors
    ///
    /// I/O, or the engine.
    pub fn finish(mut self, batch: u64) -> Result<Built, Error> {
        let io = |path: &Path, e| Error::Io {
            path: path.to_path_buf(),
            source: e,
        };
        commit(&mut self.writer, batch)?;
        self.writer.wait_merging_threads()?;
        let segments = self.index.searchable_segment_metas()?.len();
        let bytes = bytes_under(&self.part).map_err(|e| io(&self.part, e))?;
        let manifest = Manifest {
            search: crate::VERSION,
            model: model::VERSION,
            merge: merge::VERSION,
            engine: crate::ENGINE.to_owned(),
            built_at: ingest::time::now(),
            batch,
            documents: self.documents,
            mapping: schema::description(self.analyzer),
            segments,
            bytes,
            analyzer: self.analyzer,
        };
        manifest.write(&self.part)?;
        drop(self.index);
        if self.dir.exists() {
            fs::remove_dir_all(&self.dir).map_err(|e| io(&self.dir, e))?;
        }
        fs::rename(&self.part, &self.dir).map_err(|e| io(&self.dir, e))?;
        Ok(Built {
            dir: self.dir,
            manifest,
        })
    }
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
    let (index, fields) = engine(dir, &manifest)?;
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
    /// the schema's fields, with the analyzer the manifest names.
    ///
    /// # Errors
    ///
    /// I/O, the engine, another version's index, or text fields split by
    /// an analyzer the manifest does not name.
    pub fn open(dir: &Path) -> Result<Self, Error> {
        let manifest = Manifest::read(dir)?;
        manifest.check()?;
        let (index, fields) = engine(dir, &manifest)?;
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

    /// The top `limit` hits for `asked` over the searched fields, and the
    /// top `top` values of every facet root over every match, in one pass
    /// over the matches; stopped at `until` when there is one.
    ///
    /// # Errors
    ///
    /// [`Error::Deadline`] past `until`, or the engine.
    pub fn search(
        &self,
        asked: &query_parser::Query,
        limit: usize,
        top: usize,
        until: Option<Instant>,
    ) -> Result<Hits, Error> {
        let query = translate::query(&self.index, self.fields.searched(), asked)?;
        let searcher = self.reader.searcher();
        let collectors = (
            TopDocs::with_limit(limit.max(1)).order_by_score(),
            Count,
            facet_collector(),
        );
        let (docs, total, counted) = match until {
            None => searcher.search(&*query, &collectors)?,
            Some(until) => {
                let collector = Until::new(collectors, until);
                searcher.search(&*query, &collector).map_err(|e| {
                    if collector.expired() {
                        Error::Deadline
                    } else {
                        e.into()
                    }
                })?
            }
        };
        Ok(Hits {
            total,
            hits: self.hits(&searcher, docs)?,
            facets: facet_counts(&counted, top),
        })
    }

    /// The top `limit` works for a citation's words, read by the query
    /// parser (citation lookup plan, D3): searched in the fields a citation
    /// names a work by ([`Fields::cited`]), with no facets and no count —
    /// they are candidates for the check, not an answer; stopped at `until`
    /// when there is one.
    ///
    /// # Errors
    ///
    /// [`Error::Deadline`] past `until`, or the engine.
    pub fn candidates(
        &self,
        asked: &query_parser::Query,
        limit: usize,
        until: Option<Instant>,
    ) -> Result<Vec<Hit>, Error> {
        let query = translate::query(&self.index, self.fields.cited(), asked)?;
        let searcher = self.reader.searcher();
        let collector = TopDocs::with_limit(limit.max(1)).order_by_score();
        let docs = match until {
            None => searcher.search(&*query, &collector)?,
            Some(until) => {
                let collector = Until::new(collector, until);
                searcher.search(&*query, &collector).map_err(|e| {
                    if collector.expired() {
                        Error::Deadline
                    } else {
                        e.into()
                    }
                })?
            }
        };
        self.hits(&searcher, docs)
    }

    /// Each scored document as a hit: its id and stored title.
    fn hits(&self, searcher: &Searcher, docs: Vec<(f32, DocAddress)>) -> Result<Vec<Hit>, Error> {
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
        Ok(hits)
    }

    /// The top `top` values of every facet root over every document.
    ///
    /// # Errors
    ///
    /// The engine.
    pub fn facets_of_all(&self, top: usize) -> Result<FacetCounts, Error> {
        let query: &dyn Query = &AllQuery;
        let counted = self.reader.searcher().search(query, &facet_collector())?;
        Ok(facet_counts(&counted, top))
    }
}

/// A collector of every facet root's counts.
fn facet_collector() -> FacetCollector {
    let mut collector = FacetCollector::for_field("facet");
    for root in schema::ROOTS {
        collector.add_facet(root);
    }
    collector
}

/// The top `top` values of every facet root in `counted`.
fn facet_counts(counted: &Counted, top: usize) -> FacetCounts {
    schema::ROOTS
        .iter()
        .map(|root| {
            let values = counted
                .top_k(root, top)
                .into_iter()
                .map(|(facet, n)| {
                    let path = facet.to_path_string();
                    let value = path.rsplit('/').next().unwrap_or("").to_owned();
                    (value, n)
                })
                .collect();
            ((*root).to_owned(), values)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use merge::Document;
    use merge::kind::{Form, Genre};
    use merge::{SourceId, Sources, WorkId};

    use std::time::{Duration, Instant};

    use super::{Opened, build};
    use crate::Error;

    fn asked(text: &str) -> query_parser::Query {
        query_parser::parse(text).unwrap()
    }

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
        let hits = opened.search(&asked("aspirin"), 10, 5, None).unwrap();
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
        assert_eq!(
            opened
                .search(&asked("lovelace"), 10, 5, None)
                .unwrap()
                .total,
            3
        );
        assert_eq!(
            opened
                .search(&asked("\"of tests\""), 10, 5, None)
                .unwrap()
                .total,
            3
        );
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
        let hits = opened.search(&asked("aspirin"), 10, 5, None).unwrap();
        let mut ids: Vec<u32> = hits.hits.iter().map(|h| h.id.get()).collect();
        ids.sort_unstable();
        assert_eq!(ids, [1, 2, 4]);
        assert_eq!(
            opened.search(&asked("revised"), 10, 5, None).unwrap().total,
            1
        );
        assert_eq!(
            opened
                .search(&asked("entirely"), 10, 5, None)
                .unwrap()
                .total,
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

    /// A citation's candidates are found by title, authors and journal,
    /// never by abstract, which no citation carries (citation lookup plan,
    /// D3); the search box still finds by all four.
    #[test]
    fn candidates_are_found_by_what_a_citation_names() {
        let root = std::env::temp_dir().join(format!("ol-rht-search-cited-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let dir = root.join(crate::version());
        let docs = vec![
            document(
                1,
                "Randomized trial of aspirin",
                "Adults with headache.",
                2020,
            ),
            document(2, "Something else entirely", "Mentions aspirin once.", 2021),
        ];
        build(&dir, &docs, 1).unwrap();
        let opened = Opened::open(&dir).unwrap();
        let ids = |hits: Vec<super::Hit>| hits.iter().map(|h| h.id.get()).collect::<Vec<_>>();

        let cited = opened.candidates(&asked("aspirin"), 20, None).unwrap();
        assert_eq!(
            ids(cited),
            [1],
            "the abstract's aspirin is not a citation's"
        );
        let boxed = opened.search(&asked("aspirin"), 20, 5, None).unwrap();
        assert_eq!(boxed.total, 2, "the search box's is");

        // Authors and journal are searched, and the stored title comes back.
        let cited = opened
            .candidates(&asked("Lovelace. Journal of Tests."), 20, None)
            .unwrap();
        assert_eq!(cited.len(), 2);
        assert!(cited.iter().all(|h| h.title.is_some()));
        // A limit is a limit.
        assert_eq!(
            opened
                .candidates(&asked("Lovelace"), 1, None)
                .unwrap()
                .len(),
            1
        );
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// A treatment's index (tokenizer plan T1.2): built with switches,
    /// opened with the analyzer its manifest names, and asked as its text
    /// was split — the spellings the plan says must meet (I3), and scripts
    /// written without spaces (I1), found; the published index beside it
    /// finds none of them, and its manifest names no analyzer.
    #[test]
    fn a_treatments_index_is_asked_as_it_was_built() {
        use crate::analyzer::{Switches, Unspaced};
        let root = std::env::temp_dir().join(format!("ol-rht-search-treat-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let docs = vec![
            document(1, "IL-6 and TNF-α in sepsis", "", 2020),
            document(2, "Étude de l'efficacité", "", 2020),
            document(3, "新型冠状病毒肺炎患者的临床特征分析", "", 2020),
            document(4, "Dose of 5 µg of ＤＮＡ", "", 2020),
        ];
        let switches = Switches {
            cap: None,
            unspaced: Unspaced::Bigrams,
            fold: true,
            greek: true,
            joins: true,
        };
        let treated = root.join("treated");
        let mut treatment = super::Build::start(&treated, Some(switches)).unwrap();
        treatment.add(&docs).unwrap();
        let manifest = treatment.finish(3).unwrap().manifest;
        assert_eq!(manifest.analyzer, Some(switches));
        assert!(
            manifest.mapping.contains(&switches.name()),
            "{}",
            manifest.mapping
        );
        let published = root.join("published");
        let plain = build(&published, &docs, 3).unwrap();
        assert_eq!(plain.manifest.analyzer, None);
        assert_eq!(plain.manifest.mapping, crate::schema::DESCRIPTION);
        let text = std::fs::read_to_string(published.join(super::MANIFEST)).unwrap();
        assert!(!text.contains("analyzer"), "{text}");

        let (treated, published) = (
            Opened::open(&treated).unwrap(),
            Opened::open(&published).unwrap(),
        );
        let found = |opened: &Opened, text: &str| {
            let hits = opened.search(&asked(text), 10, 0, None).unwrap();
            let mut ids: Vec<u32> = hits.hits.iter().map(|h| h.id.get()).collect();
            ids.sort_unstable();
            ids
        };
        for (text, work) in [
            ("IL6", 1),
            ("TNF-alpha", 1),
            ("alpha", 1),
            ("etude", 2),
            ("ETUDE", 2),
            ("肺炎", 3),
            ("临床特征", 3),
            ("ug", 4),
            ("DNA", 4),
        ] {
            assert_eq!(found(&treated, text), [work], "{text:?}, treated");
            assert!(
                !found(&published, text).contains(&work),
                "{text:?}, published"
            );
        }
        // As written, both find them.
        for (text, work) in [("IL-6", 1), ("Étude", 2), ("sepsis", 1)] {
            assert_eq!(found(&treated, text), [work], "{text:?}, treated");
            assert_eq!(found(&published, text), [work], "{text:?}, published");
        }
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// An index whose manifest names another analyzer than its text was
    /// split by is refused when it is opened, not at the first query; and
    /// a manifest written before there were treatments still reads.
    #[test]
    fn an_index_is_opened_only_as_it_was_built() {
        use crate::analyzer::{Switches, Unspaced};
        let root = std::env::temp_dir().join(format!("ol-rht-search-named-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let docs = vec![document(1, "Randomized trial of aspirin", "", 2020)];
        let rewrite = |dir: &std::path::Path, analyzer: Option<Switches>| {
            let mut manifest = super::Manifest::read(dir).unwrap();
            manifest.analyzer = analyzer;
            manifest.write(dir).unwrap();
        };
        let other = Switches {
            unspaced: Unspaced::Words,
            ..Switches::TODAY
        };

        let treated = root.join("treated");
        let mut treatment = super::Build::start(&treated, Some(Switches::TODAY)).unwrap();
        treatment.add(&docs).unwrap();
        treatment.finish(1).unwrap();
        Opened::open(&treated).unwrap();
        for wrong in [None, Some(other)] {
            rewrite(&treated, wrong);
            let refused = Opened::open(&treated).unwrap_err();
            assert!(matches!(refused, Error::Mismatch(_)), "{refused}");
        }

        let published = root.join("published");
        build(&published, &docs, 1).unwrap();
        let before = std::fs::read_to_string(published.join(super::MANIFEST)).unwrap();
        rewrite(&published, None);
        let after = std::fs::read_to_string(published.join(super::MANIFEST)).unwrap();
        assert_eq!(before, after, "a published manifest is written as it was");
        Opened::open(&published).unwrap();
        rewrite(&published, Some(other));
        let refused = Opened::open(&published).unwrap_err();
        assert!(matches!(refused, Error::Mismatch(_)), "{refused}");
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// A search past its time stops with [`Error::Deadline`]; one with
    /// time to spare answers as a search with no limit does; and what
    /// twenty nested parentheses cost Tantivy's parser a second for is,
    /// at thirty thousand, a word like any other.
    #[test]
    fn a_search_keeps_to_its_time() {
        let root = std::env::temp_dir().join(format!("ol-rht-search-until-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let dir = root.join(crate::version());
        let docs: Vec<_> = (1..=50)
            .map(|id| {
                document(
                    id,
                    "Randomized trial of aspirin",
                    "Aspirin in adults.",
                    2020,
                )
            })
            .collect();
        build(&dir, &docs, 1).unwrap();
        let opened = Opened::open(&dir).unwrap();
        let aspirin = asked("aspirin");
        let past = Instant::now();
        assert!(matches!(
            opened.search(&aspirin, 10, 5, Some(past)),
            Err(Error::Deadline)
        ));
        let spare = Instant::now() + Duration::from_secs(60);
        assert_eq!(
            opened.search(&aspirin, 10, 5, Some(spare)).unwrap(),
            opened.search(&aspirin, 10, 5, None).unwrap()
        );
        let deep = asked(&format!(
            "{}aspirin{}",
            "(".repeat(30_000),
            ")".repeat(30_000)
        ));
        let started = Instant::now();
        let hits = opened.search(&deep, 10, 5, Some(spare)).unwrap();
        assert!(started.elapsed() < Duration::from_secs(5));
        assert_eq!(hits.total, 50);
        std::fs::remove_dir_all(&root).unwrap();
    }
}
