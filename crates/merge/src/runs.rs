//! Items sorted on disk (the serving layer's memory, 2026-09-24): pushed in
//! any order, written in sorted runs under a directory of their own as a
//! budget of memory fills, and given back merged, in order, each distinct
//! item once. What a builder that used to sort one `Vec` of everything
//! holds is one run's worth.
//!
//! Items compare by their own `Ord`, never by their bytes, so a builder
//! that sorted a `Vec` sorts the same way here and writes the same bytes.

use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::fmt;
use std::fs::{self, File};
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::marker::PhantomData;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde::de::DeserializeOwned;

/// Items gathered into sorted runs on disk. The directory is the runs'
/// own: made empty when they begin, removed with them.
pub struct Runs<T> {
    dir: PathBuf,
    buffer: Vec<T>,
    held: usize,
    budget: usize,
    files: Vec<PathBuf>,
    pushed: u64,
}

impl<T> fmt::Debug for Runs<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Runs")
            .field("dir", &self.dir)
            .field("runs", &self.files.len())
            .field("pushed", &self.pushed)
            .finish_non_exhaustive()
    }
}

impl<T: Ord + Clone + Serialize + DeserializeOwned> Runs<T> {
    /// Runs under `dir`, each holding about `budget` bytes of items before
    /// it is sorted and written.
    ///
    /// # Errors
    ///
    /// I/O.
    pub fn create(dir: &Path, budget: usize) -> io::Result<Self> {
        if dir.exists() {
            fs::remove_dir_all(dir)?;
        }
        fs::create_dir_all(dir)?;
        Ok(Self {
            dir: dir.to_path_buf(),
            buffer: Vec::new(),
            held: 0,
            budget,
            files: Vec::new(),
            pushed: 0,
        })
    }

    /// One item, `bytes` about what it holds.
    ///
    /// # Errors
    ///
    /// I/O, writing a run.
    pub fn push(&mut self, item: T, bytes: usize) -> io::Result<()> {
        self.buffer.push(item);
        self.held += bytes + size_of::<T>();
        self.pushed += 1;
        if self.held >= self.budget {
            self.spill()?;
        }
        Ok(())
    }

    /// Items pushed, repeats included.
    #[must_use]
    pub const fn pushed(&self) -> u64 {
        self.pushed
    }

    fn spill(&mut self) -> io::Result<()> {
        if self.buffer.is_empty() {
            return Ok(());
        }
        self.buffer.sort_unstable();
        self.buffer.dedup();
        let path = self.dir.join(format!("run_{:04}", self.files.len()));
        let mut out = BufWriter::with_capacity(1 << 20, File::create(&path)?);
        let mut bytes = Vec::new();
        for item in self.buffer.drain(..) {
            bytes.clear();
            bytes = postcard::to_extend(&item, bytes).map_err(io::Error::other)?;
            let len = u32::try_from(bytes.len()).map_err(io::Error::other)?;
            out.write_all(&len.to_le_bytes())?;
            out.write_all(&bytes)?;
        }
        out.flush()?;
        self.files.push(path);
        self.held = 0;
        self.buffer.shrink_to_fit();
        Ok(())
    }

    /// Every distinct item, in order.
    ///
    /// # Errors
    ///
    /// I/O, writing the last run or opening the runs.
    pub fn sorted(mut self) -> io::Result<Sorted<T>> {
        self.spill()?;
        let mut readers = self
            .files
            .iter()
            .map(|p| Ok(BufReader::with_capacity(1 << 20, File::open(p)?)))
            .collect::<io::Result<Vec<_>>>()?;
        let mut heap = BinaryHeap::new();
        for (r, reader) in readers.iter_mut().enumerate() {
            if let Some(item) = read_item::<T>(reader)? {
                heap.push(Reverse((item, r)));
            }
        }
        Ok(Sorted {
            dir: std::mem::take(&mut self.dir),
            readers,
            heap,
            last: None,
            kind: PhantomData,
        })
    }
}

impl<T> Drop for Runs<T> {
    fn drop(&mut self) {
        if !self.dir.as_os_str().is_empty() {
            let _ = fs::remove_dir_all(&self.dir);
        }
    }
}

fn read_item<T: DeserializeOwned>(reader: &mut impl Read) -> io::Result<Option<T>> {
    let mut len = [0u8; 4];
    match reader.read_exact(&mut len) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e),
    }
    let mut bytes = vec![0u8; u32::from_le_bytes(len) as usize];
    reader.read_exact(&mut bytes)?;
    postcard::from_bytes(&bytes)
        .map(Some)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

/// The runs merged: every distinct item, in order. Takes the runs'
/// directory with it when dropped.
pub struct Sorted<T> {
    dir: PathBuf,
    readers: Vec<BufReader<File>>,
    heap: BinaryHeap<Reverse<(T, usize)>>,
    last: Option<T>,
    kind: PhantomData<T>,
}

impl<T> fmt::Debug for Sorted<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Sorted")
            .field("dir", &self.dir)
            .field("runs", &self.readers.len())
            .finish_non_exhaustive()
    }
}

impl<T: Ord + Clone + DeserializeOwned> Iterator for Sorted<T> {
    type Item = io::Result<T>;

    fn next(&mut self) -> Option<io::Result<T>> {
        loop {
            let Reverse((item, r)) = self.heap.pop()?;
            match read_item::<T>(&mut self.readers[r]) {
                Ok(Some(next)) => self.heap.push(Reverse((next, r))),
                Ok(None) => {}
                Err(e) => return Some(Err(e)),
            }
            if self.last.as_ref() == Some(&item) {
                continue;
            }
            self.last = Some(item.clone());
            return Some(Ok(item));
        }
    }
}

impl<T> Drop for Sorted<T> {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

#[cfg(test)]
mod tests {
    use super::Runs;

    /// Whatever the runs, the items come back as one sorted, deduplicated
    /// `Vec` would give them, and the directory goes with them.
    #[test]
    fn runs_give_back_what_one_sort_would() {
        let dir = std::env::temp_dir().join(format!("ol-rht-runs-{}", std::process::id()));
        let mut items: Vec<(String, u32)> = (0..500u32)
            .map(|i| (format!("name{}", (i * 7919) % 97), i % 13))
            .collect();
        let mut runs = Runs::create(&dir, 200).unwrap();
        for item in &items {
            runs.push(item.clone(), item.0.len()).unwrap();
        }
        assert!(runs.files.len() > 5, "a small budget makes many runs");
        let back: Vec<(String, u32)> = runs.sorted().unwrap().map(Result::unwrap).collect();
        items.sort_unstable();
        items.dedup();
        assert_eq!(back, items);
        assert!(!dir.exists());
    }
}
