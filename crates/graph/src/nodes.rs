//! What the works set says about every id (graph plan §8): whether it is
//! a work at all — an id retired by a merge or a split has none — whether
//! it is held or a stub, and which sources hold a record of it. The build
//! checks every edge's ends against it, `--held` filters through it, and
//! the report conditions agreement on it.

use std::path::Path;

use ingest::Kind;
use merge::record::Work;
use merge::{Sources, WorkId};

use crate::error::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Absent,
    Held,
    Stub,
}

#[derive(Debug, Clone, Copy)]
struct Node {
    state: State,
    by: Sources,
}

const ABSENT: Node = Node {
    state: State::Absent,
    by: Sources::NONE,
};

/// One entry per id from 1 to the highest in the works set.
#[derive(Debug, Clone, Default)]
pub struct Nodes {
    states: Vec<Node>,
    works: u64,
    held: u64,
    stubs: u64,
}

impl Nodes {
    /// No works.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Reads every works set under a merged directory.
    ///
    /// # Errors
    ///
    /// I/O, a set that is not a merge's, or a work seen twice.
    pub fn read(merged: &Path) -> Result<Self, Error> {
        let mut nodes = Self::new();
        for path in merge::sets::find(merged, Kind::Works).map_err(|e| Error::io(merged, e))? {
            let reader = merge::layout::open(&path).map_err(|e| Error::io(&path, e))?;
            reader.with_table(|| -> Result<(), Error> {
                for row in reader.records::<Work<'_>>() {
                    let work = row.map_err(|e| Error::io(&path, e))?;
                    let by = work.records.iter().map(|r| r.source).collect();
                    nodes.add(work.id, work.stub, by)?;
                }
                Ok(())
            })?;
        }
        Ok(nodes)
    }

    /// Adds one work; ids may arrive in any order.
    ///
    /// # Errors
    ///
    /// The id was added before.
    pub fn add(&mut self, id: WorkId, stub: bool, by: Sources) -> Result<(), Error> {
        let ix = index(id);
        if ix >= self.states.len() {
            self.states.resize(ix + 1, ABSENT);
        }
        if self.states[ix].state != State::Absent {
            return Err(Error::DuplicateWork { id });
        }
        self.states[ix] = Node {
            state: if stub { State::Stub } else { State::Held },
            by,
        };
        self.works += 1;
        if stub {
            self.stubs += 1;
        } else {
            self.held += 1;
        }
        Ok(())
    }

    /// The highest id, which is the graph's node count.
    #[must_use]
    pub fn len(&self) -> u32 {
        u32::try_from(self.states.len()).unwrap_or(u32::MAX)
    }

    /// No works at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.states.is_empty()
    }

    /// Works, held and stubs; the ids up to [`len`](Self::len) that are
    /// neither were retired.
    #[must_use]
    pub const fn works(&self) -> u64 {
        self.works
    }

    /// Works with records.
    #[must_use]
    pub const fn held(&self) -> u64 {
        self.held
    }

    /// Works with names and no records.
    #[must_use]
    pub const fn stubs(&self) -> u64 {
        self.stubs
    }

    fn get(&self, id: WorkId) -> Node {
        self.states.get(index(id)).copied().unwrap_or(ABSENT)
    }

    /// Whether the id is a work.
    #[must_use]
    pub fn present(&self, id: WorkId) -> bool {
        self.get(id).state != State::Absent
    }

    /// Whether the id is a work with records.
    #[must_use]
    pub fn is_held(&self, id: WorkId) -> bool {
        self.get(id).state == State::Held
    }

    /// Whether the id is a stub.
    #[must_use]
    pub fn is_stub(&self, id: WorkId) -> bool {
        self.get(id).state == State::Stub
    }

    /// The sources holding a record of the work; none for a stub or an
    /// absent id.
    #[must_use]
    pub fn held_by(&self, id: WorkId) -> Sources {
        self.get(id).by
    }
}

/// The index of an id: ids are one-based, positions are not.
#[must_use]
pub fn index(id: WorkId) -> usize {
    usize::try_from(id.get() - 1).unwrap_or(usize::MAX)
}

#[cfg(test)]
mod tests {
    use merge::{SourceId, Sources, WorkId};

    use super::Nodes;

    fn id(n: u32) -> WorkId {
        WorkId::new(n).unwrap()
    }

    #[test]
    fn nodes_know_presence_kind_and_holders() {
        let mut nodes = Nodes::new();
        let by: Sources = [SourceId::Pubmed, SourceId::Crossref].into_iter().collect();
        nodes.add(id(3), false, by).unwrap();
        nodes.add(id(1), true, Sources::NONE).unwrap();
        assert_eq!(nodes.len(), 3);
        assert_eq!((nodes.works(), nodes.held(), nodes.stubs()), (2, 1, 1));
        assert!(nodes.present(id(1)) && nodes.is_stub(id(1)));
        assert!(!nodes.present(id(2)));
        assert!(nodes.is_held(id(3)) && nodes.held_by(id(3)) == by);
        assert!(!nodes.present(id(4)));
        assert!(nodes.add(id(3), true, Sources::NONE).is_err());
    }
}
