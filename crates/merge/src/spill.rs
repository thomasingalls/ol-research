//! Pass 2's rows on disk, by work id, as they are made — and read back one
//! range of ids at a time (merge plan §8; the merge's memory, step 1,
//! 2026-09-24).
//!
//! Pass 2 makes rows in the order it reads the normalized sets, and every
//! row it makes belongs to one work: a view to its record's work, an edge,
//! a relation, a `cited_as` row and an unresolved reference to the work
//! that cites, a stub's citer to the stub. Nothing `finish` does to a row
//! needs a row of another work's, except whether the other end of an edge
//! or a relation is a work at all, which is a table of one entry per id.
//! So the rows can go to disk as they are made, each to the file of the
//! range its work's id falls in, and `finish` can take the ranges one at a
//! time, in id order, holding one range's rows where it held the whole
//! corpus's: on the PubMed baseline that was the difference between a run
//! killed at 80 GiB with its references 60% read and one bounded by a
//! single range.
//!
//! A range is a whole number of shards (`layout::SHARD`), so a range's
//! rows, finished, are exactly the rows of its shards and can be written
//! at once. Within a range the rows come back in the order they were
//! made, which is the only order `finish` relies on beyond the sorts it
//! does itself; its output is the same, byte for byte.

use std::fs;
use std::io::{self, BufReader, Read, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::arena::NameIx;
use crate::build::{OwnedCitedAs, OwnedUnresolved};
use crate::identity::RecordIx;
use crate::record::{Citer, Relation};
use crate::view::Owned;
use crate::work::{SourceId, WorkId};

/// One row of pass 2, as it goes to disk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Row {
    /// A record's view, before it is joined with the record's other
    /// copies: the join happens when the range is read, in this order.
    View(RecordIx, Owned),
    /// One reference's arc, before edges are joined by source.
    Edge(WorkId, WorkId, SourceId),
    #[allow(missing_docs)]
    Relation(Relation),
    #[allow(missing_docs)]
    CitedAs(OwnedCitedAs),
    #[allow(missing_docs)]
    Unresolved(OwnedUnresolved),
    /// A reference that named a stub name.
    StubCiter(NameIx, Citer),
}

/// Bytes a range's rows gather in memory before they are appended to its
/// file: a range's file is opened only to append a buffer, so the files
/// open at once are one, however many ranges there are.
const FLUSH: usize = 1 << 20;

/// Pass 2's rows, spilled by work id.
///
/// The directory is the spill's own: made empty when the spill begins and
/// removed when it is dropped, so a run that fails leaves nothing behind,
/// and one killed outright leaves a directory the next run replaces.
#[derive(Debug)]
pub struct Spill {
    dir: PathBuf,
    width: u32,
    buffers: Vec<Vec<u8>>,
    rows: u64,
    bytes: u64,
}

