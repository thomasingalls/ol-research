//! Paths between two works (graph plan §7): a bounded, **directed**,
//! bidirectional breadth-first search, answering "does A cite something
//! that cites … B" and never the undirected question, which wanders up
//! and down the arcs arbitrarily and means nothing. The other direction
//! is the other question, asked by swapping the ends; the layer above
//! offers both rather than merging them.
//!
//! The search is over a caller's neighbourhoods, not over a [`Base`]
//! directly, because the served graph is a base with the edges delta
//! folded over it (serving plan §5) and a path that ignored the delta
//! would answer as of the wrong batch. [`in_base`] is the base alone,
//! for a build's own checks and the report.
//!
//! **Every interior work of a directed path is held.** A step out of a
//! work needs an out-arc, and only a record cites, so a stub — which has
//! citers and no references (plan §11, item 6) — ends a forward walk
//! wherever it appears. The subgraph a path search really moves through
//! is therefore the held-to-held one, and that subgraph's sparsity is
//! what decides how often a path exists at all: on the sample's epoch 7
//! of 2026-09-19, 93,007 distinct held-to-held arcs over 148,210 held
//! works — a mean out-degree of 0.628, an order below the percolation
//! threshold, with the largest weak component holding a quarter of them
//! — so **no path is the ordinary answer** and must read as one. Hence
//! [`Found::None`], which says how far the search looked and that it
//! looked exhaustively, rather than an empty list that reads as a
//! failure. Measured on the same epoch: of 200,000 random held pairs, 4
//! had a directed path within six hops; of 200,000 pairs both of which
//! have a held-to-held arc at all, 72.
//!
//! The bound is honest in both directions. The forward search is grown
//! to `depth.div_ceil(2)` and the backward to `depth / 2`, which between
//! them cover every path of at most `depth` arcs: a path of length *L*
//! has the work at index `L.div_ceil(2)` within both radii. So an
//! intersection that is empty is a proof of absence within the bound,
//! and the least `forward + backward` distance over an intersection that
//! is not empty is the exact shortest length.

use std::collections::HashMap;

use merge::{Sources, WorkId};

use crate::base::Base;

/// What bounds a search: how long a path may be, how many to return, and
/// how much work the answer is worth.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bounds {
    /// The longest path the search will find, in arcs.
    pub depth: u32,
    /// How many paths to return. Each goes through a different meeting
    /// work, so `paths` distinct paths are `paths` distinct middles.
    pub paths: usize,
    /// Works whose arcs the two searches may read between them before
    /// giving up. Exceeding it is [`Found::Stopped`], never a truncated
    /// answer dressed up as a complete one.
    pub visits: u64,
}

impl Bounds {
    /// Six hops, three paths, 250,000 works expanded — what the transport
    /// asks for when a request says nothing.
    pub const DEFAULT: Self = Self {
        depth: 6,
        paths: 3,
        visits: 250_000,
    };
}

impl Default for Bounds {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// One work on a path, with the asserters of the arc that reached it.
/// The first work of a path is reached by no arc and carries
/// [`Sources::NONE`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Step {
    #[allow(missing_docs)]
    pub work: WorkId,
    /// Who asserted the arc from the previous work to this one.
    pub by: Sources,
}

/// How hard the search looked, on every answer: what a reader needs to
/// tell a thin neighbourhood from a wide one that held no path.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Reached {
    /// Works the forward search reached, `from` counted.
    pub forward: u64,
    /// Works the backward search reached, `to` counted.
    pub backward: u64,
    /// Works whose arc lists were read, over both searches.
    pub expanded: u64,
}

/// What a search answers. Never an empty list: absence within the bound
/// is its own answer, and a search that gave up says so rather than
/// passing for one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Found {
    /// Paths, every one of the shortest length there is and each through
    /// a different meeting work, `from` first and `to` last. `hops` is
    /// that length; zero means the two ends are one work, which is the
    /// answer when two names resolve to the same id.
    Paths {
        #[allow(missing_docs)]
        hops: u32,
        #[allow(missing_docs)]
        paths: Vec<Vec<Step>>,
        /// Meeting works at the shortest length, of which `paths` holds
        /// at most [`Bounds::paths`].
        meetings: u64,
        #[allow(missing_docs)]
        reached: Reached,
    },
    /// There is no directed path of `depth` arcs or fewer, and the search
    /// was exhaustive to that bound: an answer, not a failure.
    None {
        #[allow(missing_docs)]
        depth: u32,
        #[allow(missing_docs)]
        reached: Reached,
    },
    /// The visit budget ran out first. There is no path within `proved`
    /// arcs — the levels both searches did finish — and whether there is
    /// one within `depth` is not known. Ask again with a smaller depth,
    /// or from the other end.
    Stopped {
        #[allow(missing_docs)]
        depth: u32,
        /// The bound the answer does hold for.
        proved: u32,
        #[allow(missing_docs)]
        visits: u64,
        #[allow(missing_docs)]
        reached: Reached,
    },
}

