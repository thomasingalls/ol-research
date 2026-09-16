//! The names lookup (serving plan §3): every name in the names set and
//! every public id, to its work, in a finite state transducer mapped
//! into memory — a million DOIs sharing `10.1016/` store it once, and a
//! lookup costs the key's length. A shared name points at a side list; a
//! retired id knows its survivor. Its delta needs no file: the registry's
//! tail past the epoch is exactly the events a lookup needs (SP5).

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use fst::{Map, MapBuilder};
use ingest::Kind;
use memmap2::Mmap;
use merge::public::Public;
use merge::record::{NameRow, Work};
use merge::{Registry, WorkId};
use serde::{Deserialize, Serialize};

use crate::digest::{FileInfo, Sink};
use crate::error::Error;
use crate::mapped::{Mapped, u32_at};

#[allow(missing_docs)]
pub const FST: &str = "names.fst";
#[allow(missing_docs)]
pub const SHARED: &str = "shared.u32";
#[allow(missing_docs)]
pub const RETIRED: &str = "retired.u32";
/// `WorkId` to the 64 bits of its public id, zero for none: the reverse
/// of the transducer for the one namespace the transport speaks.
pub const PUBLIC: &str = "public.u64";
/// One bit per id, set for a stub: what a reference's target is, without
/// the works index.
pub const STUBS: &str = "stubs.bits";
#[allow(missing_docs)]
pub const MANIFEST: &str = "manifest.json";

/// A value whose low bits index the shared list rather than name a work.
const SHARED_BIT: u64 = 1 << 63;

/// What a build records about itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    #[allow(missing_docs)]
    pub serve: u32,
    #[allow(missing_docs)]
    pub model: u32,
    #[allow(missing_docs)]
    pub merge: u32,
    /// The journal batch the base is built from.
    pub batch: u64,
    #[allow(missing_docs)]
    pub built_at: String,
    /// Slots in the public array: the highest id.
    pub nodes: u32,
    /// Distinct keys: names and public ids.
    pub keys: u64,
    /// Keys naming more than one work.
    pub shared: u64,
    #[allow(missing_docs)]
    pub public: u64,
    #[allow(missing_docs)]
    pub retired: u64,
    #[allow(missing_docs)]
    pub files: BTreeMap<String, FileInfo>,
}

impl Manifest {
    /// Reads `dir`'s manifest.
    ///
    /// # Errors
    ///
    /// I/O, or a manifest that does not parse.
    pub fn read(dir: &Path) -> Result<Self, Error> {
        let path = dir.join(MANIFEST);
        let text = fs::read_to_string(&path).map_err(|e| Error::io(&path, e))?;
        serde_json::from_str(&text).map_err(|source| Error::Manifest { path, source })
    }

    fn write(&self, dir: &Path) -> Result<(), Error> {
        let path = dir.join(MANIFEST);
        let mut text = serde_json::to_string_pretty(self).map_err(|source| Error::Manifest {
            path: path.clone(),
            source,
        })?;
        text.push('\n');
        fs::write(&path, text).map_err(|e| Error::io(&path, e))
    }

