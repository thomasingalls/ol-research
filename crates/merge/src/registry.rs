//! The registry (merge plan §4): the one part of the merged layer that is
//! of record. A rebuild can recompute everything else from the normalized
//! sets; what it cannot recompute is which [`WorkId`] a work was given
//! last time. So that is what this log keeps, append-only like receipts:
//! one JSON line per event, folded into a table at the start of a run,
//! never reused.

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::fs;
use std::io::{self, BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::name::OwnedName;
use crate::work::WorkId;

/// One event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "lowercase", deny_unknown_fields)]
pub enum Line {
    /// A new id, with the names that earned it. Ids are dense: each mint
    /// is the number after the last.
    ///
    /// A name here can already be mapped, and then the later line wins as
    /// it does for [`Line::Name`]. It takes a change to
    /// `publisher::ELSEVIER` to arrange: two spellings of one PII, minted
    /// under two ids by two batches, are one name the day the registrant
    /// joins the list. The fold is what heals it — the next assignment
    /// sees one work claiming both ids and retires the younger into the
    /// older — so a log written under an older list still reads.
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
    /// An id retired because its names split between works and no part
    /// continues it, each part minted anew; the id has no successor.
    /// Since 2026-09-19 (merge plan §11, item 3) this is the fallback:
    /// where exactly one part holds the records, that part reuses the id
    /// and no line of this kind is written.
    Split {
        #[allow(missing_docs)]
        id: WorkId,
        #[allow(missing_docs)]
        into: Vec<WorkId>,
        #[allow(missing_docs)]
        at: String,
    },
}

/// A names table the assignment can run over: the whole log folded
/// ([`Registry`]), or a names base with the log's tail past it (the
/// serving layer's live table, serving plan §5).
pub trait Table {
    /// The live id a name maps to.
    fn lookup(&self, name: &OwnedName) -> Option<WorkId>;
    /// Ids minted so far: the next mint is one more.
    fn minted(&self) -> u32;
    /// Folds one event.
    ///
    /// # Errors
    ///
    /// The event does not fit what came before; see [`Registry::apply`].
    fn apply(&mut self, line: &Line) -> Result<(), String>;
}

/// The log, folded: which names map to which ids, which ids are retired.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Registry {
    minted: u32,
    names: HashMap<OwnedName, WorkId>,
    retired: HashMap<WorkId, Option<WorkId>>,
    /// The parts of every id a split retired, in the order the line
    /// gives them: what an id with no successor can still answer with
    /// (serving plan §11, item 24).
    split: HashMap<WorkId, Vec<WorkId>>,
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
                self.split.insert(*id, into.clone());
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

    /// Appends events, one line each, creating the file and its
    /// directory if needed, and returns once the bytes are on the disk
    /// rather than merely in the kernel — [`journal::append`] has always
    /// done that and the registry is no less of record.
    ///
    /// This is the bare write. A run that can still fail after it wants
    /// [`Pending`] instead, which makes the whole run's lines
    /// all-or-nothing.
    ///
    /// [`journal::append`]: crate::journal::append
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
        file.flush()?;
        file.get_ref().sync_all()
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

    /// Every retired id and what it was retired into: an older id for a
    /// merge, nothing for a split. In no particular order.
    pub fn retirements(&self) -> impl Iterator<Item = (WorkId, Option<WorkId>)> + '_ {
        self.retired.iter().map(|(id, into)| (*id, *into))
    }

    /// The parts a split retired `id` into, in the line's order; empty
    /// for an id no split retired. An id with no successor still answers
    /// with these (serving plan §11, item 24).
    #[must_use]
    pub fn split_into(&self, id: WorkId) -> &[WorkId] {
        self.split.get(&id).map_or(&[], Vec::as_slice)
    }

    /// Every split id with its parts. In no particular order.
    pub fn splits(&self) -> impl Iterator<Item = (WorkId, &[WorkId])> + '_ {
        self.split.iter().map(|(id, into)| (*id, into.as_slice()))
    }

    /// The live id a name maps to.
    #[must_use]
    pub fn lookup(&self, name: &OwnedName) -> Option<WorkId> {
        self.names.get(name).and_then(|id| self.live(*id))
    }

    /// Gives every work an id (merge plan §4); see [`assign`], whose
    /// `held` this passes on. With `complete`, a name the registry maps
    /// that no work here has is dropped, so `complete` is only for a run
    /// over the whole store.
    #[must_use]
    pub fn assign(
        &mut self,
        works: &[Vec<OwnedName>],
        held: usize,
        at: &str,
        complete: bool,
    ) -> Assignment {
        let mut out = assign(self, works, held, at);
        if complete {
            let seen: HashSet<&OwnedName> = works.iter().flatten().collect();
            let dropped = self.drop_unseen(|n| seen.contains(n), at);
            out.dropped += dropped.len() as u64;
            out.lines.extend(dropped);
        }
        out
    }
}

impl Table for Registry {
    fn lookup(&self, name: &OwnedName) -> Option<WorkId> {
        Self::lookup(self, name)
    }

