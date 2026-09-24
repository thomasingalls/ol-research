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
use merge::WorkId;
use merge::record::{NameRow, Work};
use merge::registry::IdFold;
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
/// The parts of every id a split retired, as `(id, part)` pairs sorted
/// by id: an id with no survivor still answers with them, as what was
/// once so (serving plan §11, item 24).
pub const SPLIT: &str = "split.u32";
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
    /// Ids a split retired, of the `retired` above. Defaulted, and
    /// [`SPLIT`] is not required below, so that a base written before
    /// the file existed still reads and simply answers with no parts —
    /// what the layer answered anyway until 2026-09-19. A manifest is
    /// read before its version is checked, and a parse error there says
    /// nothing useful about which version it is.
    #[serde(default)]
    pub split: u64,
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

/// Bytes of pairs a sorted run holds before it is written.
const RUN: usize = 256 << 20;

/// Builds the lookup into `dir` from the names sets under `merged`, the
/// public log at `public` and the registry's retirements, replacing what
/// was there. The names and public ids are sorted on disk and the
/// transducer written as they come back; the registry is read for its ids
/// and the public log as a stream (the serving layer's memory,
/// 2026-09-24): on the corpus the pairs sorted in one `Vec`, the registry
/// folded by name and the public log in two maps came to about 26 GB.
///
/// # Errors
///
/// I/O, a set that is not a merge's, a public log that gives an id or a
/// public id twice, or the transducer refusing a key.
#[allow(clippy::too_many_lines)]
pub fn build(
    dir: &Path,
    merged: &Path,
    registry: &Path,
    public: &Path,
    batch: u64,
) -> Result<Built, Error> {
    static RUNS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let scratch = std::env::temp_dir().join(format!(
        "ol-rht-names-{}-{}",
        std::process::id(),
        RUNS.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let registry = IdFold::read(registry).map_err(|e| Error::io(registry, e))?;
    let mut runs: merge::runs::Runs<(String, WorkId)> =
        merge::runs::Runs::create(&scratch, RUN).map_err(|e| Error::io(&scratch, e))?;
    for path in merge::sets::find(merged, Kind::Names).map_err(|e| Error::io(merged, e))? {
        let reader = merge::layout::open(&path).map_err(|e| Error::io(&path, e))?;
        reader.with_table(|| -> Result<(), Error> {
            for row in reader.records::<NameRow<'_>>() {
                let row = row.map_err(|e| Error::io(&path, e))?;
                let name = row.name.to_string();
                let bytes = name.len();
                runs.push((name, row.work), bytes)
                    .map_err(|e| Error::io(&scratch, e))?;
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
    // The public log read as a stream: each id's public id into the
    // transducer and its value into `public.u64`, the two things this
    // lookup keeps of it, and each id once, as `Public::read` has it.
    let slots = usize::try_from(nodes).unwrap_or(usize::MAX);
    let mut public_bits = vec![0u64; slots];
    let mut minted = vec![0u64; slots.div_ceil(64)];
    let mut public_keys = 0u64;
    for line in merge::public::lines(public).map_err(|e| Error::io(public, e))? {
        let (id, text) = line.map_err(|e| Error::io(public, e))?;
        let slot = usize::try_from(id.get() - 1).unwrap_or(usize::MAX);
        if let Some(word) = minted.get_mut(slot / 64) {
            if *word & (1 << (slot % 64)) != 0 {
                return Err(Error::Mismatch(format!(
                    "{}: {id} minted twice",
                    public.display()
                )));
            }
            *word |= 1 << (slot % 64);
        }
        if let (Some(value), Ok(decoded)) =
            (public_bits.get_mut(slot), merge::public::decode(&text))
        {
            *value = decoded;
        }
        let bytes = text.len();
        runs.push((text, id), bytes)
            .map_err(|e| Error::io(&scratch, e))?;
        public_keys += 1;
    }
    let sorted = runs.sorted().map_err(|e| Error::io(&scratch, e))?;
    let part = part_of(dir);
    if part.exists() {
        fs::remove_dir_all(&part).map_err(|e| Error::io(&part, e))?;
    }
    fs::create_dir_all(&part).map_err(|e| Error::io(&part, e))?;
    let mut builder = MapBuilder::new(Sink::create(&part.join(FST))?)?;
    let mut shared: Vec<u32> = Vec::new();
    let (mut keys, mut shared_keys) = (0u64, 0u64);
    let mut group: Vec<WorkId> = Vec::new();
    let mut key: Option<String> = None;
    let mut close =
        |builder: &mut MapBuilder<Sink>, key: &str, group: &[WorkId]| -> Result<(), Error> {
            let value = if let [id] = group {
                u64::from(id.get())
            } else {
                if key.starts_with(merge::public::PREFIX) {
                    return Err(Error::Mismatch(format!("{key} given twice")));
                }
                let offset = u32::try_from(shared.len()).map_err(|_| {
                    Error::Mismatch("more shared names than the side list holds".into())
                })?;
                shared.push(u32::try_from(group.len()).unwrap_or(u32::MAX));
                shared.extend(group.iter().map(|id| id.get()));
                shared_keys += 1;
                SHARED_BIT | u64::from(offset)
            };
            builder.insert(key, value)?;
            keys += 1;
            Ok(())
        };
    for item in sorted {
        let (name, id) = item.map_err(|e| Error::io(&scratch, e))?;
        if key.as_deref() != Some(name.as_str()) {
            if let Some(done) = key.take() {
                close(&mut builder, &done, &group)?;
            }
            group.clear();
            key = Some(name);
        }
        group.push(id);
    }
    if let Some(done) = key.take() {
        close(&mut builder, &done, &group)?;
    }
    let fst = builder.into_inner()?.finish()?;
    let mut retirements: Vec<(u32, u32)> = registry
        .retirements()
        .map(|(id, into)| (id.get(), into.map_or(0, WorkId::get)))
        .collect();
    retirements.sort_unstable();
    // One pair per part, sorted, so a rebuild is byte-identical as the
    // retirements beside it are; the open folds them into a map.
    let mut split: Vec<(u32, u32)> = registry
        .splits()
        .flat_map(|(id, into)| into.iter().map(move |part| (id.get(), part.get())))
        .collect();
    split.sort_unstable();
    let split_ids = registry.splits().count() as u64;
    let mut files = write_files(&part, &shared, &retirements, &split, &public_bits, &stubs)?;
    files.insert(FST.to_owned(), fst);
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
        split: split_ids,
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

/// Writes every file but the transducer into the part directory.
fn write_files(
    part: &Path,
    shared: &[u32],
    retirements: &[(u32, u32)],
    split: &[(u32, u32)],
    public: &[u64],
    stubs: &[u8],
) -> Result<BTreeMap<String, FileInfo>, Error> {
    let mut files = BTreeMap::new();
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
    let mut sink = Sink::create(&part.join(SPLIT))?;
    for (id, into) in split {
        sink.put(&id.to_le_bytes())?;
        sink.put(&into.to_le_bytes())?;
    }
    files.insert(SPLIT.to_owned(), sink.finish()?);
    let mut sink = Sink::create(&part.join(PUBLIC))?;
    for v in public {
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
    split: HashMap<WorkId, Vec<WorkId>>,
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
        let mut split: HashMap<WorkId, Vec<WorkId>> = HashMap::new();
        if manifest.files.contains_key(SPLIT) {
            let split_bytes = open(SPLIT)?;
            let bytes = split_bytes.bytes();
            for i in 0..bytes.len() / 8 {
                let id = u32_at(bytes, 2 * i);
                let part = u32_at(bytes, 2 * i + 1);
                if let (Some(id), Some(part)) = (WorkId::new(id), WorkId::new(part)) {
                    split.entry(id).or_default().push(part);
                }
            }
        }
        Ok(Self {
            manifest,
            map,
            shared,
            public,
            stubs,
            retired,
            split,
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

    /// One hop: what `id` was retired into — an older id for a merge,
    /// nothing for a split — or `None` when it is live.
    #[must_use]
    pub fn retired_into(&self, id: WorkId) -> Option<Option<WorkId>> {
        self.retired.get(&id).copied()
    }

    /// The parts a split retired `id` into, in the registry's order;
    /// empty for every other id, and for every id when the base was
    /// written before [`SPLIT`] existed.
    #[must_use]
    pub fn split_into(&self, id: WorkId) -> &[WorkId] {
        self.split.get(&id).map_or(&[], Vec::as_slice)
    }
}