/// A neighbourhood: `walk(work, &mut sink)` gives every neighbour of
/// `work` in one orientation with the asserters of the arc between them.
/// A visitor rather than an iterator, so a caller whose lists borrow a
/// lock or a map can hand them over without allocating a vector a work.
type Neighbours<'a> = &'a mut dyn FnMut(WorkId, &mut dyn FnMut(WorkId, Sources));

/// One side's tree: the distance of every work reached and the arc that
/// reached it.
#[derive(Debug, Default)]
struct Side {
    seen: HashMap<WorkId, (u32, Option<(WorkId, Sources)>)>,
    frontier: Vec<WorkId>,
    /// Levels finished, which is the radius the side is complete to.
    done: u32,
}

impl Side {
    fn start(at: WorkId) -> Self {
        Self {
            seen: HashMap::from([(at, (0, None))]),
            frontier: vec![at],
            done: 0,
        }
    }

    /// Grows one level. `budget` is what is left of the visit allowance;
    /// `false` means it ran out inside the level, which leaves `done`
    /// where it was — the radius the side is still complete to.
    fn grow(&mut self, walk: Neighbours<'_>, budget: &mut u64, expanded: &mut u64) -> bool {
        let mut next = Vec::new();
        let depth = self.done + 1;
        for work in std::mem::take(&mut self.frontier) {
            if *budget == 0 {
                return false;
            }
            *budget -= 1;
            *expanded += 1;
            let seen = &mut self.seen;
            walk(work, &mut |other, by| {
                if let std::collections::hash_map::Entry::Vacant(slot) = seen.entry(other) {
                    slot.insert((depth, Some((work, by))));
                    next.push(other);
                }
            });
        }
        self.frontier = next;
        self.done = depth;
        true
    }
}

