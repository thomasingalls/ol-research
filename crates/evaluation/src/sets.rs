//! The published sets (evaluation plan §3.2, E3): each set's own files
//! read into queries and judgments. A file that is not the shape its set
//! publishes stops the read, naming what was found.
//!
//! A set names works by PMID; [`key_of`] turns its judgments into an
//! answer key through whatever finds a PMID's work, and counts what the
//! corpus holds of them (evaluation plan §3.2, coverage).

use std::collections::BTreeMap;
use std::fmt;
use std::io::Read;

use quick_xml::events::Event;
use quick_xml::{Reader, XmlVersion, escape};
use serde::Deserialize;

use crate::trec::Key;

/// A set's file that is not what the set publishes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error(pub String);

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

fn error(what: impl Into<String>) -> Error {
    Error(what.into())
}

/// One TREC Precision Medicine topic, scientific-abstracts task.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PmTopic {
    /// The topic's number, the answer key's query id.
    pub number: String,
    #[allow(missing_docs)]
    pub disease: String,
    #[allow(missing_docs)]
    pub gene: String,
    /// 2020's topics each ask about one treatment.
    pub treatment: Option<String>,
    /// 2017–2019's: a patient's age and sex. Not in the query.
    pub demographic: Option<String>,
    /// 2017's free-text extra. Not in the query.
    pub other: Option<String>,
}

impl PmTopic {
    /// The query: what a person would type for the topic (the user's go,
    /// 2026-09-26; evaluation plan §12, item 15) — the disease and the
    /// gene, and the treatment where the topic names one.
    #[must_use]
    pub fn query(&self) -> String {
        let mut words = vec![self.disease.as_str(), self.gene.as_str()];
        if let Some(treatment) = &self.treatment {
            words.push(treatment);
        }
        words.join(" ")
    }
}

/// A TREC Precision Medicine topics file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PmTopics {
    /// The year its `task` attribute names: `2019 TREC Precision Medicine`.
    pub year: u16,
    #[allow(missing_docs)]
    pub topics: Vec<PmTopic>,
}

/// Reads a TREC Precision Medicine topics file: `<topics task="…">`
/// holding `<topic number="…">`s, each with `disease` and `gene` and any
/// of `treatment`, `demographic`, `other`, once each.
///
/// # Errors
///
/// Anything else: another element or attribute, text outside a field, a
/// field twice or missing, a topic number twice, no year in the task.
///
/// # Panics
///
/// Never: a field is read only at the depth inside a topic, where the
/// topic being read is always open.
// One state machine over the file's events, long together.
#[allow(clippy::too_many_lines)]
pub fn read_pm_topics(xml: &str) -> Result<PmTopics, Error> {
    let mut reader = Reader::from_str(xml);
    let mut year = None;
    let mut topics: Vec<PmTopic> = Vec::new();
    let mut open: Option<(String, BTreeMap<String, String>)> = None;
    let mut depth = 0;
    loop {
        let event = reader
            .read_event()
            .map_err(|e| error(format!("at byte {}: {e}", reader.buffer_position())))?;
        match event {
            Event::Decl(_) | Event::Comment(_) => {}
            Event::Text(t) if t.trim().is_empty() => {}
            Event::Start(s) => {
                let name = s.name().as_ref().to_owned();
                let attribute = |wanted: &str| -> Result<String, Error> {
                    let mut value = None;
                    for attr in s.attributes() {
                        let attr = attr.map_err(|e| error(format!("<{name}>: {e}")))?;
                        let key = attr.key.as_ref().to_owned();
                        if key != wanted {
                            return Err(error(format!("<{name}> has attribute {key:?}")));
                        }
                        value = Some(
                            attr.normalized_value(XmlVersion::Explicit1_0)
                                .map_err(|e| error(format!("<{name} {key}>: {e}")))?
                                .into_owned(),
                        );
                    }
                    value.ok_or_else(|| error(format!("<{name}> has no {wanted:?}")))
                };
                match (depth, name.as_str()) {
                    (0, "topics") => {
                        let task = attribute("task")?;
                        let digits = task
                            .get(..4)
                            .filter(|d| d.bytes().all(|b| b.is_ascii_digit()));
                        year = Some(
                            digits
                                .and_then(|d| d.parse::<u16>().ok())
                                .ok_or_else(|| error(format!("task {task:?} names no year")))?,
                        );
                        depth = 1;
                    }
                    (1, "topic") => {
                        let number = attribute("number")?;
                        if topics.iter().any(|t| t.number == number) {
                            return Err(error(format!("topic {number} twice")));
                        }
                        open = Some((number, BTreeMap::new()));
                        depth = 2;
                    }
                    (2, "disease" | "gene" | "treatment" | "demographic" | "other") => {
                        if s.attributes().next().is_some() {
                            return Err(error(format!("<{name}> has attributes")));
                        }
                        let span = reader
                            .read_to_end(s.name())
                            .map_err(|e| error(format!("<{name}>: {e}")))?;
                        let raw = &xml[offset(span.start)..offset(span.end)];
                        if raw.contains('<') {
                            return Err(error(format!("<{name}> holds markup")));
                        }
                        let text = escape::unescape(raw)
                            .map_err(|e| error(format!("<{name}>: {e}")))?
                            .trim()
                            .to_owned();
                        let (number, fields) = open.as_mut().expect("depth 2 is inside a topic");
                        if fields.insert(name.clone(), text).is_some() {
                            return Err(error(format!("topic {number}: <{name}> twice")));
                        }
                    }
                    _ => return Err(error(format!("<{name}> where it does not belong"))),
                }
            }
            Event::End(e) => match (depth, e.name().as_ref()) {
                (2, "topic") => {
                    let (number, mut fields) = open.take().expect("depth 2 is inside a topic");
                    let mut take = |field: &str| fields.remove(field);
                    let disease = take("disease")
                        .ok_or_else(|| error(format!("topic {number} has no disease")))?;
                    let gene =
                        take("gene").ok_or_else(|| error(format!("topic {number} has no gene")))?;
                    topics.push(PmTopic {
                        disease,
                        gene,
                        treatment: take("treatment"),
                        demographic: take("demographic"),
                        other: take("other"),
                        number,
                    });
                    depth = 1;
                }
                (1, "topics") => depth = 0,
                (_, other) => {
                    return Err(error(format!("</{other}> where it does not belong")));
                }
            },
            Event::Eof => break,
            other => return Err(error(format!("unexpected {other:?}"))),
        }
    }
    if depth != 0 {
        return Err(error("the file ends inside an element"));
    }
    let year = year.ok_or_else(|| error("no <topics>"))?;
    Ok(PmTopics { year, topics })
}

