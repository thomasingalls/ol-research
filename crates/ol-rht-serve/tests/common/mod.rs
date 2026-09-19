//! The fake both transports are tested over: works 1 (held, cites 2 and
//! 3, cited by 3) and 2 and 3 (stubs), a shared DOI on 2 and 3, and id 9
//! retired by a split. Everything the query core answers, answered from
//! memory, so the transports are tested without a store.
//!
//! Its three arcs are 1 → 2, 1 → 3 and 3 → 1, and every list it answers
//! is one of those read in one orientation, so an expansion and a path
//! over it see the same graph the pages do.

use std::collections::{BTreeMap, HashMap};

use merge::identity::Standing;
use merge::record::{Field, Flags, StatusBy};
use merge::{OwnedName, SourceId, Sources, WorkId};
use ol_rht_serve::answers::{Answers, Counts, Direction, Page};
use search::Hits;
use serve::epoch::Manifest;
use serve::row::{Fields, Named, Record, Text};
use serve::{By, Expansion, Neighbour, Resolved, Row, Walk};

pub(crate) fn id(n: u32) -> WorkId {
    WorkId::new(n).unwrap()
}

pub(crate) fn public_text(n: u32) -> String {
    merge::public::encode(u64::from(n))
}

/// Works 1 (held, cites 2 and 3, cited by 3) and 2 and 3 (stubs); a
/// shared DOI on 2 and 3; id 9 retired by a split into 2 and 3.
pub(crate) struct Fake {
    epoch: Manifest,
    rows: BTreeMap<WorkId, Row>,
    names: BTreeMap<String, Resolved>,
}

fn row(n: u32, names: &[&str], stub: bool) -> Row {
    Row {
        id: id(n),
        stub,
        public: Some(public_text(n)),
        names: names
            .iter()
            .map(|t| Named {
                name: OwnedName::parse(t).unwrap(),
                standing: Standing::Promoted,
                by: [SourceId::Pubmed].into_iter().collect(),
                native: !stub,
                cited_by: smallvec::SmallVec::new(),
            })
            .collect(),
        records: if stub {
            Vec::new()
        } else {
            vec![Record {
                source: SourceId::Pubmed,
                native: OwnedName::parse(names[0]).unwrap(),
                promoted: true,
                asserts: smallvec::smallvec![serve::row::Assert {
                    name: OwnedName::parse(names[0]).unwrap(),
                    field: "pmid".into(),
                    native: true,
                }],
                view: merge::view::Owned::default(),
            }]
        },
        rejected: Vec::new(),
        fields: Fields {
            title: (!stub).then(|| Field {
                value: "A <i>titled</i> work".into(),
                from: SourceId::Pubmed,
                others: smallvec::smallvec![merge::record::Other {
                    source: SourceId::Crossref,
                    value: "A titled work: with subtitle".into()
                }],
            }),
            abstract_in: if stub {
                Sources::NONE
            } else {
                [SourceId::Pubmed].into_iter().collect()
            },
            status: StatusBy::default(),
            ..Fields::default()
        },
        flags: Flags::default(),
        text: (!stub).then(|| Text {
            title: Some("A titled work".to_owned()),
            abstract_text: Some("The abstract.".to_owned()),
            authors: vec!["Ada Lovelace".to_owned()],
            venue: Some("Journal of Tests".to_owned()),
        }),
    }
}

impl Fake {
    pub(crate) fn new(epoch: u64) -> Self {
        let mut names = BTreeMap::new();
        names.insert("pmid:1".to_owned(), Resolved::One(id(1)));
        names.insert("doi:10.1000/one".to_owned(), Resolved::One(id(1)));
        names.insert("pmid:2".to_owned(), Resolved::One(id(2)));
        names.insert("pmid:3".to_owned(), Resolved::One(id(3)));
        names.insert(
            "doi:10.1000/shared".to_owned(),
            Resolved::Shared(vec![id(2), id(3)]),
        );
        names.insert(
            "pmid:9".to_owned(),
            Resolved::Split {
                id: id(9),
                into: vec![id(2), id(3)],
            },
        );
        for n in [1, 2, 3, 9] {
            names.insert(public_text(n), Resolved::One(id(n)));
        }
        let mut rows = BTreeMap::new();
        rows.insert(id(1), row(1, &["pmid:1", "doi:10.1000/one"], false));
        rows.insert(id(2), row(2, &["pmid:2", "doi:10.1000/shared"], true));
        rows.insert(id(3), row(3, &["pmid:3", "doi:10.1000/shared"], true));
        Self {
            epoch: Manifest {
                epoch,
                batch: epoch,
                at: format!("2026-09-16T00:00:0{epoch}Z"),
                merged: "v1.1".into(),
                artifacts: BTreeMap::new(),
            },
            rows,
            names,
        }
    }

    /// A whole list in one orientation, which is what an expansion and a
    /// path read; the pages are slices of the same lists.
    fn arcs(&self, work: WorkId, direction: Direction) -> Vec<graph::Arc> {
        self.page(work, direction, 0, usize::MAX).arcs
    }
}

impl Answers for Fake {
    fn epoch(&self) -> &Manifest {
        &self.epoch
    }

    fn watermark(&self) -> serve::Watermark {
        serve::Watermark {
            batch: self.epoch.batch,
            at: self.epoch.at.clone(),
        }
    }

    fn counts(&self) -> Counts {
        Counts {
            merged: "v1.1".into(),
            nodes: 9,
            works: 3,
            arcs: 3,
        }
    }