/// The shortest directed paths from `from` to `to` within the bounds.
///
/// `references(work, sink)` gives the works `work` cites and
/// `citers(work, sink)` the works citing it; both are the caller's, so
/// the search runs over a base, over a base with a delta folded, or over
/// a fixture, without knowing which.
#[must_use]
pub fn find(
    from: WorkId,
    to: WorkId,
    bounds: Bounds,
    mut references: impl FnMut(WorkId, &mut dyn FnMut(WorkId, Sources)),
    mut citers: impl FnMut(WorkId, &mut dyn FnMut(WorkId, Sources)),
) -> Found {
    if from == to {
        return Found::Paths {
            hops: 0,
            paths: vec![vec![Step {
                work: from,
                by: Sources::NONE,
            }]],
            meetings: 1,
            reached: Reached {
                forward: 1,
                backward: 1,
                expanded: 0,
            },
        };
    }
    let (mut ahead, mut behind) = (Side::start(from), Side::start(to));
    let (half_f, half_b) = (bounds.depth.div_ceil(2), bounds.depth / 2);
    let mut budget = bounds.visits;
    let mut expanded = 0u64;
    let mut cut = false;
    // Level by level and alternately, so that a budget that runs out has
    // been spent evenly and `proved` is as large as it can be.
    for level in 1..=half_f.max(half_b) {
        if level <= half_f && !ahead.grow(&mut references, &mut budget, &mut expanded) {
            cut = true;
            break;
        }
        if level <= half_b && !behind.grow(&mut citers, &mut budget, &mut expanded) {
            cut = true;
            break;
        }
    }
    let reached = Reached {
        forward: ahead.seen.len() as u64,
        backward: behind.seen.len() as u64,
        expanded,
    };
    // What the two radii between them cover: every path of at most this
    // many arcs has a work in both trees, so a meeting no longer than
    // this is the shortest there is and its absence is a proof. Without
    // a cut it is exactly `depth`, the two halves adding back up.
    let proved = ahead.done + behind.done;
    // The meeting: over the smaller side, the least sum of the two
    // distances.
    let swapped = ahead.seen.len() > behind.seen.len();
    let (small, large) = if swapped {
        (&behind.seen, &ahead.seen)
    } else {
        (&ahead.seen, &behind.seen)
    };
    let mut best = u32::MAX;
    let mut meetings: Vec<(WorkId, u32)> = Vec::new();
    for (work, (here, _)) in small {
        let Some((there, _)) = large.get(work) else {
            continue;
        };
        let (near, far) = if swapped {
            (*there, *here)
        } else {
            (*here, *there)
        };
        let total = near + far;
        if total < best {
            best = total;
            meetings.clear();
        }
        if total == best {
            meetings.push((*work, near));
        }
    }
    if meetings.is_empty() || best > proved {
        return if cut {
            Found::Stopped {
                depth: bounds.depth,
                proved,
                visits: bounds.visits,
                reached,
            }
        } else {
            Found::None {
                depth: bounds.depth,
                reached,
            }
        };
    }
    // Every shortest path holds exactly one work at forward distance
    // `hops` halved and rounded up — its distances there are forced to be
    // exact — so keeping the meetings at that distance alone makes them
    // and the paths one to one, where a meeting anywhere else along the
    // same path would only reconstruct it again. The fallback stands
    // against nothing the note allows, and keeps an answer an answer.
    let middle = best.div_ceil(2);
    let mut canonical: Vec<WorkId> = meetings
        .iter()
        .filter(|(_, near)| *near == middle)
        .map(|(work, _)| *work)
        .collect();
    if canonical.is_empty() {
        canonical = meetings.into_iter().map(|(work, _)| work).collect();
    }
    // A stable order, since a HashMap's is not one.
    canonical.sort_unstable();
    let count = canonical.len() as u64;
    let paths = canonical
        .iter()
        .take(bounds.paths)
        .map(|meet| path_through(*meet, &ahead.seen, &behind.seen))
        .collect();
    Found::Paths {
        hops: best,
        paths,
        meetings: count,
        reached,
    }
}

type Tree = HashMap<WorkId, (u32, Option<(WorkId, Sources)>)>;

/// The path through one meeting work: the forward tree walked back to
/// `from` and reversed, then the backward tree walked on to `to`. Both
/// trees are breadth-first, so each half is a shortest path, and the two
/// halves share no work — one that was on both would have a smaller
/// distance sum than the meeting chosen, which was the least.
fn path_through(meet: WorkId, ahead: &Tree, behind: &Tree) -> Vec<Step> {
    let mut head = vec![Step {
        work: meet,
        by: Sources::NONE,
    }];
    let mut at = meet;
    while let Some((_, Some((parent, by)))) = ahead.get(&at) {
        // The arc runs parent -> at, so its asserters belong to `at`.
        if let Some(last) = head.last_mut() {
            last.by = *by;
        }
        head.push(Step {
            work: *parent,
            by: Sources::NONE,
        });
        at = *parent;
    }
    head.reverse();
    let mut at = meet;
    while let Some((_, Some((next, by)))) = behind.get(&at) {
        // Here the arc runs at -> next, so its asserters belong to `next`.
        head.push(Step {
            work: *next,
            by: *by,
        });
        at = *next;
    }
    head
}

/// [`find`] over a base alone, with no delta folded: what a build's own
/// checks and the report ask.
#[must_use]
pub fn in_base(base: &Base, from: WorkId, to: WorkId, bounds: Bounds) -> Found {
    find(
        from,
        to,
        bounds,
        |work, sink| {
            for arc in base.references(work) {
                sink(arc.work, arc.by);
            }
        },
        |work, sink| {
            for arc in base.citers(work) {
                sink(arc.work, arc.by);
            }
        },
    )
}

#[cfg(test)]
mod tests {
    use merge::{SourceId, Sources, WorkId};

    use super::{Bounds, Found, Step, find};

    fn id(n: u32) -> WorkId {
        WorkId::new(n).unwrap()
    }

    fn by() -> Sources {
        [SourceId::Pubmed].into_iter().collect()
    }

    /// A chain 1 -> 2 -> 3 -> 4 -> 5, with 1 -> 6 -> 5 and 1 -> 9 -> 5
    /// beside it and 7 alone; 8 is cited by 4 and cites nothing, a
    /// stub's shape.
    fn arcs() -> Vec<(u32, u32)> {
        vec![
            (1, 2),
            (2, 3),
            (3, 4),
            (4, 5),
            (1, 6),
            (6, 5),
            (1, 9),
            (9, 5),
            (4, 8),
        ]
    }

