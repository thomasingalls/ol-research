//! The reader (graph plan §7, §8): the manifest checked, then a memory
//! map and a slice per file. `references(work)` and `citers(work)` are
//! lazy over the mapped list, so a methods paper's hundred thousand
//! citers cost what the caller reads of them; a degree is two offsets.
//! Positions never leave: a `WorkId` goes in and `WorkId`s come out.

use std::fs::File;
use std::path::{Path, PathBuf};

use memmap2::Mmap;
use merge::{Sources, WorkId};

use crate::error::Error;
use crate::layout::{
    FORWARD_LABELS, FORWARD_NEIGHBOURS, FORWARD_OFFSETS, INV, Manifest, PERM, TRANSPOSE_LABELS,
    TRANSPOSE_NEIGHBOURS, TRANSPOSE_OFFSETS,
};
use crate::nodes;

/// One array file, mapped; an empty file maps to nothing.
#[derive(Debug)]
struct Mapped(Option<Mmap>);

impl Mapped {
    fn open(path: &Path, bytes: u64) -> Result<Self, Error> {
        let file = File::open(path).map_err(|e| Error::io(path, e))?;
        let len = file.metadata().map_err(|e| Error::io(path, e))?.len();
        if len != bytes {
            return Err(Error::Mismatch {
                what: format!(
                    "{}: {len} bytes on disk, {bytes} in the manifest",
                    path.display()
                ),
            });
        }
        if len == 0 {
            return Ok(Self(None));
        }
        // SAFETY: a base file is written once by `build` and replaced whole
        // with its directory, never modified in place, and nothing opens it
        // for writing while a `Base` holds it; every access is a bounds-
        // checked slice of plain bytes decoded as little-endian integers.
        let map = unsafe { Mmap::map(&file) }.map_err(|e| Error::io(path, e))?;
        Ok(Self(Some(map)))
    }

    fn bytes(&self) -> &[u8] {
        self.0.as_deref().unwrap_or(&[])
    }
}

#[derive(Debug)]
struct Orientation {
    offsets: Mapped,
    neighbours: Mapped,
    labels: Mapped,
}

/// An open base: both orientations, the permutation, the asserter table.
#[derive(Debug)]
pub struct Base {
    dir: PathBuf,
    manifest: Manifest,
    table: Vec<Sources>,
    perm: Mapped,
    inv: Mapped,
    forward: Orientation,
    transpose: Orientation,
}

impl Base {
    /// Opens the base under `dir`: the manifest is checked against this
    /// binary's versions and every file against the sizes the counts
    /// imply; the bytes are mapped, not read. Digests are `check`'s.
    ///
    /// # Errors
    ///
    /// I/O, another version's base, a moved directory, or a file that is
    /// not the size it should be.
    pub fn open(dir: &Path) -> Result<Self, Error> {
        let manifest = Manifest::read(dir)?;
        manifest.check(dir)?;
        let table = manifest.table()?;
        let n = u64::from(manifest.nodes);
        let arcs = manifest.arcs;
        let expect = |name: &str, bytes: u64| -> Result<Mapped, Error> {
            let listed = manifest.files[name].bytes;
            if listed != bytes {
                return Err(Error::Mismatch {
                    what: format!(
                        "{}: {name} is {listed} bytes in the manifest, {bytes} by its counts",
                        dir.display()
                    ),
                });
            }
            Mapped::open(&dir.join(name), bytes)
        };
        let orientation = |offsets: &str, neighbours: &str, labels: &str| {
            Ok::<_, Error>(Orientation {
                offsets: expect(offsets, (n + 1) * 8)?,
                neighbours: expect(neighbours, arcs * 4)?,
                labels: expect(labels, arcs)?,
            })
        };
        Ok(Self {
            dir: dir.to_path_buf(),
            perm: expect(PERM, n * 4)?,
            inv: expect(INV, n * 4)?,
            forward: orientation(FORWARD_OFFSETS, FORWARD_NEIGHBOURS, FORWARD_LABELS)?,
            transpose: orientation(TRANSPOSE_OFFSETS, TRANSPOSE_NEIGHBOURS, TRANSPOSE_LABELS)?,
            table,
            manifest,
        })
    }