    fn check(&self, dir: &Path) -> Result<(), Error> {
        if (self.serve, self.model, self.merge) != (crate::VERSION, model::VERSION, merge::VERSION)
        {
            return Err(Error::Mismatch(format!(
                "{}: built as serve {} over model {} and merge {}, not this binary's {}",
                dir.display(),
                self.serve,
                self.model,
                self.merge,
                crate::version()
            )));
        }
        for file in [FST, SHARED, RETIRED, PUBLIC, STUBS] {
            if !self.files.contains_key(file) {
                return Err(Error::Mismatch(format!(
                    "{}: the manifest lists no {file}",
                    dir.display()
                )));
            }
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

/// The directory a build lands in before it replaces `dir`.
pub(crate) fn part_of(dir: &Path) -> PathBuf {
    let name = dir.file_name().and_then(|n| n.to_str()).unwrap_or("base");
    dir.with_file_name(format!("{name}.part"))
}

/// Builds the lookup into `dir` from the names sets under `merged`, the
/// public log and the registry's retirements, replacing what was there.
///
/// # Errors
///
/// I/O, a set that is not a merge's, or the transducer refusing a key.
pub fn build(
    dir: &Path,
    merged: &Path,
    public: &Public,
    registry: &Registry,
    batch: u64,
) -> Result<Built, Error> {
    let mut pairs: Vec<(String, WorkId)> = Vec::new();
    for path in merge::sets::find(merged, Kind::Names).map_err(|e| Error::io(merged, e))? {
        let reader = merge::layout::open(&path).map_err(|e| Error::io(&path, e))?;
        reader.with_table(|| -> Result<(), Error> {
            for row in reader.records::<NameRow<'_>>() {
                let row = row.map_err(|e| Error::io(&path, e))?;
                pairs.push((row.name.to_string(), row.work));
            }
            Ok(())
        })?;
    }
    let nodes = registry.minted();
    let mut stubs = vec![0u8; usize::try_from(nodes).unwrap_or(usize::MAX).div_ceil(8)];
    for path in merge::sets::find(merged, Kind::Works).map_err(|e| Error::io(merged, e))? {
        let reader = merge::layout::open(&path).map_err(|e| Error::io(&path, e))?;
        reader.with_table(|| -> Result<(), Error> {
            for row in reader.records::<Work<'_>>() {
                let row = row.map_err(|e| Error::io(&path, e))?;
                if row.stub {
                    let ix = usize::try_from(row.id.get() - 1).unwrap_or(usize::MAX);
                    if let Some(byte) = stubs.get_mut(ix / 8) {
                        *byte |= 1 << (ix % 8);
                    }
                }
            }
            Ok(())
        })?;
    }
    let mut public_keys = 0u64;
    for (id, text) in public.iter() {
        pairs.push((text.to_owned(), id));
        public_keys += 1;
    }
    pairs.sort_unstable();
    pairs.dedup();
    let mut builder = MapBuilder::memory();
    let mut shared: Vec<u32> = Vec::new();
    let (mut keys, mut shared_keys) = (0u64, 0u64);
    let mut i = 0;
    while i < pairs.len() {
        let key = pairs[i].0.as_str();
        let mut j = i;
        while j < pairs.len() && pairs[j].0 == key {
            j += 1;
        }
        let group = &pairs[i..j];
        let value = if let [(_, id)] = group {
            u64::from(id.get())
        } else {
            let offset = u32::try_from(shared.len()).map_err(|_| {
                Error::Mismatch("more shared names than the side list holds".into())
            })?;
            shared.push(u32::try_from(group.len()).unwrap_or(u32::MAX));
            shared.extend(group.iter().map(|(_, id)| id.get()));
            shared_keys += 1;
            SHARED_BIT | u64::from(offset)
        };
        builder.insert(key, value)?;
        keys += 1;
        i = j;
    }
    let fst_bytes = builder.into_inner()?;
    let mut retirements: Vec<(u32, u32)> = registry
        .retirements()
        .map(|(id, into)| (id.get(), into.map_or(0, WorkId::get)))
        .collect();
    retirements.sort_unstable();

    let part = part_of(dir);
    let files = write_files(
        &part,
        &fst_bytes,
        &shared,
        &retirements,
        public,
        nodes,
        &stubs,
    )?;
    let manifest = Manifest {
        serve: crate::VERSION,
        model: model::VERSION,
        merge: merge::VERSION,
        batch,
        built_at: ingest::time::now(),
        nodes,
        keys,
        shared: shared_keys,
        public: public_keys,
        retired: retirements.len() as u64,
        files,
    };
    manifest.write(&part)?;
    if dir.exists() {
        fs::remove_dir_all(dir).map_err(|e| Error::io(dir, e))?;
    }
    fs::rename(&part, dir).map_err(|e| Error::io(dir, e))?;
    Ok(Built {
        dir: dir.to_path_buf(),
        manifest,
    })
}

/// Writes the five files into a fresh part directory.
fn write_files(
    part: &Path,
    fst_bytes: &[u8],
    shared: &[u32],
    retirements: &[(u32, u32)],
    public: &Public,
    nodes: u32,
    stubs: &[u8],
) -> Result<BTreeMap<String, FileInfo>, Error> {
    if part.exists() {
        fs::remove_dir_all(part).map_err(|e| Error::io(part, e))?;
    }
    fs::create_dir_all(part).map_err(|e| Error::io(part, e))?;
    let mut files = BTreeMap::new();
    let mut sink = Sink::create(&part.join(FST))?;
    sink.put(fst_bytes)?;
    files.insert(FST.to_owned(), sink.finish()?);
    let mut sink = Sink::create(&part.join(SHARED))?;
    for v in shared {
        sink.put(&v.to_le_bytes())?;
    }
    files.insert(SHARED.to_owned(), sink.finish()?);
    let mut sink = Sink::create(&part.join(RETIRED))?;
    for (id, into) in retirements {
        sink.put(&id.to_le_bytes())?;
        sink.put(&into.to_le_bytes())?;
    }
    files.insert(RETIRED.to_owned(), sink.finish()?);
    let mut bits = vec![0u64; usize::try_from(nodes).unwrap_or(usize::MAX)];
    for (id, text) in public.iter() {
        if let (Some(slot), Ok(value)) = (
            bits.get_mut(usize::try_from(id.get() - 1).unwrap_or(usize::MAX)),
            merge::public::decode(text),
        ) {
            *slot = value;
        }
    }
    let mut sink = Sink::create(&part.join(PUBLIC))?;
    for v in &bits {
        sink.put(&v.to_le_bytes())?;
    }
    files.insert(PUBLIC.to_owned(), sink.finish()?);
    let mut sink = Sink::create(&part.join(STUBS))?;
    sink.put(stubs)?;
    files.insert(STUBS.to_owned(), sink.finish()?);
    Ok(files)
}

/// What a key names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolved {
    /// One work.
    One(WorkId),
    /// A shared name: every work it was accepted into.
    Shared(Vec<WorkId>),
}

