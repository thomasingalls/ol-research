//! The registry (merge plan §4): the one part of the merged layer that is
//! of record. A rebuild can recompute everything else from the normalized
//! sets; what it cannot recompute is which [`WorkId`] a work was given
//! last time. So that is what this log keeps, append-only like receipts:
//! one JSON line per event, folded into a table at the start of a run,
//! never reused.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{self, BufRead, BufReader, Write};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::name::OwnedName;
use crate::work::WorkId;

/// One event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "lowercase", deny_unknown_fields)]
pub enum Line {
    /// A new id, with the names that earned it. Ids are dense: each mint
    /// is the number after the last.
    Mint {
        #[allow(missing_docs)]
        id: WorkId,
        #[allow(missing_docs)]
        names: Vec<OwnedName>,
        #[allow(missing_docs)]
        at: String,
    },
    /// A name attached to an existing id. The latest line for a name
    /// wins, which is how a name moves between ids.
    Name {
        #[allow(missing_docs)]
        id: WorkId,
        #[allow(missing_docs)]
        name: OwnedName,
        #[allow(missing_docs)]
        at: String,
    },
    /// A name no work claims any more.
    Drop {
        #[allow(missing_docs)]
        name: OwnedName,
        #[allow(missing_docs)]
        at: String,
    },
    /// An id retired into an older one: the two turned out to be one
    /// work. Lookups follow it.
    Merge {
        #[allow(missing_docs)]
        id: WorkId,
        #[allow(missing_docs)]
        into: WorkId,
        #[allow(missing_docs)]
        at: String,
    },
    /// An id retired because its names split between works, each minted
    /// anew; the id has no successor.
    Split {
        #[allow(missing_docs)]
        id: WorkId,
        #[allow(missing_docs)]
        into: Vec<WorkId>,
        #[allow(missing_docs)]
        at: String,
    },
}

/// The log, folded: which names map to which ids, which ids are retired.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Registry {
    minted: u32,
    names: HashMap<OwnedName, WorkId>,
    retired: HashMap<WorkId, Option<WorkId>>,
    lines: u64,
}

/// What one run's assignment did.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Assignment {
    /// One id per work given, in order.
    pub ids: Vec<WorkId>,
    /// The lines to append.
    pub lines: Vec<Line>,
    #[allow(missing_docs)]
    pub minted: u64,
    #[allow(missing_docs)]
    pub reused: u64,
    /// Ids retired into an older one.
    pub merged: u64,
    /// Ids retired because their names split.
    pub split: u64,
    /// Names attached to an existing id.
    pub attached: u64,
    #[allow(missing_docs)]
    pub dropped: u64,
}