    fn minted(&self) -> u32 {
        self.minted
    }

    fn apply(&mut self, line: &Line) -> Result<(), String> {
        Self::apply(self, line)
    }
}

/// Gives every work an id (merge plan §4): each work's names, given in
/// the deterministic order the plan asks for, are looked up; none known
/// mints, one id reuses, several merge into the oldest, an id several
/// works claim is a split (merge plan §11, item 3).
///
/// `works[..held]` hold records and `works[held..]` are stubs, which is
/// the order the plan's minting asks for anyway and the one fact a
/// split needs: **the part that holds the records keeps the id**, since
/// a name leaving a work is not that work ending, and the part that is
/// names only becomes a stub of its own. Where no part holds records —
/// two stubs a reference no longer cites together — or where several
/// do, no part continues the work: the id is retired with no successor
/// and every part is minted, which is what [`Line::Split`] says. Decided
/// 2026-09-19 on the fourteen splits the corpus holds, every one of them
/// one record-holding part and one bare stub.
///
/// Every line is applied to `table` as it is made, and returned to be
/// appended.
///
/// # Panics
///
/// Never: every line it applies is one the fold accepts by
/// construction.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn assign<T: Table>(
    table: &mut T,
    works: &[Vec<OwnedName>],
    held: usize,
    at: &str,
) -> Assignment {
    {
        let mut out = Assignment::default();
        let claims: Vec<Vec<WorkId>> = works
            .iter()
            .map(|names| {
                let mut ids: Vec<WorkId> = names.iter().filter_map(|n| table.lookup(n)).collect();
                ids.sort_unstable();
                ids.dedup();
                ids
            })
            .collect();
        let mut claimants: HashMap<WorkId, Vec<usize>> = HashMap::new();
        for (w, ids) in claims.iter().enumerate() {
            for id in ids {
                claimants.entry(*id).or_default().push(w);
            }
        }
        // The one part of a contested id that holds records keeps it.
        // The answer depends only on which works claim the id, never on
        // the order this map is walked in.
        let keeper: HashMap<WorkId, usize> = claimants
            .iter()
            .filter(|(_, ws)| ws.len() > 1)
            .filter_map(|(id, ws)| {
                let mut holders = ws.iter().copied().filter(|w| *w < held);
                let first = holders.next()?;
                holders.next().is_none().then_some((*id, first))
            })
            .collect();
        let contested = |id: &WorkId, w: usize| -> bool {
            claimants[id].len() > 1 && keeper.get(id) != Some(&w)
        };
        let mut split_into: HashMap<WorkId, Vec<WorkId>> = HashMap::new();
        for (w, names) in works.iter().enumerate() {
            let unique: Vec<WorkId> = claims[w]
                .iter()
                .copied()
                .filter(|id| !contested(id, w))
                .collect();
            let id = match unique.as_slice() {
                [] => {
                    let id = WorkId::new(table.minted() + 1).expect("ids fit u32");
                    let line = Line::Mint {
                        id,
                        names: names.clone(),
                        at: at.to_owned(),
                    };
                    table.apply(&line).expect("a mint after the last id");
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
                        table.apply(&line).expect("both live, winner older");
                        out.lines.push(line);
                        out.merged += 1;
                    }
                    out.reused += 1;
                    *winner
                }
            };
            for name in names {
                if table.lookup(name) != Some(id) {
                    let line = Line::Name {
                        id,
                        name: name.clone(),
                        at: at.to_owned(),
                    };
                    table.apply(&line).expect("the id is live");
                    out.lines.push(line);
                    out.attached += 1;
                }
            }
            for claimed in &claims[w] {
                if claimants[claimed].len() > 1 && !keeper.contains_key(claimed) {
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
            table.apply(&line).expect("the parts are live");
            out.lines.push(line);
            out.split += 1;
        }
        out
    }
}

impl Registry {
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

/// What a run stages beside the registry while it is still running.
const PENDING: &str = ".pending";

/// What it writes before the sidecar is whole.
const STAGING: &str = ".staging";

/// How much of a torn promotion is compared at a time.
const CHUNK: usize = 64 * 1024;

/// A sidecar's first line: what a run staged, and the registry it staged
/// against.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Head {
    /// Bytes the registry held when the run staged. The payload belongs
    /// exactly there, which is what lets a torn promotion be undone: the
    /// bytes past this offset are the run's own and nobody else's.
    base: u64,
    /// Payload lines after this one.
    lines: u64,
    /// When the run staged them.
    at: String,
}

/// One run's registry lines, staged beside the registry and not of
/// record until [`commit`](Pending::commit) puts them there.
///
/// The registry is the store of record, and a line in it asserts that
/// the run which wrote it concluded. Appending as the lines were
/// computed broke that promise. The user, 2026-09-19: "the registry
/// appends before a merge can fail — cosmetic on a sample, genuinely bad
/// when a corpus merge runs for hours and dies at the end." A run that
/// day died in its second pass and left 39,907 lines behind, eleven of
/// them retiring ids under a split rule that had already been replaced;
/// the log is append-only, so there was no taking them back.
///
/// So a run writes its lines to `registry.jsonl.pending` before the work
/// that can fail, and moves them into the registry in one append after
/// that work is done. A run that dies in between leaves the registry
/// exactly as it found it.
///
/// Why this shape and not the other two we weighed (2026-09-19):
///
/// - Rewriting the whole registry into a sibling and renaming it over
///   would be atomic outright, with no rule to remember. But it costs
///   the log's whole length per run — 472 MiB and 5.35 million lines
///   today — and the incremental merge (serving plan §5) commits on the
///   order of a second, so it could never adopt the same mechanism. A
///   fix only the nightly full merge can use is half a fix.
/// - A begin/commit sentinel in the log itself is the cheapest write,
///   but every reader would have to learn the rule, and one reader
///   cannot: the serving layer's fold (`crates/serve/src/fold.rs`) tails
///   the registry as a *byte range* taken from the journal and hands
///   every line in it to serde, so a sentinel would fail to parse, and
///   [`journal::offset_of`] turns a line count into a byte offset for
///   journal lines written before SP5, so a sentinel would move offsets
///   that published lines already point into. The format is out in the
///   open — epoch 2 is published against it.
///
/// [`journal::offset_of`]: crate::journal::offset_of
///
/// # Durability
///
/// The sidecar is written under `.staging`, `fsync`ed, then renamed to
/// `.pending`, and the directory `fsync`ed: the pending name exists only
/// if the sidecar behind it is whole, since `rename` is atomic on APFS
/// and on ext4 alike. `sync_all` is `F_FULLFSYNC` on macOS, so the bytes
/// are on the disk and not just in its cache.
///
/// The promotion itself — one append of the payload — is not atomic, and
/// cannot be: no filesystem promises that a multi-megabyte write to a
/// regular file is all-or-nothing. What it is instead is *undoable*. The
/// sidecar survives the promotion and records the length the registry
/// had before it, so a process killed mid-append leaves a state the next
/// run can read: see [`recover`], which checks that the registry's tail
/// past that length is byte-for-byte the payload's own prefix and only
/// then takes it back. That is the one place anything is ever removed
/// from the log, and it removes only bytes this sidecar can prove it
/// wrote and no journal line ever sealed.
///
/// Dropping a `Pending` without committing discards it, so every early
/// return in a merge driver leaves the registry alone without having to
/// say so.
#[derive(Debug)]
pub struct Pending {
    registry: PathBuf,
    path: PathBuf,
    head: Head,
    /// Where the payload begins in the sidecar: past the head line.
    from: u64,
    /// Payload bytes.
    bytes: u64,
    /// The append has begun: the sidecar must outlive this run so
    /// [`recover`] can undo what was written.
    promoting: bool,
    /// The lines are of record and the sidecar is gone.
    done: bool,
}

impl Pending {
    /// Stages `lines` beside the registry at `path`, creating the
    /// directory if needed. Nothing of the registry is touched.
    ///
    /// # Errors
    ///
    /// I/O, or a sidecar already there — an earlier run's lines that
    /// [`recover`] has not settled yet, which a second run must not
    /// bury.
    pub fn stage(path: &Path, lines: &[Line]) -> io::Result<Self> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let pending = sibling(path, PENDING);
        if pending.try_exists()? {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!(
                    "{}: an earlier run's lines are still staged; recover before staging more",
                    pending.display()
                ),
            ));
        }
        let head = Head {
            base: length(path)?,
            lines: lines.len() as u64,
            at: ingest::time::now(),
        };
        let staging = sibling(path, STAGING);
        let mut head_text = serde_json::to_string(&head)?;
        head_text.push('\n');
        let mut bytes = 0u64;
        {
            let mut file = io::BufWriter::new(fs::File::create(&staging)?);
            file.write_all(head_text.as_bytes())?;
            for line in lines {
                let mut text = serde_json::to_string(line)?;
                text.push('\n');
                file.write_all(text.as_bytes())?;
                bytes += text.len() as u64;
            }
            file.flush()?;
            file.get_ref().sync_all()?;
        }
        // The rename is what says the staging finished: a sidecar under
        // the pending name is whole, or it is not there at all.
        fs::rename(&staging, &pending)?;
        sync_dir(path)?;
        Ok(Self {
            registry: path.to_path_buf(),
            path: pending,
            from: head_text.len() as u64,
            bytes,
            head,
            promoting: false,
            done: false,
        })
    }

    /// The sidecar's path.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Lines staged.
    #[must_use]
    pub const fn lines(&self) -> u64 {
        self.head.lines
    }

    /// Puts the staged lines in the registry and drops the sidecar:
    /// after this they are of record. Returns the lines committed.
    ///
    /// # Errors
    ///
    /// I/O, or a registry no longer the length it was staged against —
    /// something else wrote to the store of record while this run was
    /// working, and the run's own lines were computed against a table
    /// that is no longer what the file says.
    pub fn commit(mut self) -> io::Result<u64> {
        let len = length(&self.registry)?;
        if len != self.head.base {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "{}: {len} bytes where this run staged against {}; something else wrote to it",
                    self.registry.display(),
                    self.head.base
                ),
            ));
        }
        let mut source = fs::File::open(&self.path)?;
        source.seek(SeekFrom::Start(self.from))?;
        let mut target = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.registry)?;
        // Past here the registry may hold part of the payload, so the
        // sidecar has to survive a failure for `recover` to undo it.
        self.promoting = true;
        let copied = io::copy(&mut source, &mut target)?;
        target.sync_all()?;
        if copied != self.bytes {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "{}: {copied} bytes promoted where {} were staged",
                    self.path.display(),
                    self.bytes
                ),
            ));
        }
        drop(target);
        fs::remove_file(&self.path)?;
        sync_dir(&self.registry)?;
        self.done = true;
        Ok(self.head.lines)
    }

    /// Throws the staged lines away: the run did not conclude.
    ///
    /// # Errors
    ///
    /// I/O.
    pub fn discard(mut self) -> io::Result<()> {
        self.done = true;
        remove(&self.path)?;
        sync_dir(&self.registry)
    }
}

