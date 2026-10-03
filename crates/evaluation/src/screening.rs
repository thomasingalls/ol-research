//! A screening set (evaluation plan §14.3): the works a small index is
//! built over, so that an index-time change can be tried in the time a
//! small build takes — every work a suite judged, since a variant must be
//! able to find them, and a sample of the rest of the corpus as
//! distractors, since it must find them among many others.
//!
//! The set is a file, drawn once and kept: `works.tsv`, a work's public id
//! and why it is there, a line each, in the ids' order.

use std::collections::{BTreeMap, BinaryHeap};
use std::io::{self, Write};

use crate::suite::Suite;
use crate::trec;

/// The set's file.
pub const WORKS: &str = "works.tsv";

/// Why a work is in the set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Why {
    /// A suite's key judged it, at any grade and in either half.
    Judged,
    /// A query's own seed, which its run leaves out.
    Seed,
    /// Drawn from the rest of the corpus.
    Sample,
}

impl Why {
    /// As the file spells it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Judged => "judged",
            Self::Seed => "seed",
            Self::Sample => "sample",
        }
    }
}

/// A screening set: each work's public id, and why it is there.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Set {
    #[allow(missing_docs)]
    pub works: BTreeMap<String, Why>,
}

/// A set, and what drawing it counted.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Drawn {
    #[allow(missing_docs)]
    pub set: Set,
    /// Works the corpus's index holds.
    pub held: u64,
    /// Works the suites judged or seeded that the index does not hold — a
    /// work known only as a citation has no text to index — and so are
    /// not in the set.
    pub not_held: u64,
}

/// A set being drawn, the corpus's held works given to it one at a time —
/// there are too many to hold: every work the suites judged or named as a
/// seed that is held, and the `n` other held works whose draw is
/// smallest. A work's draw is its own — its id's and a seed's — so a
/// larger sample holds every smaller one, and the set is the same in
/// whatever order the held works come.
pub struct Draw<'a, F> {
    wanted: BTreeMap<&'a str, Why>,
    found: BTreeMap<String, Why>,
    /// The smallest draws, the largest on top.
    sample: BinaryHeap<(u64, String)>,
    n: usize,
    place: F,
    held: u64,
}

impl<F> std::fmt::Debug for Draw<'_, F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Draw")
            .field("wanted", &self.wanted.len())
            .field("n", &self.n)
            .field("held", &self.held)
            .finish_non_exhaustive()
    }
}

impl<'a, F: Fn(&str) -> u64> Draw<'a, F> {
    /// A draw of `suites`' works and a sample of `n` others, each placed
    /// by `draw`.
    pub fn new(suites: impl IntoIterator<Item = &'a Suite>, n: usize, draw: F) -> Self {
        let mut wanted: BTreeMap<&str, Why> = BTreeMap::new();
        for suite in suites {
            for filter in suite.filters.values() {
                if let Some(seed) = &filter.exclude {
                    wanted.entry(seed).or_insert(Why::Seed);
                }
            }
            for work in suite.key.queries.values().flat_map(BTreeMap::keys) {
                wanted.insert(work, Why::Judged);
            }
        }
        Self {
            wanted,
            found: BTreeMap::new(),
            sample: BinaryHeap::new(),
            n,
            place: draw,
            held: 0,
        }
    }

    /// One work the corpus's index holds, by its public id.
    pub fn held(&mut self, work: &str) {
        self.held += 1;
        if let Some(why) = self.wanted.get(work) {
            self.found.insert(work.to_owned(), *why);
            return;
        }
        let place = (self.place)(work);
        if self.sample.len() < self.n {
            self.sample.push((place, work.to_owned()));
        } else if self
            .sample
            .peek()
            .is_some_and(|(largest, of)| (place, work) < (*largest, of.as_str()))
        {
            self.sample.pop();
            self.sample.push((place, work.to_owned()));
        }
    }

    /// The set, and what drawing it counted.
    #[must_use]
    pub fn finish(self) -> Drawn {
        let not_held = (self.wanted.len() - self.found.len()) as u64;
        let mut works = self.found;
        works.extend(self.sample.into_iter().map(|(_, work)| (work, Why::Sample)));
        Drawn {
            set: Set { works },
            held: self.held,
            not_held,
        }
    }
}