    #[allow(missing_docs)]
    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    #[allow(missing_docs)]
    #[must_use]
    pub const fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    /// The asserter table: a label indexes it.
    #[must_use]
    pub fn asserters(&self) -> &[Sources] {
        &self.table
    }

    /// Positions, which is the highest id the base was built over.
    #[must_use]
    pub const fn nodes(&self) -> u32 {
        self.manifest.nodes
    }

    /// Distinct edges; arcs per orientation.
    #[must_use]
    pub const fn arcs(&self) -> u64 {
        self.manifest.arcs
    }

    /// The permutation as bytes, for `check`.
    #[must_use]
    pub fn perm(&self) -> &[u8] {
        self.perm.bytes()
    }

    /// Its inverse as bytes, for `check`.
    #[must_use]
    pub fn inv(&self) -> &[u8] {
        self.inv.bytes()
    }

    /// A work's position; `None` for an id past the base.
    #[must_use]
    pub fn position(&self, id: WorkId) -> Option<u32> {
        let ix = nodes::index(id);
        (ix < self.perm.bytes().len() / 4).then(|| u32_at(self.perm.bytes(), ix))
    }

    /// The work at a position; `None` past the base.
    #[must_use]
    pub fn work_at(&self, pos: u32) -> Option<WorkId> {
        let ix = at(u64::from(pos));
        (ix < self.inv.bytes().len() / 4)
            .then(|| WorkId::new(u32_at(self.inv.bytes(), ix)))
            .flatten()
    }

    /// The forward orientation: what each work cites.
    #[must_use]
    pub fn forward(&self) -> Lists<'_> {
        Lists::new(&self.forward, &self.table, self.nodes())
    }

    /// The transpose: what cites each work.
    #[must_use]
    pub fn transpose(&self) -> Lists<'_> {
        Lists::new(&self.transpose, &self.table, self.nodes())
    }

    /// The works `id` cites, with who asserted each; empty past the base.
    #[must_use]
    pub fn references(&self, id: WorkId) -> Arcs<'_> {
        self.position(id)
            .map_or_else(|| Arcs::empty(&self.table), |pos| self.forward().list(pos))
    }

    /// The works citing `id`, with who asserted each; empty past the base.
    #[must_use]
    pub fn citers(&self, id: WorkId) -> Arcs<'_> {
        self.position(id).map_or_else(
            || Arcs::empty(&self.table),
            |pos| self.transpose().list(pos),
        )
    }

    /// How many works `id` cites, from the offsets alone.
    #[must_use]
    pub fn out_degree(&self, id: WorkId) -> u64 {
        self.position(id)
            .map_or(0, |pos| self.forward().degree(pos))
    }

    /// How many works cite `id`, from the offsets alone.
    #[must_use]
    pub fn in_degree(&self, id: WorkId) -> u64 {
        self.position(id)
            .map_or(0, |pos| self.transpose().degree(pos))
    }
}

/// One orientation's arrays.
#[derive(Debug, Clone, Copy)]
pub struct Lists<'a> {
    offsets: &'a [u8],
    neighbours: &'a [u8],
    labels: &'a [u8],
    table: &'a [Sources],
    rows: u32,
}

impl<'a> Lists<'a> {
    fn new(o: &'a Orientation, table: &'a [Sources], rows: u32) -> Self {
        Self {
            offsets: o.offsets.bytes(),
            neighbours: o.neighbours.bytes(),
            labels: o.labels.bytes(),
            table,
            rows,
        }
    }

    /// Positions.
    #[must_use]
    pub const fn rows(&self) -> u32 {
        self.rows
    }

    /// The `i`th offset, `i` from 0 to the row count inclusive.
    #[must_use]
    pub fn offset(&self, i: u64) -> u64 {
        u64_at(self.offsets, at(i))
    }

    /// The list length at a position.
    #[must_use]
    pub fn degree(&self, pos: u32) -> u64 {
        self.offset(u64::from(pos) + 1) - self.offset(u64::from(pos))
    }