    fn search(from: u32, to: u32, bounds: Bounds) -> Found {
        let edges = arcs();
        find(
            id(from),
            id(to),
            bounds,
            |work, sink| {
                for (a, b) in &edges {
                    if *a == work.get() {
                        sink(id(*b), by());
                    }
                }
            },
            |work, sink| {
                for (a, b) in &edges {
                    if *b == work.get() {
                        sink(id(*a), by());
                    }
                }
            },
        )
    }

    fn works(path: &[Step]) -> Vec<u32> {
        path.iter().map(|s| s.work.get()).collect()
    }

    #[test]
    fn the_shortest_path_is_found_in_both_halves_of_the_bound() {
        // 1 -> 2 -> 3 -> 4: odd length, so the forward half is the larger.
        let Found::Paths { hops, paths, .. } = search(1, 4, Bounds::DEFAULT) else {
            panic!("no path 1 -> 4")
        };
        assert_eq!(hops, 3);
        assert_eq!(works(&paths[0]), [1, 2, 3, 4]);
        // The first work is reached by no arc; every later one carries one.
        assert_eq!(paths[0][0].by, Sources::NONE);
        assert!(paths[0][1..].iter().all(|s| s.by == by()));
    }

    #[test]
    fn several_paths_come_back_one_per_meeting_work() {
        let Found::Paths {
            hops,
            paths,
            meetings,
            ..
        } = search(1, 5, Bounds::DEFAULT)
        else {
            panic!("no path 1 -> 5")
        };
        // 1 -> 6 -> 5 and 1 -> 9 -> 5, both shorter than the chain
        // through 2, 3 and 4; one path a middle and no repeats.
        assert_eq!((hops, meetings), (2, 2));
        assert_eq!(
            paths.iter().map(|p| works(p)).collect::<Vec<_>>(),
            [vec![1, 6, 5], vec![1, 9, 5]]
        );
    }

    #[test]
    fn a_path_is_returned_once_however_many_meetings_lie_on_it() {
        // Every work of 1 -> 2 -> 3 -> 4 meets at the same total, and a
        // meeting off the middle would rebuild the one path again.
        let Found::Paths {
            paths, meetings, ..
        } = search(1, 4, Bounds::DEFAULT)
        else {
            panic!("no path 1 -> 4")
        };
        assert_eq!((meetings, paths.len()), (1, 1));
    }

    #[test]
    fn the_answer_to_no_path_says_how_far_it_looked() {
        // Nothing reaches 7, and the direction is not symmetric: 5 does
        // not cite 1, though 1 reaches 5.
        for (from, to) in [(1u32, 7u32), (5, 1)] {
            let found = search(from, to, Bounds::DEFAULT);
            let Found::None { depth, reached } = found else {
                panic!("{from} -> {to}: {found:?}")
            };
            assert_eq!(depth, 6);
            assert!(reached.forward >= 1 && reached.backward >= 1);
        }
    }

    #[test]
    fn a_bound_too_short_is_none_and_not_a_wrong_path() {
        let bounds = Bounds {
            depth: 2,
            ..Bounds::DEFAULT
        };
        assert!(matches!(search(1, 4, bounds), Found::None { depth: 2, .. }));
        assert!(matches!(search(1, 5, bounds), Found::Paths { hops: 2, .. }));
    }

    #[test]
    fn one_work_to_itself_is_zero_hops() {
        let Found::Paths { hops, paths, .. } = search(3, 3, Bounds::DEFAULT) else {
            panic!("a work is not itself")
        };
        assert_eq!(hops, 0);
        assert_eq!(works(&paths[0]), [3]);
    }

    #[test]
    fn a_spent_budget_is_stopped_and_says_what_it_proved() {
        let bounds = Bounds {
            depth: 6,
            paths: 3,
            visits: 1,
        };
        let found = search(1, 4, bounds);
        let Found::Stopped {
            depth,
            proved,
            visits,
            ..
        } = found
        else {
            panic!("{found:?}")
        };
        assert_eq!((depth, visits), (6, 1));
        // One work expanded: the forward level finished, the backward did
        // not, so one hop is all the answer holds for.
        assert_eq!(proved, 1);
    }
}