fn offset(position: u64) -> usize {
    usize::try_from(position).unwrap_or(usize::MAX)
}

/// One RELISH annotation: a seed paper and its 60 candidates as one
/// person graded them.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RelishRecord {
    #[allow(missing_docs)]
    pub uid: String,
    /// The seed's PMID.
    pub pmid: String,
    #[allow(missing_docs)]
    pub response: RelishResponse,
    /// The annotator's standing, as RELISH records it.
    pub experience: String,
    #[allow(missing_docs)]
    pub is_anonymous: bool,
}

/// The candidates by grade.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RelishResponse {
    #[allow(missing_docs)]
    pub relevant: Vec<String>,
    #[allow(missing_docs)]
    pub partial: Vec<String>,
    #[allow(missing_docs)]
    pub irrelevant: Vec<String>,
}

impl RelishRecord {
    /// Every candidate with its grade: relevant 2, partial 1, irrelevant 0.
    #[must_use]
    pub fn judgments(&self) -> Vec<(&str, i64)> {
        let r = &self.response;
        r.relevant
            .iter()
            .map(|p| (p.as_str(), 2))
            .chain(r.partial.iter().map(|p| (p.as_str(), 1)))
            .chain(r.irrelevant.iter().map(|p| (p.as_str(), 0)))
            .collect()
    }
}

/// Reads RELISH's JSON: an array of annotations.
///
/// # Errors
///
/// JSON that is not RELISH's shape, field for field.
pub fn read_relish(json: &str) -> Result<Vec<RelishRecord>, Error> {
    serde_json::from_str(json).map_err(|e| error(format!("RELISH: {e}")))
}

/// Reads RELISH's published file, `RELISH_v1.json.gz`.
///
/// # Errors
///
/// A gzip stream that does not decode, or [`read_relish`]'s.
pub fn read_relish_gz(bytes: &[u8]) -> Result<Vec<RelishRecord>, Error> {
    let mut json = String::new();
    flate2::read::GzDecoder::new(bytes)
        .read_to_string(&mut json)
        .map_err(|e| error(format!("RELISH: {e}")))?;
    read_relish(&json)
}