impl Registry {
    /// The log at `path`, folded; empty when there is no file yet.
    ///
    /// # Errors
    ///
    /// I/O, a line that is not an event, or an event the fold refuses
    /// (with its line number): the log is of record and must be
    /// consistent.
    pub fn read(path: &Path) -> io::Result<Self> {
        let file = match fs::File::open(path) {
            Ok(file) => file,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => return Err(e),
        };
        let mut registry = Self::default();
        let bad = |number: usize, what: String| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{}: line {number}: {what}", path.display()),
            )
        };
        for (index, line) in BufReader::new(file).lines().enumerate() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let event: Line =
                serde_json::from_str(&line).map_err(|e| bad(index + 1, e.to_string()))?;
            registry
                .apply(&event)
                .map_err(|what| bad(index + 1, what))?;
        }
        for (name, id) in &registry.names {
            if registry.live(*id).is_none() {
                return Err(bad(0, format!("{name} maps to {id}, which was split")));
            }
        }
        Ok(registry)
    }

    /// Folds one event.
    ///
    /// # Errors
    ///
    /// The event does not fit what came before: an id out of sequence, a
    /// name on an unknown or retired id, a merge into a younger id.
    pub fn apply(&mut self, line: &Line) -> Result<(), String> {
        match line {
            Line::Mint { id, names, .. } => {
                if id.get() != self.minted + 1 {
                    return Err(format!("mint of {id} after {} ids", self.minted));
                }
                self.minted += 1;
                for name in names {
                    self.names.insert(name.clone(), *id);
                }
            }
            Line::Name { id, name, .. } => {
                self.check_live(*id)?;
                self.names.insert(name.clone(), *id);
            }
            Line::Drop { name, .. } => {
                if self.names.remove(name).is_none() {
                    return Err(format!("drop of {name}, which is not mapped"));
                }
            }
            Line::Merge { id, into, .. } => {
                self.check_live(*id)?;
                self.check_live(*into)?;
                if into >= id {
                    return Err(format!("merge of {id} into {into}, which is not older"));
                }
                self.retired.insert(*id, Some(*into));
            }
            Line::Split { id, into, .. } => {
                self.check_live(*id)?;
                for part in into {
                    self.check_live(*part)?;
                }
                self.retired.insert(*id, None);
            }
        }
        self.lines += 1;
        Ok(())
    }

    fn check_live(&self, id: WorkId) -> Result<(), String> {
        if id.get() > self.minted {
            return Err(format!("{id} was never minted"));
        }
        if self.retired.contains_key(&id) {
            return Err(format!("{id} is retired"));
        }
        Ok(())
    }

    /// Appends events, one line each, flushed, creating the file and its
    /// directory if needed.
    ///
    /// # Errors
    ///
    /// I/O.
    pub fn append(path: &Path, lines: &[Line]) -> io::Result<()> {
        if lines.is_empty() {
            return Ok(());
        }
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let mut file = io::BufWriter::new(
            fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)?,
        );
        for line in lines {
            serde_json::to_writer(&mut file, line)?;
            file.write_all(b"\n")?;
        }
        file.flush()
    }

    /// Ids minted so far.
    #[must_use]
    pub const fn minted(&self) -> u32 {
        self.minted
    }

    /// Names mapped.
    #[must_use]
    pub fn names(&self) -> usize {
        self.names.len()
    }

    /// Events folded.
    #[must_use]
    pub const fn lines(&self) -> u64 {
        self.lines
    }

    /// The live id `id` stands for: itself, the id it was merged into
    /// (transitively), or `None` when it was split.
    #[must_use]
    pub fn live(&self, mut id: WorkId) -> Option<WorkId> {
        loop {
            match self.retired.get(&id) {
                None => return Some(id),
                Some(Some(into)) => id = *into,
                Some(None) => return None,
            }
        }
    }

    /// The live id a name maps to.
    #[must_use]
    pub fn lookup(&self, name: &OwnedName) -> Option<WorkId> {
        self.names.get(name).and_then(|id| self.live(*id))
    }

    /// Gives every work an id (merge plan §4): each work's names, given in
    /// the deterministic order the plan asks for, are looked up; none
    /// known mints, one id reuses, several merge into the oldest, an id
    /// two works claim is split and both are minted. With `complete`, a
    /// name the registry maps that no work here has is dropped, so
    /// `complete` is only for a run over the whole store.
    ///
    /// # Panics
    ///
    /// Never: every line it applies is one the fold accepts by
    /// construction.
    #[must_use]
    pub fn assign(&mut self, works: &[Vec<OwnedName>], at: &str, complete: bool) -> Assignment {
        let mut out = Assignment::default();
        let claims: Vec<Vec<WorkId>> = works
            .iter()
            .map(|names| {
                let mut ids: Vec<WorkId> = names.iter().filter_map(|n| self.lookup(n)).collect();
                ids.sort_unstable();
                ids.dedup();
                ids
            })
            .collect();
        let mut claimants: HashMap<WorkId, usize> = HashMap::new();
        for ids in &claims {
            for id in ids {
                *claimants.entry(*id).or_default() += 1;
            }
        }
        let mut split_into: HashMap<WorkId, Vec<WorkId>> = HashMap::new();
        for (w, names) in works.iter().enumerate() {
            let unique: Vec<WorkId> = claims[w]
                .iter()
                .copied()
                .filter(|id| claimants[id] == 1)
                .collect();
            let id = match unique.as_slice() {
                [] => {
                    let id = WorkId::new(self.minted + 1).expect("ids fit u32");
                    let line = Line::Mint {
                        id,
                        names: names.clone(),
                        at: at.to_owned(),
                    };
                    self.apply(&line).expect("a mint after the last id");
                    out.lines.push(line);
                    out.minted += 1;
                    id
                }
                [id] => {
                    out.reused += 1;
                    *id
                }
                [winner, losers @ ..] => {
                    for loser in losers {
                        let line = Line::Merge {
                            id: *loser,
                            into: *winner,
                            at: at.to_owned(),
                        };
                        self.apply(&line).expect("both live, winner older");
                        out.lines.push(line);
                        out.merged += 1;
                    }
                    out.reused += 1;
                    *winner
                }
            };
            for name in names {
                if self.lookup(name) != Some(id) {
                    let line = Line::Name {
                        id,
                        name: name.clone(),
                        at: at.to_owned(),
                    };
                    self.apply(&line).expect("the id is live");
                    out.lines.push(line);
                    out.attached += 1;
                }
            }
            for claimed in &claims[w] {
                if claimants[claimed] > 1 {
                    split_into.entry(*claimed).or_default().push(id);
                }
            }
            out.ids.push(id);
        }
        let mut splits: Vec<(WorkId, Vec<WorkId>)> = split_into.into_iter().collect();
        splits.sort_unstable();
        for (id, mut into) in splits {
            into.sort_unstable();
            into.dedup();
            let line = Line::Split {
                id,
                into,
                at: at.to_owned(),
            };
            self.apply(&line).expect("the parts are live");
            out.lines.push(line);
            out.split += 1;
        }
        if complete {
            let seen: HashSet<&OwnedName> = works.iter().flatten().collect();
            let dropped = self.drop_unseen(|n| seen.contains(n), at);
            out.dropped += dropped.len() as u64;
            out.lines.extend(dropped);
        }
        out
    }

    /// Drops every mapped name `seen` does not vouch for, returning the
    /// lines: the end of a run over the whole store, after every work and
    /// stub was assigned.
    ///
    /// # Panics
    ///
    /// Never: a name taken from the map is mapped.
    pub fn drop_unseen(&mut self, seen: impl Fn(&OwnedName) -> bool, at: &str) -> Vec<Line> {
        let mut gone: Vec<OwnedName> = self.names.keys().filter(|n| !seen(n)).cloned().collect();
        gone.sort();
        let mut lines = Vec::with_capacity(gone.len());
        for name in gone {
            let line = Line::Drop {
                name,
                at: at.to_owned(),
            };
            self.apply(&line).expect("the name is mapped");
            lines.push(line);
        }
        lines
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{Line, Registry};
    use crate::name::OwnedName;
    use crate::work::WorkId;

    fn n(text: &str) -> OwnedName {
        OwnedName::parse(text).unwrap()
    }

    fn id(n: u32) -> WorkId {
        WorkId::new(n).unwrap()
    }

    #[test]
    fn a_fresh_registry_mints_in_order_and_a_rebuild_mints_nothing() {
        let works = vec![
            vec![n("doi:10.1000/a"), n("pmid:1")],
            vec![n("doi:10.1000/b")],
            vec![n("openalex:W3")],
        ];
        let mut r = Registry::default();
        let first = r.assign(&works, "t0", true);
        assert_eq!(first.ids, [id(1), id(2), id(3)]);
        assert_eq!(
            (first.minted, first.reused, first.attached, first.dropped),
            (3, 0, 0, 0)
        );
        assert_eq!(first.lines.len(), 3);
        let again = r.assign(&works, "t1", true);
        assert_eq!(again.ids, first.ids);
        assert!(again.lines.is_empty(), "{:?}", again.lines);
        assert_eq!(again.reused, 3);
        let mut fresh = Registry::default();
        let other = fresh.assign(&works, "t0", true);
        assert_eq!(other.lines, first.lines, "byte-identical from scratch");
    }

    #[test]
    fn names_attach_merge_split_and_drop() {
        let mut r = Registry::default();
        let _ = r.assign(
            &[
                vec![n("doi:10.1000/a")],
                vec![n("pmid:2")],
                vec![n("pmid:3"), n("pmc:PMC3")],
            ],
            "t0",
            true,
        );
        // Work 1 gains a name; works 1 and 2 turn out to be one; work 3 splits.
        let works = vec![
            vec![n("doi:10.1000/a"), n("pmid:2"), n("openalex:W9")],
            vec![n("pmid:3")],
            vec![n("pmc:PMC3")],
        ];
        let a = r.assign(&works, "t1", true);
        assert_eq!(a.ids, [id(1), id(4), id(5)]);
        assert_eq!(
            (a.minted, a.reused, a.merged, a.split, a.attached),
            (2, 1, 1, 1, 1)
        );
        assert_eq!(r.live(id(2)), Some(id(1)));
        assert_eq!(r.live(id(3)), None);
        assert_eq!(r.lookup(&n("pmid:2")), Some(id(1)), "through the merge");
        assert_eq!(r.lookup(&n("pmc:PMC3")), Some(id(5)));
        assert!(
            matches!(a.lines.last(), Some(Line::Split { id: split, into, .. }) if *split == id(3) && *into == [id(4), id(5)])
        );
        // A run without the DOI drops it; the id stays.
        let b = r.assign(&[vec![n("pmid:2")]], "t2", true);
        assert_eq!(b.ids, [id(1)]);
        assert_eq!(b.dropped, 4, "the doi, W9 and the split parts' names");
        assert_eq!(r.lookup(&n("doi:10.1000/a")), None);
        // An incomplete run drops nothing.
        let mut r2 = Registry::default();
        let _ = r2.assign(&[vec![n("pmid:1")], vec![n("pmid:2")]], "t0", true);
        let c = r2.assign(&[vec![n("pmid:1")]], "t1", false);
        assert_eq!(c.dropped, 0);
        assert_eq!(r2.names(), 2);
    }

    #[test]
    fn the_log_reads_back_and_refuses_inconsistency() {
        let dir =
            std::env::temp_dir().join(format!("ol-rht-merge-registry-{}", std::process::id()));
        let path = dir.join("registry.jsonl");
        assert_eq!(
            Registry::read(&path).unwrap(),
            Registry::default(),
            "no file yet"
        );
        let mut r = Registry::default();
        let a = r.assign(
            &[vec![n("doi:10.1000/a")], vec![n("pmid:2")]],
            "2026-09-15T00:00:00Z",
            true,
        );
        Registry::append(&path, &a.lines).unwrap();
        let b = r.assign(
            &[vec![n("doi:10.1000/a"), n("pmid:2")]],
            "2026-09-15T00:00:01Z",
            true,
        );
        Registry::append(&path, &b.lines).unwrap();
        let read = Registry::read(&path).unwrap();
        assert_eq!(read, r);
        assert_eq!(read.minted(), 2);
        assert_eq!(read.lines(), 3);
        let text = fs::read_to_string(&path).unwrap();
        assert!(
            text.contains(
                r#"{"op":"mint","id":1,"names":["doi:10.1000/a"],"at":"2026-09-15T00:00:00Z"}"#
            ),
            "{text}"
        );
        assert!(text.contains(r#"{"op":"merge","id":2,"into":1,"#), "{text}");
        fs::write(
            &path,
            format!("{text}{{\"op\":\"name\",\"id\":2,\"name\":\"pmid:9\",\"at\":\"x\"}}\n"),
        )
        .unwrap();
        let err = Registry::read(&path).unwrap_err().to_string();
        assert!(err.contains("line 4") && err.contains("retired"), "{err}");
        fs::write(&path, r#"{"op":"mint","id":2,"names":[],"at":"x"}"#).unwrap();
        assert!(
            Registry::read(&path)
                .unwrap_err()
                .to_string()
                .contains("after 0 ids")
        );
        fs::write(
            &path,
            r#"{"op":"mint","id":1,"names":["doi:10.1000/a"],"at":"x"}
{"op":"mint","id":2,"names":[],"at":"x"}
{"op":"split","id":1,"into":[2],"at":"x"}
"#,
        )
        .unwrap();
        assert!(
            Registry::read(&path)
                .unwrap_err()
                .to_string()
                .contains("was split")
        );
        fs::remove_dir_all(dir).unwrap();
    }
}