    fn resolve(&self, text: &str) -> Result<Option<Resolved>, serve::Error> {
        if text.starts_with("ol:") {
            let canonical = merge::public::canonical(text)
                .map_err(|e| serve::Error::NotAName(e.to_string()))?;
            return Ok(self.names.get(&canonical).cloned());
        }
        merge::Name::parse(text).map_err(|e| serve::Error::NotAName(e.to_string()))?;
        Ok(self.names.get(text).cloned())
    }

    fn work(&self, id: WorkId) -> Result<Option<Row>, serve::Error> {
        Ok(self.rows.get(&id).cloned())
    }

    fn rows(&self, ids: &[WorkId]) -> Result<HashMap<WorkId, Row>, serve::Error> {
        Ok(ids
            .iter()
            .filter_map(|id| self.rows.get(id).map(|row| (*id, row.clone())))
            .collect())
    }

    fn public_of(&self, id: WorkId) -> Option<String> {
        (id.get() <= 9).then(|| public_text(id.get()))
    }

    fn page(&self, work: WorkId, direction: Direction, offset: u64, n: usize) -> Page {
        let by: Sources = [SourceId::Pubmed, SourceId::Crossref].into_iter().collect();
        let all: Vec<graph::Arc> = match (work.get(), direction) {
            (1, Direction::References) => vec![
                graph::Arc { work: id(2), by },
                graph::Arc { work: id(3), by },
            ],
            (1, Direction::Citers) => vec![graph::Arc { work: id(3), by }],
            (2 | 3, Direction::Citers) | (3, Direction::References) => {
                vec![graph::Arc { work: id(1), by }]
            }
            _ => Vec::new(),
        };
        let total = all.len() as u64;
        let arcs: Vec<graph::Arc> = all
            .into_iter()
            .skip(usize::try_from(offset).unwrap())
            .take(n)
            .collect();
        Page {
            by: if arcs.is_empty() { Sources::NONE } else { by },
            arcs,
            total,
        }
    }

    fn search(&self, text: &str, limit: usize, _facets: usize) -> Result<Hits, serve::Error> {
        let hits = if text.contains("titled") {
            vec![search::Hit {
                score: 1.5,
                id: id(1),
                title: Some("A titled work".into()),
            }]
        } else {
            Vec::new()
        };
        Ok(Hits {
            total: hits.len(),
            hits: hits.into_iter().take(limit).collect(),
            facets: vec![("/year".into(), vec![("2026".into(), 1)])],
        })
    }

    // A seed count and a degree the fixture keeps in single digits.
    #[allow(clippy::cast_precision_loss)]
    fn expand(
        &self,
        seeds: &[WorkId],
        walk: Walk,
        budget: usize,
        about: Option<WorkId>,
    ) -> Expansion {
        let distinct: std::collections::BTreeSet<WorkId> = seeds.iter().copied().collect();
        let mut touched: BTreeMap<WorkId, u32> = BTreeMap::new();
        for seed in &distinct {
            let mut reached: Vec<WorkId> = Vec::new();
            for direction in [Direction::References, Direction::Citers] {
                if matches!(
                    (walk, direction),
                    (Walk::References, Direction::Citers) | (Walk::Citers, Direction::References)
                ) {
                    continue;
                }
                reached.extend(self.arcs(*seed, direction).into_iter().map(|a| a.work));
            }
            reached.sort_unstable();
            reached.dedup();
            for work in reached {
                if !distinct.contains(&work) && about != Some(work) {
                    *touched.entry(work).or_default() += 1;
                }
            }
        }
        let degree = |work: WorkId| match walk {
            Walk::References => self.arcs(work, Direction::Citers).len(),
            Walk::Citers => self.arcs(work, Direction::References).len(),
            Walk::Both => {
                self.arcs(work, Direction::Citers).len()
                    + self.arcs(work, Direction::References).len()
            }
        } as u64;
        let total = touched.len() as u64;
        let mut out: Vec<Neighbour> = touched
            .into_iter()
            .map(|(work, touches)| {
                let degree = degree(work);
                Neighbour {
                    work,
                    touches,
                    degree,
                    score: f64::from(touches)
                        / (distinct.len() as f64 * degree.max(1) as f64).sqrt(),
                }
            })
            .collect();
        out.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.work.cmp(&b.work))
        });
        out.truncate(budget);
        Expansion {
            neighbours: out,
            total,
            seeds_used: distinct.len() as u64,
            seeds_total: distinct.len() as u64,
        }
    }

    /// The seeds gathered here as the core gathers them, the fixture's
    /// lists being short enough that no cap ever bites.
    fn related(&self, work: WorkId, by: By, budget: usize) -> Expansion {
        let seeds: Vec<WorkId> = self
            .arcs(
                work,
                match by {
                    By::CitesSame => Direction::References,
                    By::CitedAlongside => Direction::Citers,
                },
            )
            .into_iter()
            .map(|a| a.work)
            .filter(|seed| *seed != work)
            .collect();
        self.expand(&seeds, by.walk(), budget, Some(work))
    }

    fn path(&self, from: WorkId, to: WorkId, bounds: graph::Bounds) -> graph::Found {
        let walk = |direction| {
            move |work: WorkId, sink: &mut dyn FnMut(WorkId, Sources)| {
                for arc in self.arcs(work, direction) {
                    sink(arc.work, arc.by);
                }
            }
        };
        graph::path::find(
            from,
            to,
            bounds,
            walk(Direction::References),
            walk(Direction::Citers),
        )
    }
}