/// One annotation per seed: the first in the file's order. A seed
/// several people annotated keeps its first, and the others' `uid`s are
/// returned (evaluation plan §12: one judgment per work).
#[must_use]
pub fn first_per_seed(records: Vec<RelishRecord>) -> (Vec<RelishRecord>, Vec<String>) {
    let mut seen = std::collections::HashSet::new();
    let mut kept = Vec::new();
    let mut dropped = Vec::new();
    for record in records {
        if seen.insert(record.pmid.clone()) {
            kept.push(record);
        } else {
            dropped.push(record.uid);
        }
    }
    (kept, dropped)
}

/// What the corpus holds of a PMID a set judged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Found {
    /// A work with a record, so in the index: its public id.
    Held(String),
    /// A work with no record, known only as cited, so never returned by
    /// a search: its public id.
    Stub(String),
    /// No work carries the PMID.
    Missing,
    /// The PMID names more than one work, or one a split retired.
    Ambiguous,
}

/// How much of a key the corpus holds, by grade: judged works held,
/// stubs, missing and ambiguous; and judgments of one query that landed
/// on the same work twice.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Coverage {
    /// By grade: `[held, stub, missing, ambiguous]`.
    pub by_grade: BTreeMap<i64, [usize; 4]>,
    /// Two PMIDs of one query found as one work: the higher grade kept.
    pub collided: usize,
}

/// An answer key from `(query, PMID, grade)` judgments, each PMID named
/// by the public id of the work `find` finds for it, or kept as
/// `pmid:<n>` when there is none — so the key's relevant works stay as
/// many as the set judged, and one the corpus lacks counts as missed.
///
/// # Errors
///
/// A PMID that is not digits, or whatever `find` fails with.
pub fn key_of<'a, E: fmt::Display>(
    judgments: impl IntoIterator<Item = (&'a str, &'a str, i64)>,
    mut find: impl FnMut(&str) -> Result<Found, E>,
) -> Result<(Key, Coverage), Error> {
    let mut key = Key::default();
    let mut coverage = Coverage::default();
    for (query, pmid, grade) in judgments {
        if pmid.is_empty() || !pmid.bytes().all(|b| b.is_ascii_digit()) {
            return Err(error(format!("query {query}: PMID {pmid:?} is not digits")));
        }
        let found = find(pmid).map_err(|e| error(format!("PMID {pmid}: {e}")))?;
        let (slot, work) = match found {
            Found::Held(public) => (0, public),
            Found::Stub(public) => (1, public),
            Found::Missing => (2, format!("pmid:{pmid}")),
            Found::Ambiguous => (3, format!("pmid:{pmid}")),
        };
        coverage.by_grade.entry(grade).or_default()[slot] += 1;
        let judged = key.queries.entry(query.to_owned()).or_default();
        match judged.get(&work) {
            Some(before) => {
                coverage.collided += 1;
                if grade > *before {
                    judged.insert(work, grade);
                }
            }
            None => {
                judged.insert(work, grade);
            }
        }
    }
    Ok((key, coverage))
}

#[cfg(test)]
mod tests {
    use super::*;

    const PM_2019: &str = include_str!("../fixtures/trec-pm-2019-topics.xml");
    const PM_2020: &str = include_str!("../fixtures/trec-pm-2020-topics.xml");
    const RELISH: &str = include_str!("../fixtures/relish-v1.json");

    #[test]
    fn pm_2019_reads_its_topics_and_queries() {
        let got = read_pm_topics(PM_2019).unwrap();
        assert_eq!(got.year, 2019);
        let numbers: Vec<&str> = got.topics.iter().map(|t| t.number.as_str()).collect();
        assert_eq!(numbers, vec!["1", "2", "40"]);
        assert_eq!(got.topics[0].query(), "melanoma BRAF (E586K)");
        assert_eq!(
            got.topics[0].demographic.as_deref(),
            Some("64-year-old female")
        );
        assert_eq!(got.topics[2].query(), "malignant hyperthermia RYR1");
    }

    #[test]
    fn pm_2020_puts_the_treatment_in_the_query() {
        let got = read_pm_topics(PM_2020).unwrap();
        assert_eq!(got.year, 2020);
        assert_eq!(got.topics[0].query(), "colorectal cancer ABL1 Regorafenib");
        assert_eq!(got.topics[1].treatment.as_deref(), Some("Alectinib"));
        assert_eq!(got.topics[1].demographic, None);
    }