impl Drop for Pending {
    fn drop(&mut self) {
        if self.done {
            return;
        }
        if self.promoting {
            eprintln!(
                "{}: a promotion did not finish; the next run's recovery takes it back",
                self.path.display()
            );
            return;
        }
        if let Err(e) = remove(&self.path) {
            eprintln!("{}: {e}", self.path.display());
        }
    }
}

/// What [`recover`] found and did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Recovered {
    /// A run staged its lines and never promoted them; the registry
    /// never held them.
    Discarded {
        #[allow(missing_docs)]
        lines: u64,
        /// When the run that staged them staged them.
        at: String,
    },
    /// A promotion the process died inside: the bytes it had written are
    /// taken back.
    RolledBack {
        /// Bytes removed from the registry.
        bytes: u64,
        /// The length it is back to.
        to: u64,
        #[allow(missing_docs)]
        lines: u64,
        #[allow(missing_docs)]
        at: String,
    },
    /// A promotion that finished and did not get to drop its sidecar;
    /// the lines are of record and stay.
    Completed {
        #[allow(missing_docs)]
        lines: u64,
        #[allow(missing_docs)]
        at: String,
    },
}

impl fmt::Display for Recovered {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Discarded { lines, at } => write!(
                f,
                "{lines} lines staged at {at} by a run that did not finish; discarded, the registry never held them"
            ),
            Self::RolledBack {
                bytes,
                to,
                lines,
                at,
            } => write!(
                f,
                "a promotion of {lines} lines staged at {at} died after {bytes} bytes; taken back to {to} bytes"
            ),
            Self::Completed { lines, at } => write!(
                f,
                "{lines} lines staged at {at} were promoted in full; the sidecar is dropped and they stay of record"
            ),
        }
    }
}