impl Spill {
    /// A spill under `dir` of ranges `width` ids wide, for ids up to
    /// `max`.
    ///
    /// # Errors
    ///
    /// I/O, or a width of zero.
    pub fn create(dir: &Path, width: u32, max: WorkId) -> io::Result<Self> {
        if width == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "a spill range is at least one id wide",
            ));
        }
        if dir.exists() {
            fs::remove_dir_all(dir)?;
        }
        fs::create_dir_all(dir)?;
        let ranges = (max.get() - 1) / width + 1;
        Ok(Self {
            dir: dir.to_path_buf(),
            width,
            buffers: vec![Vec::new(); ranges as usize],
            rows: 0,
            bytes: 0,
        })
    }

    /// Ranges, in id order.
    #[must_use]
    pub fn ranges(&self) -> u32 {
        u32::try_from(self.buffers.len()).unwrap_or(u32::MAX)
    }

    /// Where the spill is.
    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Ids a range is wide.
    #[must_use]
    pub const fn width(&self) -> u32 {
        self.width
    }

    /// The range `id` falls in.
    #[must_use]
    pub const fn range_of(&self, id: WorkId) -> u32 {
        (id.get() - 1) / self.width
    }

    /// Rows spilled.
    #[must_use]
    pub const fn rows(&self) -> u64 {
        self.rows
    }

    /// Bytes spilled, flushed or not.
    #[must_use]
    pub const fn bytes(&self) -> u64 {
        self.bytes
    }

    fn path(&self, range: u32) -> PathBuf {
        self.dir.join(format!("range_{range:06}"))
    }

    /// Puts `row` in the range of `id`, the work it belongs to.
    ///
    /// # Errors
    ///
    /// I/O, or an id past the spill's last range.
    pub fn put(&mut self, id: WorkId, row: &Row) -> io::Result<()> {
        let range = self.range_of(id);
        let slot = self.buffers.get_mut(range as usize).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{id} is past the spill's last range"),
            )
        })?;
        // A row is its length, four bytes little-endian, then its bytes.
        let mut buffer = std::mem::take(slot);
        let at = buffer.len();
        buffer.extend_from_slice(&[0; 4]);
        let mut buffer = postcard::to_extend(row, buffer).map_err(io::Error::other)?;
        let len = u32::try_from(buffer.len() - at - 4).map_err(io::Error::other)?;
        buffer[at..at + 4].copy_from_slice(&len.to_le_bytes());
        self.bytes += (buffer.len() - at) as u64;
        self.rows += 1;
        let full = buffer.len() >= FLUSH;
        self.buffers[range as usize] = buffer;
        if full {
            self.flush(range)?;
        }
        Ok(())
    }

    fn flush(&mut self, range: u32) -> io::Result<()> {
        let buffer = &mut self.buffers[range as usize];
        if buffer.is_empty() {
            return Ok(());
        }
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.dir.join(format!("range_{range:06}")))?;
        file.write_all(buffer)?;
        let buffer = &mut self.buffers[range as usize];
        buffer.clear();
        buffer.shrink_to(FLUSH);
        Ok(())
    }

    /// Every row of `range`, in the order they were put; the range's file
    /// and buffer are gone afterwards.
    ///
    /// # Errors
    ///
    /// I/O, or a row that does not decode.
    pub fn take(&mut self, range: u32) -> io::Result<Vec<Row>> {
        self.flush(range)?;
        self.buffers[range as usize] = Vec::new();
        let path = self.path(range);
        let file = match fs::File::open(&path) {
            Ok(file) => file,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e),
        };
        let mut reader = BufReader::with_capacity(FLUSH, file);
        let mut rows = Vec::new();
        let mut bytes = Vec::new();
        loop {
            let mut len = [0u8; 4];
            match reader.read_exact(&mut len) {
                Ok(()) => {}
                Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => break,
                Err(e) => return Err(e),
            }
            bytes.resize(u32::from_le_bytes(len) as usize, 0);
            reader.read_exact(&mut bytes)?;
            rows.push(postcard::from_bytes(&bytes).map_err(|e| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("{}: {e}", path.display()),
                )
            })?);
        }
        drop(reader);
        fs::remove_file(&path)?;
        Ok(rows)
    }
}

impl Drop for Spill {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

#[cfg(test)]
mod tests {
    use super::{Row, Spill};
    use crate::record::{Citer, Relation};
    use crate::relation;
    use crate::title::{OwnedTitles, Titles};
    use crate::view::Owned;
    use crate::work::{SourceId, WorkId};

    fn id(n: u32) -> WorkId {
        WorkId::new(n).unwrap()
    }

    /// Rows come back from their range in the order they were put, the
    /// ranges keep to themselves, and a view survives the trip whole —
    /// its title fingerprints, which are not written, recomputed from the
    /// title it carries (2026-09-24).
    #[test]
    fn rows_come_back_by_range_in_the_order_they_went() {
        let dir = std::env::temp_dir().join(format!("ol-rht-spill-{}", std::process::id()));
        let mut spill = Spill::create(&dir, 10, id(25)).unwrap();
        assert_eq!(spill.ranges(), 3);
        let title = "A title: with a subtitle";
        let view = Owned {
            title: Some(title.into()),
            titles: OwnedTitles::of(Titles {
                title: Some(title),
                vernacular: None,
            }),
            set: "pubmed26n0001".into(),
            authors: 3,
            ..Owned::default()
        };
        let rows = [
            (id(3), Row::Edge(id(3), id(21), SourceId::Pubmed)),
            (id(12), Row::View(7, view.clone())),
            (id(3), Row::Edge(id(3), id(4), SourceId::Crossref)),
            (
                id(25),
                Row::Relation(Relation {
                    from: id(25),
                    to: id(1),
                    kind: relation::Kind::ErratumIn,
                    by: SourceId::Pubmed,
                }),
            ),
            (
                id(12),
                Row::StubCiter(
                    4,
                    Citer {
                        from: id(12),
                        by: SourceId::Pubmed,
                        in_text: false,
                        held_beside: true,
                    },
                ),
            ),
        ];
        for (at, row) in &rows {
            spill.put(*at, row).unwrap();
        }
        assert_eq!(spill.rows(), 5);
        let back: Vec<Vec<Row>> = (0..spill.ranges())
            .map(|r| spill.take(r).unwrap())
            .collect();
        assert_eq!(back[0], [rows[0].1.clone(), rows[2].1.clone()]);
        assert_eq!(back[1], [rows[1].1.clone(), rows[4].1.clone()]);
        assert_eq!(back[2], [rows[3].1.clone()]);
        let Row::View(7, returned) = &back[1][0] else {
            panic!("{:?}", back[1][0]);
        };
        assert_eq!(returned, &view, "the fingerprints are recomputed the same");
        assert!(
            spill.put(id(31), &rows[0].1).is_err(),
            "past the last range"
        );
        drop(spill);
        assert!(!dir.exists(), "a spill takes its directory with it");
    }
}