    #[test]
    fn pm_topics_refuse_what_the_set_does_not_publish() {
        let wrap =
            |inner: &str| format!("<topics task=\"2019 TREC Precision Medicine\">{inner}</topics>");
        let cases = [
            wrap("<topic number=\"1\"><gene>BRAF</gene></topic>"),
            wrap(
                "<topic number=\"1\"><disease>a</disease><disease>b</disease><gene>g</gene></topic>",
            ),
            wrap("<topic number=\"1\"><disease>a</disease><gene>g</gene><notes>x</notes></topic>"),
            wrap(
                "<topic number=\"1\"><disease>a</disease><gene>g</gene></topic><topic number=\"1\"><disease>a</disease><gene>g</gene></topic>",
            ),
            wrap("<topic number=\"1\">loose text<disease>a</disease><gene>g</gene></topic>"),
            wrap("<topic id=\"1\"><disease>a</disease><gene>g</gene></topic>"),
            "<topics task=\"TREC Precision Medicine\"></topics>".to_owned(),
            wrap("<topic number=\"1\"><disease>a</disease><gene>g</gene>"),
        ];
        for case in &cases {
            assert!(read_pm_topics(case).is_err(), "{case}");
        }
    }

    #[test]
    fn pm_topic_text_is_unescaped() {
        let xml = "<topics task=\"2019 x\"><topic number=\"7\"><disease>A &amp; B</disease><gene> g </gene></topic></topics>";
        let got = read_pm_topics(xml).unwrap();
        assert_eq!(got.topics[0].query(), "A & B g");
    }

    #[test]
    fn relish_reads_and_keeps_one_annotation_per_seed() {
        let records = read_relish(RELISH).unwrap();
        assert_eq!(records.len(), 3);
        assert_eq!(records[0].pmid, "22569528");
        assert_eq!(records[0].judgments().len(), 60);
        // Seed 29463761 was annotated twice, by different people who
        // graded the same 60 candidates differently.
        assert_eq!(records[1].pmid, "29463761");
        assert_eq!(records[2].pmid, "29463761");
        assert_ne!(records[1].response, records[2].response);
        let (kept, dropped) = first_per_seed(records);
        assert_eq!(kept.len(), 2);
        assert_eq!(kept[1].uid, "5b13f254fdb55a4a783f6375");
        assert_eq!(dropped, vec!["5b14c68ffdb55a8d26a6860d".to_owned()]);
    }

    #[test]
    fn relish_refuses_another_shape() {
        assert!(read_relish(r#"[{"uid":"u","pmid":"1","response":{"relevant":[],"partial":[],"irrelevant":[]},"experience":"x","is_anonymous":false,"extra":1}]"#).is_err());
        assert!(read_relish(r#"[{"uid":"u","pmid":"1","response":{"relevant":[]},"experience":"x","is_anonymous":false}]"#).is_err());
    }

    #[test]
    fn a_key_names_what_it_finds_and_keeps_what_it_does_not() {
        let judgments = [
            ("1", "100", 2),
            ("1", "200", 0),
            ("1", "300", 1),
            ("1", "400", 2),
            ("2", "100", 1),
            // 500 is the same work as 100: the higher grade stays.
            ("1", "500", 1),
        ];
        let (key, coverage) = key_of(judgments, |pmid| {
            Ok::<_, String>(match pmid {
                "100" | "500" => Found::Held("ol:A".to_owned()),
                "200" => Found::Stub("ol:B".to_owned()),
                "300" => Found::Missing,
                _ => Found::Ambiguous,
            })
        })
        .unwrap();
        assert_eq!(key.queries["1"]["ol:A"], 2);
        assert_eq!(key.queries["1"]["ol:B"], 0);
        assert_eq!(key.queries["1"]["pmid:300"], 1);
        assert_eq!(key.queries["1"]["pmid:400"], 2);
        assert_eq!(key.queries["2"]["ol:A"], 1);
        assert_eq!(coverage.collided, 1);
        assert_eq!(coverage.by_grade[&2], [1, 0, 0, 1]);
        assert_eq!(coverage.by_grade[&1], [2, 0, 1, 0]);
        assert_eq!(coverage.by_grade[&0], [0, 1, 0, 0]);
        assert!(key_of([("1", "12a", 1)], |_| Ok::<_, String>(Found::Missing)).is_err());
        assert!(key_of([("1", "12", 1)], |_| Err("no public id")).is_err());
    }
}