/// Settles whatever an earlier run left staged beside the registry at
/// `path`, so the next run reads a registry that is wholly of record.
/// Call it under the merge lock, before [`Registry::read`].
///
/// `sealed` is the byte offset a journal line has already told readers
/// about — the serving layer tails the registry to exactly there (serving
/// plan §5) — and recovery refuses to touch anything below it. Pass 0
/// when there is no journal.
///
/// Returns what it did, or `None` when there was nothing staged.
///
/// # Errors
///
/// I/O, a sidecar that does not parse, or a registry that the sidecar
/// cannot account for: recovery removes bytes from the store of record
/// and will only do so where it can prove they are the sidecar's own and
/// no reader was ever told about them. Every refusal is loud and leaves
/// both files alone; the fix is a person looking at them.
pub fn recover(path: &Path, sealed: u64) -> io::Result<Option<Recovered>> {
    let staging = sibling(path, STAGING);
    if staging.try_exists()? {
        // A stage that never reached its rename. It was never whole and
        // the registry never saw a byte of it.
        remove(&staging)?;
    }
    let pending = sibling(path, PENDING);
    if !pending.try_exists()? {
        return Ok(None);
    }
    let bad = |what: String| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{}: {what}", pending.display()),
        )
    };
    let mut reader = BufReader::new(fs::File::open(&pending)?);
    let mut head_text = String::new();
    reader.read_line(&mut head_text)?;
    if !head_text.ends_with('\n') {
        return Err(bad("no whole first line".to_owned()));
    }
    let head: Head =
        serde_json::from_str(head_text.trim_end()).map_err(|e| bad(format!("first line: {e}")))?;
    let from = head_text.len() as u64;
    let staged = length(&pending)?
        .checked_sub(from)
        .ok_or_else(|| bad("shorter than its own first line".to_owned()))?;
    let len = length(path)?;
    let at = head.at.clone();
    if head.base < sealed {
        return Err(bad(format!(
            "staged against {} bytes of {}, which batch offset {sealed} already sealed",
            head.base,
            path.display()
        )));
    }
    if len < head.base || len > head.base + staged {
        return Err(bad(format!(
            "{} is {len} bytes, not between the {} it was staged against and the {} a whole promotion reaches",
            path.display(),
            head.base,
            head.base + staged
        )));
    }
    let written = len - head.base;
    if written > 0 && !same(path, head.base, &pending, from, written)? {
        return Err(bad(format!(
            "{} holds {written} bytes past {} that are not this run's; nothing was touched",
            path.display(),
            head.base
        )));
    }
    let out = if written == 0 {
        Recovered::Discarded {
            lines: head.lines,
            at,
        }
    } else if written == staged {
        Recovered::Completed {
            lines: head.lines,
            at,
        }
    } else {
        let file = fs::OpenOptions::new().write(true).open(path)?;
        file.set_len(head.base)?;
        file.sync_all()?;
        Recovered::RolledBack {
            bytes: written,
            to: head.base,
            lines: head.lines,
            at,
        }
    };
    remove(&pending)?;
    sync_dir(path)?;
    Ok(Some(out))
}