impl Set {
    /// A set drawn from `held` at once: [`Draw`], for works few enough to
    /// be given together.
    pub fn draw<'a>(
        suites: impl IntoIterator<Item = &'a Suite>,
        held: impl IntoIterator<Item = String>,
        n: usize,
        draw: impl Fn(&str) -> u64,
    ) -> Drawn {
        let mut drawing = Draw::new(suites, n, draw);
        for work in held {
            drawing.held(&work);
        }
        drawing.finish()
    }

    /// How many of the set's works are there for `why`.
    #[must_use]
    pub fn count(&self, why: Why) -> usize {
        self.works.values().filter(|w| **w == why).count()
    }

    /// Reads the set's file.
    ///
    /// # Errors
    ///
    /// A line that is not a work and why, or a work named twice.
    pub fn read(text: &str) -> Result<Self, trec::Error> {
        let mut works = BTreeMap::new();
        for (n, line) in text.lines().enumerate() {
            let error = |what: String| trec::Error { line: n + 1, what };
            let why = match line.split_once('\t') {
                Some((work, "judged")) if !work.is_empty() => (work, Why::Judged),
                Some((work, "seed")) if !work.is_empty() => (work, Why::Seed),
                Some((work, "sample")) if !work.is_empty() => (work, Why::Sample),
                _ => return Err(error(format!("not a work and why it is here: {line:?}"))),
            };
            if works.insert(why.0.to_owned(), why.1).is_some() {
                return Err(error(format!("{} again", why.0)));
            }
        }
        Ok(Self { works })
    }

    /// Writes the set's file, the works in their ids' order.
    ///
    /// # Errors
    ///
    /// Writing.
    pub fn write(&self, out: &mut impl Write) -> io::Result<()> {
        for (work, why) in &self.works {
            writeln!(out, "{work}\t{}", why.name())?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;
    use crate::suite::Filter;
    use crate::trec::Key;

    /// A draw that is a work's own and nothing else's: FNV-1a of its id.
    fn place(id: &str) -> u64 {
        id.bytes().fold(0xcbf2_9ce4_8422_2325, |h, b| {
            (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
        })
    }

    fn suite(judged: &[&str], seed: Option<&str>) -> Suite {
        Suite {
            queries: vec![("q".into(), "text".into())],
            key: Key {
                queries: [(
                    "q".to_owned(),
                    judged
                        .iter()
                        .enumerate()
                        .map(|(i, w)| ((*w).to_owned(), i64::from(i % 2 == 0)))
                        .collect(),
                )]
                .into(),
            },
            filters: [(
                "q".to_owned(),
                Filter {
                    until: None,
                    exclude: seed.map(str::to_owned),
                },
            )]
            .into(),
            halves: BTreeMap::new(),
        }
    }

    #[test]
    fn judged_works_are_in_whatever_their_grade_and_only_if_held() {
        let suite = suite(&["w1", "w2", "stub"], Some("w3"));
        let held = ["w1", "w2", "w3", "w4", "w5", "w6"].map(str::to_owned);
        let drawn = Set::draw([&suite], held, 2, place);
        assert_eq!(drawn.held, 6);
        assert_eq!(drawn.not_held, 1, "the stub is judged and not held");
        assert_eq!(drawn.set.works["w1"], Why::Judged);
        assert_eq!(
            drawn.set.works["w2"],
            Why::Judged,
            "judged not relevant is judged"
        );
        assert_eq!(drawn.set.works["w3"], Why::Seed);
        assert_eq!(drawn.set.count(Why::Sample), 2);
        assert!(!drawn.set.works.contains_key("stub"));
        let mut file = Vec::new();
        drawn.set.write(&mut file).unwrap();
        assert_eq!(
            Set::read(std::str::from_utf8(&file).unwrap()).unwrap(),
            drawn.set
        );
        assert!(Set::read("w1\tjudged\nw1\tsample\n").is_err());
        assert!(Set::read("w1\tother\n").is_err());
        assert!(Set::read("w1\n").is_err());
    }

    proptest! {
        /// A set holds every judged work that is held and the sample asked
        /// for, or every other held work when there are fewer; it is the
        /// same in whatever order the held works come; and a larger sample
        /// holds every smaller one.
        #[test]
        fn a_set_is_its_judged_works_and_a_nested_sample(
            held in prop::collection::btree_set("[a-z]{1,3}", 0..60),
            judged in prop::collection::vec("[a-z]{1,3}", 0..12),
            n in 0usize..30,
            more in 0usize..30,
            turn in any::<prop::sample::Index>(),
        ) {
            let judged: Vec<&str> = judged.iter().map(String::as_str).collect();
            let suite = suite(&judged, None);
            let forward: Vec<String> = held.iter().cloned().collect();
            let mut turned = forward.clone();
            if !turned.is_empty() {
                let by = turn.index(turned.len());
                turned.rotate_left(by);
                turned.reverse();
            }
            let drawn = Set::draw([&suite], forward.clone(), n, place);
            prop_assert_eq!(&drawn, &Set::draw([&suite], turned, n, place));
            let judged_held = judged.iter().filter(|w| held.contains(**w)).collect::<std::collections::BTreeSet<_>>();
            prop_assert_eq!(drawn.set.count(Why::Judged), judged_held.len());
            prop_assert_eq!(drawn.set.count(Why::Sample), n.min(held.len() - judged_held.len()));
            prop_assert!(drawn.set.works.keys().all(|w| held.contains(w)));
            let larger = Set::draw([&suite], forward, n + more, place);
            prop_assert!(drawn.set.works.iter().all(|(w, why)| larger.set.works.get(w) == Some(why)));
        }
    }
}