/// An open lookup.
pub struct Names {
    manifest: Manifest,
    map: Map<Mmap>,
    shared: Mapped,
    public: Mapped,
    stubs: Mapped,
    retired: HashMap<WorkId, Option<WorkId>>,
}

impl std::fmt::Debug for Names {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Names")
            .field("manifest", &self.manifest)
            .finish_non_exhaustive()
    }
}

impl Names {
    /// Opens the lookup under `dir`, checking the manifest and every
    /// file's size.
    ///
    /// # Errors
    ///
    /// I/O, another version's base, or a file of the wrong size.
    pub fn open(dir: &Path) -> Result<Self, Error> {
        let manifest = Manifest::read(dir)?;
        manifest.check(dir)?;
        let open = |name: &str| Mapped::open(&dir.join(name), manifest.files[name].bytes);
        let fst_map = open(FST)?
            .into_inner()
            .ok_or_else(|| Error::Mismatch(format!("{}: an empty {FST}", dir.display())))?;
        let map = Map::new(fst_map)?;
        let shared = open(SHARED)?;
        let public = open(PUBLIC)?;
        let stubs = open(STUBS)?;
        if manifest.files[STUBS].bytes != u64::from(manifest.nodes).div_ceil(8) {
            return Err(Error::Mismatch(format!(
                "{}: {} nodes but {} stub bytes",
                dir.display(),
                manifest.nodes,
                manifest.files[STUBS].bytes
            )));
        }
        if manifest.files[PUBLIC].bytes != u64::from(manifest.nodes) * 8 {
            return Err(Error::Mismatch(format!(
                "{}: {} nodes but {} public bytes",
                dir.display(),
                manifest.nodes,
                manifest.files[PUBLIC].bytes
            )));
        }
        let retired_bytes = open(RETIRED)?;
        let bytes = retired_bytes.bytes();
        let mut retired = HashMap::with_capacity(bytes.len() / 8);
        for i in 0..bytes.len() / 8 {
            let id = u32_at(bytes, 2 * i);
            let into = u32_at(bytes, 2 * i + 1);
            if let Some(id) = WorkId::new(id) {
                retired.insert(id, WorkId::new(into));
            }
        }
        Ok(Self {
            manifest,
            map,
            shared,
            public,
            stubs,
            retired,
        })
    }

    /// Whether an id is a stub in the base: names and no records. False
    /// for an id past the base or a retired one.
    #[must_use]
    pub fn is_stub(&self, id: WorkId) -> bool {
        let ix = usize::try_from(id.get() - 1).unwrap_or(usize::MAX);
        self.stubs
            .bytes()
            .get(ix / 8)
            .is_some_and(|byte| byte & (1 << (ix % 8)) != 0)
    }

    /// A work's public id in its text form, from the array; `None` when
    /// none was minted or the id is past the base.
    #[must_use]
    pub fn public_of(&self, id: WorkId) -> Option<String> {
        let ix = usize::try_from(id.get() - 1).ok()?;
        let bytes = self.public.bytes();
        if ix * 8 + 8 > bytes.len() {
            return None;
        }
        let bits = crate::mapped::u64_at(bytes, ix);
        (bits != 0).then(|| merge::public::encode(bits))
    }

    #[allow(missing_docs)]
    #[must_use]
    pub const fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    /// Distinct keys.
    #[must_use]
    pub fn keys(&self) -> u64 {
        self.map.len() as u64
    }

    /// Every key in order, with its stored value.
    #[must_use]
    pub fn stream(&self) -> fst::map::Stream<'_> {
        self.map.stream()
    }

    /// The work or works a key names, as stored: ids retired since are
    /// not followed here — see [`live`](Self::live).
    #[must_use]
    pub fn get(&self, key: &str) -> Option<Resolved> {
        let value = self.map.get(key)?;
        if value & SHARED_BIT == 0 {
            return WorkId::new(u32::try_from(value).ok()?).map(Resolved::One);
        }
        let offset = usize::try_from(value & !SHARED_BIT).ok()?;
        let bytes = self.shared.bytes();
        let count = usize::try_from(u32_at(bytes, offset)).ok()?;
        let ids = (1..=count)
            .filter_map(|k| WorkId::new(u32_at(bytes, offset + k)))
            .collect();
        Some(Resolved::Shared(ids))
    }

    /// The live id an id stands for: itself, the id it was merged into,
    /// transitively, or `None` when it was split.
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

    /// Whether an id was retired.
    #[must_use]
    pub fn is_retired(&self, id: WorkId) -> bool {
        self.retired.contains_key(&id)
    }
}