    /// The arcs at a position, decoded.
    #[must_use]
    pub fn list(&self, pos: u32) -> Arcs<'a> {
        Arcs {
            raw: self.raw(pos),
            table: self.table,
        }
    }

    /// The arcs at a position as stored: neighbour value and label.
    #[must_use]
    pub fn raw(&self, pos: u32) -> Raw<'a> {
        let at = self.offset(u64::from(pos));
        let end = self.offset(u64::from(pos) + 1);
        Raw {
            neighbours: self.neighbours,
            labels: self.labels,
            at: self::at(at),
            end: self::at(end),
        }
    }

    /// The `k`th arc of the whole orientation as stored.
    #[must_use]
    pub fn arc_at(&self, k: u64) -> (u32, u8) {
        let k = at(k);
        (u32_at(self.neighbours, k), self.labels[k])
    }
}

/// One arc: the work at the other end and who asserted it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Arc {
    #[allow(missing_docs)]
    pub work: WorkId,
    #[allow(missing_docs)]
    pub by: Sources,
}

/// A list as stored: `(neighbour, label)` pairs.
#[derive(Debug, Clone)]
pub struct Raw<'a> {
    neighbours: &'a [u8],
    labels: &'a [u8],
    at: usize,
    end: usize,
}

impl Iterator for Raw<'_> {
    type Item = (u32, u8);

    fn next(&mut self) -> Option<(u32, u8)> {
        if self.at >= self.end {
            return None;
        }
        let item = (u32_at(self.neighbours, self.at), self.labels[self.at]);
        self.at += 1;
        Some(item)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let n = self.end - self.at;
        (n, Some(n))
    }
}

impl ExactSizeIterator for Raw<'_> {}

/// A list decoded: works and their asserters, lazily.
#[derive(Debug, Clone)]
pub struct Arcs<'a> {
    raw: Raw<'a>,
    table: &'a [Sources],
}

impl<'a> Arcs<'a> {
    fn empty(table: &'a [Sources]) -> Self {
        Self {
            raw: Raw {
                neighbours: &[],
                labels: &[],
                at: 0,
                end: 0,
            },
            table,
        }
    }
}

impl Iterator for Arcs<'_> {
    type Item = Arc;

    fn next(&mut self) -> Option<Arc> {
        let (work, label) = self.raw.next()?;
        Some(Arc {
            work: WorkId::new(work).expect("a neighbour of zero is not a work"),
            by: self.table[usize::from(label)],
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.raw.size_hint()
    }
}

impl ExactSizeIterator for Arcs<'_> {}

/// The `i`th little-endian `u32` of an array file.
#[must_use]
pub fn u32_at(bytes: &[u8], i: usize) -> u32 {
    let b = &bytes[i * 4..i * 4 + 4];
    u32::from_le_bytes([b[0], b[1], b[2], b[3]])
}

/// The `i`th little-endian `u64` of an array file.
#[must_use]
pub fn u64_at(bytes: &[u8], i: usize) -> u64 {
    let b = &bytes[i * 8..i * 8 + 8];
    u64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]])
}

/// A count as an index: past `usize` only on a target the plan's scale
/// never runs on, where the access that follows is out of bounds.
fn at(n: u64) -> usize {
    usize::try_from(n).unwrap_or(usize::MAX)
}

#[cfg(test)]
mod tests {
    use super::{Base, u32_at, u64_at};
    use crate::build::tests::{sample, scratch};
    use crate::build::{Options, build};
    use crate::layout::{self, Input, Manifest};

    #[test]
    fn little_endian_at() {
        assert_eq!(u32_at(&[1, 0, 0, 0, 2, 0, 0, 0], 1), 2);
        assert_eq!(u64_at(&[0; 8], 0), 0);
    }

    #[test]
    fn open_refuses_another_version_and_a_wrong_size() {
        let (nodes, edges) = sample();
        let root = scratch("open");
        let dir = root.join(layout::name(false));
        let input = Input {
            merged: "v1.1".into(),
            shards: 1,
        };
        build(&dir, &nodes, input, Options::default(), edges).unwrap();
        let mut manifest = Manifest::read(&dir).unwrap();
        manifest.graph += 1;
        manifest.write(&dir).unwrap();
        assert!(
            Base::open(&dir)
                .unwrap_err()
                .to_string()
                .contains("not this binary's")
        );
        manifest.graph -= 1;
        manifest.arcs += 1;
        manifest.write(&dir).unwrap();
        assert!(Base::open(&dir).unwrap_err().to_string().contains("bytes"));
        std::fs::remove_dir_all(&root).unwrap();
    }
}