/// `path` with `suffix` on the end of its file name.
fn sibling(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(suffix);
    path.with_file_name(name)
}

/// A file's length; zero when it is not there.
fn length(path: &Path) -> io::Result<u64> {
    match fs::metadata(path) {
        Ok(meta) => Ok(meta.len()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(0),
        Err(e) => Err(e),
    }
}

/// Removes a file, saying nothing when it is already gone.
fn remove(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

/// `fsync`s the directory a file lives in, so a rename or an unlink in
/// it is on the disk and not only in the kernel's idea of it.
fn sync_dir(path: &Path) -> io::Result<()> {
    let dir = match path.parent() {
        Some(dir) if !dir.as_os_str().is_empty() => dir.to_path_buf(),
        _ => PathBuf::from("."),
    };
    fs::File::open(dir)?.sync_all()
}

/// A file open and seeked to `at`.
fn open_at(path: &Path, at: u64) -> io::Result<fs::File> {
    let mut file = fs::File::open(path)?;
    file.seek(SeekFrom::Start(at))?;
    Ok(file)
}

/// Whether `len` bytes of `a` from `at` are the same as `len` bytes of
/// `b` from `from`.
fn same(a: &Path, at: u64, b: &Path, from: u64, len: u64) -> io::Result<bool> {
    let mut left = open_at(a, at)?;
    let mut right = open_at(b, from)?;
    let mut lb = vec![0u8; CHUNK];
    let mut rb = vec![0u8; CHUNK];
    let mut todo = len;
    while todo > 0 {
        let n = usize::try_from(todo).unwrap_or(CHUNK).min(CHUNK);
        left.read_exact(&mut lb[..n])?;
        right.read_exact(&mut rb[..n])?;
        if lb[..n] != rb[..n] {
            return Ok(false);
        }
        todo -= n as u64;
    }
    Ok(true)
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
        let first = r.assign(&works, works.len(), "t0", true);
        assert_eq!(first.ids, [id(1), id(2), id(3)]);
        assert_eq!(
            (first.minted, first.reused, first.attached, first.dropped),
            (3, 0, 0, 0)
        );
        assert_eq!(first.lines.len(), 3);
        let again = r.assign(&works, works.len(), "t1", true);
        assert_eq!(again.ids, first.ids);
        assert!(again.lines.is_empty(), "{:?}", again.lines);
        assert_eq!(again.reused, 3);
        let mut fresh = Registry::default();
        let other = fresh.assign(&works, works.len(), "t0", true);
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
            3,
            "t0",
            true,
        );
        // Work 1 gains a name; works 1 and 2 turn out to be one; work 3 splits.
        let works = vec![
            vec![n("doi:10.1000/a"), n("pmid:2"), n("openalex:W9")],
            vec![n("pmid:3")],
            vec![n("pmc:PMC3")],
        ];
        let a = r.assign(&works, works.len(), "t1", true);
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
        let b = r.assign(&[vec![n("pmid:2")]], 1, "t2", true);
        assert_eq!(b.ids, [id(1)]);
        assert_eq!(b.dropped, 4, "the doi, W9 and the split parts' names");
        assert_eq!(r.lookup(&n("doi:10.1000/a")), None);
        // An incomplete run drops nothing.
        let mut r2 = Registry::default();
        let _ = r2.assign(&[vec![n("pmid:1")], vec![n("pmid:2")]], 2, "t0", true);
        let c = r2.assign(&[vec![n("pmid:1")]], 1, "t1", false);
        assert_eq!(c.dropped, 0);
        assert_eq!(r2.names(), 2);
    }

    #[test]
    fn the_part_holding_the_records_keeps_a_contested_id() {
        // Merge plan §11, item 3, 2026-09-19. One work holds the PMID
        // and a new DOI, a reference still cites the old DOI: the held
        // work keeps the id, the stub is minted, and no split is
        // written. The same contest between two works that both hold
        // records has no part to continue it, and is retired instead.
        let mut r = Registry::default();
        let _ = r.assign(&[vec![n("doi:10.1000/old"), n("pmid:1")]], 1, "t0", true);
        let held = vec![n("doi:10.1000/new"), n("pmid:1")];
        let stub = vec![n("doi:10.1000/old")];
        let a = r.assign(&[held, stub], 1, "t1", true);
        assert_eq!(a.ids, [id(1), id(2)], "the held work kept its id");
        assert_eq!((a.minted, a.reused, a.merged, a.split), (1, 1, 0, 0));
        assert_eq!(r.live(id(1)), Some(id(1)));
        assert_eq!(r.lookup(&n("doi:10.1000/old")), Some(id(2)));
        assert_eq!(r.lookup(&n("doi:10.1000/new")), Some(id(1)));
        assert!(r.split_into(id(1)).is_empty());
        // Two holders: neither continues it.
        let mut r = Registry::default();
        let _ = r.assign(&[vec![n("pmid:1"), n("pmc:PMC1")]], 1, "t0", true);
        let b = r.assign(&[vec![n("pmid:1")], vec![n("pmc:PMC1")]], 2, "t1", true);
        assert_eq!(b.ids, [id(2), id(3)]);
        assert_eq!((b.minted, b.reused, b.merged, b.split), (2, 0, 0, 1));
        assert_eq!(r.live(id(1)), None);
        assert_eq!(r.split_into(id(1)), [id(2), id(3)]);
        // And two stubs: no part holds records either, as before.
        let mut r = Registry::default();
        let _ = r.assign(&[vec![n("pmid:1"), n("pmc:PMC1")]], 0, "t0", true);
        let c = r.assign(&[vec![n("pmid:1")], vec![n("pmc:PMC1")]], 0, "t1", true);
        assert_eq!((c.minted, c.split), (2, 1));
        assert_eq!(r.split_into(id(1)), [id(2), id(3)]);
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
            2,
            "2026-09-15T00:00:00Z",
            true,
        );
        Registry::append(&path, &a.lines).unwrap();
        let b = r.assign(
            &[vec![n("doi:10.1000/a"), n("pmid:2")]],
            1,
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

    /// The registry holds names sealed under whatever `publisher::ELSEVIER`
    /// said at the time, and on 2026-09-19 the merge died reading one:
    /// `"reg1006:S0003-2697(96)90292-4" is not a name: not canonical`,
    /// line 5,310,863 of `data/merged/registry.jsonl`. This is the shape
    /// of that log — two spellings of one PII, minted under separate ids
    /// by separate batches, under a registrant that rewrites them.
    #[test]
    fn a_log_written_before_the_list_grew_still_folds() {
        let dir = store("grew");
        let path = dir.join("registry.jsonl");
        // The real pair, from the registry, under the registrant the
        // list already rewrites: what `reg1006:` becomes the day 10.1006
        // joins it. The article is 10.1006/jmbi.1998.2346.
        let punctuated = "reg1016:S0022-2836(98)92346-4";
        let bare = "reg1016:S0022283698923464";
        fs::write(
            &path,
            format!(
                r#"{{"op":"mint","id":1,"names":["doi:10.1006/jmbi.1998.2346","{punctuated}"],"at":"t0"}}
{{"op":"mint","id":2,"names":["pmid:9847239","{bare}"],"at":"t1"}}
"#
            ),
        )
        .unwrap();
        let mut r = Registry::read(&path).unwrap();
        assert_eq!(r.minted(), 2);
        assert_eq!(r.names(), 3, "the DOI, the PMID, and one publisher id");
        // One name, whichever spelling asks, and the later line wins as
        // it does for any name that moved between ids.
        assert_eq!(r.lookup(&n(punctuated)), Some(id(2)));
        assert_eq!(r.lookup(&n(bare)), Some(id(2)));
        // The two works the two batches minted are one work now, so the
        // next complete run retires the younger id into the older and
        // drops nothing: the older spelling is still vouched for.
        let works = vec![vec![
            n("doi:10.1006/jmbi.1998.2346"),
            n("pmid:9847239"),
            n(bare),
        ]];
        let a = r.assign(&works, 1, "t2", true);
        assert_eq!(a.ids, [id(1)]);
        assert_eq!((a.merged, a.attached, a.dropped), (1, 0, 0));
        assert_eq!(r.live(id(2)), Some(id(1)));
        fs::remove_dir_all(dir).unwrap();
    }

    /// A temporary store no test shares.
    fn store(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ol-rht-registry-{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A pass 2 that genuinely fails: a directory that cannot be made
    /// because a regular file sits where its parent would go. No mock —
    /// the kernel refuses it.
    fn a_phase_that_fails(dir: &std::path::Path) -> std::io::Error {
        let occupied = dir.join("occupied");
        fs::write(&occupied, b"a file, not a directory").unwrap();
        fs::create_dir_all(occupied.join("v2.8")).unwrap_err()
    }

    /// Run one, which concludes: the registry from nothing.
    fn one_run(path: &std::path::Path) -> Registry {
        let works = vec![
            vec![n("doi:10.1000/a"), n("pmid:1")],
            vec![n("doi:10.1000/b")],
            vec![n("pmc:PMC3")],
        ];
        let mut r = Registry::read(path).unwrap();
        let a = r.assign(&works, works.len(), "t0", true);
        let staged = super::Pending::stage(path, &a.lines).unwrap();
        assert_eq!(staged.commit().unwrap(), 3);
        r
    }

    /// The second run's lines: a merge, a mint and a drop.
    fn a_second_run(r: &mut Registry) -> Vec<Line> {
        let works = vec![
            vec![n("doi:10.1000/a"), n("doi:10.1000/b")],
            vec![n("pmc:PMC3")],
            vec![n("openalex:W9")],
        ];
        r.assign(&works, 2, "t1", true).lines
    }

    #[test]
    fn a_run_that_dies_after_the_registry_leaves_it_as_it_was() {
        // The bug of 2026-09-19. Before the sidecar, the driver appended
        // here and pass 2 ran afterwards; a pass 2 that failed left the
        // run's lines of record with no run behind them.
        let dir = store("partial");
        let path = dir.join("registry.jsonl");
        let mut r = one_run(&path);
        let before = fs::read(&path).unwrap();
        assert_eq!(before.split(|b| *b == b'\n').count() - 1, 3);

        let lines = a_second_run(&mut r);
        assert_eq!(lines.len(), 3, "a merge, a mint and a drop: {lines:?}");
        let pending = super::Pending::stage(&path, &lines).unwrap();
        assert_eq!(pending.lines(), 3);
        assert!(pending.path().exists());
        assert_eq!(
            fs::read(&path).unwrap(),
            before,
            "staging does not touch the registry"
        );

        // Pass 2 fails, so the run returns — and `pending` is dropped on
        // the way out, as every early return in the driver drops it.
        let e = a_phase_that_fails(&dir);
        assert_eq!(e.kind(), std::io::ErrorKind::NotADirectory, "{e}");
        drop(pending);

        assert_eq!(
            fs::read(&path).unwrap(),
            before,
            "the store of record is byte for byte what it was"
        );
        assert!(!super::sibling(&path, super::PENDING).exists());
        let read = Registry::read(&path).unwrap();
        assert_eq!(read.lines(), 3);
        assert_eq!(read.minted(), 3);
        assert_eq!(read.live(id(2)), Some(id(2)), "id 2 was never retired");
        assert_eq!(super::recover(&path, 0).unwrap(), None, "nothing left over");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_run_that_finishes_puts_its_lines_of_record() {
        let dir = store("whole");
        let path = dir.join("registry.jsonl");
        let mut r = one_run(&path);
        let before = fs::read_to_string(&path).unwrap();
        let lines = a_second_run(&mut r);
        let pending = super::Pending::stage(&path, &lines).unwrap();
        let sidecar = pending.path().to_path_buf();
        assert_eq!(pending.commit().unwrap(), 3);
        assert!(!sidecar.exists(), "the sidecar is gone");

        let text = fs::read_to_string(&path).unwrap();
        assert!(text.starts_with(&before), "the log only ever grew");
        assert_eq!(text.lines().count(), 6);
        let read = Registry::read(&path).unwrap();
        assert_eq!(read, r, "the file folds to what the run had in memory");
        assert_eq!(read.live(id(2)), Some(id(1)));
        assert_eq!(read.lookup(&n("pmid:1")), None, "dropped");
        assert_eq!(super::recover(&path, 0).unwrap(), None);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_promotion_the_process_died_inside_is_taken_back() {
        // The one state a kill can leave that the sidecar cannot avoid:
        // part of the payload appended. There is no honest way to make a
        // process die inside `io::copy` from a test, so the state it
        // leaves is built here — the registry with a prefix of the
        // payload on it and the sidecar still beside it — and recovery
        // is held to its contract against it.
        let dir = store("torn");
        let path = dir.join("registry.jsonl");
        let mut r = one_run(&path);
        let before = fs::read(&path).unwrap();
        let base = before.len() as u64;
        let lines = a_second_run(&mut r);
        let pending = super::Pending::stage(&path, &lines).unwrap();
        let sidecar = pending.path().to_path_buf();
        std::mem::forget(pending); // the process died: no Drop ran

        let payload: Vec<u8> = {
            let whole = fs::read(&sidecar).unwrap();
            let head = whole.iter().position(|b| *b == b'\n').unwrap() + 1;
            whole[head..].to_vec()
        };
        // Half a payload, cut inside a line.
        let torn = payload.len() / 2;
        assert_ne!(payload[torn - 1], b'\n', "cut inside a line, not between");
        let mut half = before.clone();
        half.extend_from_slice(&payload[..torn]);
        fs::write(&path, &half).unwrap();
        assert!(
            Registry::read(&path).is_err(),
            "and the store is unreadable until this is undone"
        );

        let done = super::recover(&path, base).unwrap().unwrap();
        assert!(
            matches!(&done, super::Recovered::RolledBack { bytes, to, lines, .. }
                if *bytes == torn as u64 && *to == base && *lines == 3),
            "{done}"
        );
        assert_eq!(fs::read(&path).unwrap(), before, "back to where it was");
        assert!(!sidecar.exists());
        assert_eq!(Registry::read(&path).unwrap().lines(), 3);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_promotion_that_finished_without_dropping_its_sidecar_stands() {
        let dir = store("finished");
        let path = dir.join("registry.jsonl");
        let mut r = one_run(&path);
        let base = fs::metadata(&path).unwrap().len();
        let lines = a_second_run(&mut r);
        let pending = super::Pending::stage(&path, &lines).unwrap();
        let sidecar = pending.path().to_path_buf();
        std::mem::forget(pending);
        let whole = fs::read(&sidecar).unwrap();
        let head = whole.iter().position(|b| *b == b'\n').unwrap() + 1;
        let mut all = fs::read(&path).unwrap();
        all.extend_from_slice(&whole[head..]);
        fs::write(&path, &all).unwrap();

        let done = super::recover(&path, base).unwrap().unwrap();
        assert!(
            matches!(&done, super::Recovered::Completed { lines, .. } if *lines == 3),
            "{done}"
        );
        assert_eq!(fs::read(&path).unwrap(), all, "the lines stay");
        assert!(!sidecar.exists());
        assert_eq!(Registry::read(&path).unwrap(), r);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn recovery_refuses_what_it_cannot_prove_is_its_own() {
        let dir = store("refuses");
        let path = dir.join("registry.jsonl");
        let mut r = one_run(&path);
        let before = fs::read(&path).unwrap();
        let base = before.len() as u64;
        let lines = a_second_run(&mut r);
        let sidecar = {
            let pending = super::Pending::stage(&path, &lines).unwrap();
            let p = pending.path().to_path_buf();
            std::mem::forget(pending);
            p
        };
        let kept = fs::read(&sidecar).unwrap();

        // A tail that is not this run's: recovery will not remove it.
        let mut other = before.clone();
        other.extend_from_slice(b"{\"op\":\"mint\",\"id\":4,\"names\":[],\"at\":\"x\"}\n");
        fs::write(&path, &other).unwrap();
        let e = super::recover(&path, base).unwrap_err().to_string();
        assert!(e.contains("are not this run's"), "{e}");
        assert_eq!(fs::read(&path).unwrap(), other, "untouched");
        assert!(sidecar.exists(), "and the sidecar is kept for a person");

        // A registry shorter than the sidecar was staged against.
        fs::write(&path, &before[..before.len() - 40]).unwrap();
        let e = super::recover(&path, 0).unwrap_err().to_string();
        assert!(e.contains("not between the"), "{e}");

        // A sidecar reaching below what a journal line already sealed.
        fs::write(&path, &before).unwrap();
        let e = super::recover(&path, base + 1).unwrap_err().to_string();
        assert!(e.contains("already sealed"), "{e}");
        assert_eq!(fs::read(&sidecar).unwrap(), kept, "nor is it rewritten");

        // Settled, once the state makes sense again.
        assert!(matches!(
            super::recover(&path, base).unwrap(),
            Some(super::Recovered::Discarded { .. })
        ));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_second_run_will_not_stage_over_the_first() {
        let dir = store("twice");
        let path = dir.join("registry.jsonl");
        let mut r = one_run(&path);
        let lines = a_second_run(&mut r);
        let first = super::Pending::stage(&path, &lines).unwrap();
        let e = super::Pending::stage(&path, &lines).unwrap_err();
        assert_eq!(e.kind(), std::io::ErrorKind::AlreadyExists);
        assert!(e.to_string().contains("recover before staging more"), "{e}");
        drop(first);
        // And a stage that never reached its rename is nobody's: the
        // registry never saw a byte of it.
        let staging = super::sibling(&path, super::STAGING);
        fs::write(&staging, b"half a line").unwrap();
        assert_eq!(super::recover(&path, 0).unwrap(), None);
        assert!(!staging.exists());
        assert!(super::Pending::stage(&path, &lines).is_ok());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_partial_run_stages_nothing_at_all() {
        // `--limit` and `--survey` computed lines the driver never
        // wrote; they must not leave a sidecar either.
        let dir = store("empty");
        let path = dir.join("registry.jsonl");
        let _ = one_run(&path);
        let before = fs::read(&path).unwrap();
        let pending = super::Pending::stage(&path, &[]).unwrap();
        assert_eq!(pending.lines(), 0);
        assert_eq!(pending.commit().unwrap(), 0);
        assert_eq!(fs::read(&path).unwrap(), before);
        assert_eq!(super::recover(&path, 0).unwrap(), None);
        fs::remove_dir_all(&dir).unwrap();
    }
}
