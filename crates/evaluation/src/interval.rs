//! Error bars (evaluation plan §4): a 95% interval for a mean over a
//! suite's queries, from resampling the queries with replacement.
//!
//! Every interval must reproduce, so nothing here is a library's: the
//! generator is `SplitMix64` as its author published it, indices are drawn
//! from it by Lemire's multiply-and-reject, and the seed and the number
//! of resamples are constants a change to which is a change to this file.

/// Resamples per interval (decided 2026-09-26).
pub const RESAMPLES: usize = 10_000;

/// The generator's seed. Changing it moves every interval ever printed.
pub const SEED: u64 = 0x6f6c_2d72_6874_0001;

/// A 95% interval.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Interval {
    #[allow(missing_docs)]
    pub low: f64,
    #[allow(missing_docs)]
    pub high: f64,
}

/// `SplitMix64` (Steele, Lea and Flood 2014; Vigna's `splitmix64.c`).
#[derive(Debug, Clone)]
pub(crate) struct SplitMix64(u64);

impl SplitMix64 {
    pub(crate) const fn new(seed: u64) -> Self {
        Self(seed)
    }

    pub(crate) const fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// Uniform in `0..n`, `n > 0`: Lemire's multiply-and-reject
    /// ("Fast random integer generation in an interval", 2019), which
    /// has no bias for any `n`.
    pub(crate) fn below(&mut self, n: u64) -> u64 {
        debug_assert!(n > 0);
        let mut m = u128::from(self.next_u64()) * u128::from(n);
        // The low half of the product; truncation is the point.
        #[allow(clippy::cast_possible_truncation)]
        let mut low = m as u64;
        if low < n {
            let floor = n.wrapping_neg() % n;
            while low < floor {
                m = u128::from(self.next_u64()) * u128::from(n);
                #[allow(clippy::cast_possible_truncation)]
                {
                    low = m as u64;
                }
            }
        }
        #[allow(clippy::cast_possible_truncation)]
        let high = (m >> 64) as u64;
        high
    }
}

/// The 95% intervals of each column's mean, every column resampled with
/// the same draws: `columns[c][q]` is query `q`'s value of column `c`,
/// and `strata[q]` the stratum query `q` was drawn from, each stratum
/// resampled to its own size (all zeros for a suite drawn whole).
///
/// A paired comparison is one column of per-query differences.
///
/// # Panics
///
/// When the columns and the strata disagree on how many queries there
/// are, or there are none.
#[must_use]
pub fn intervals(columns: &[Vec<f64>], strata: &[u32]) -> Vec<Interval> {
    intervals_with(columns, strata, SEED, RESAMPLES)
}

/// [`intervals`], with the seed and resample count given; for tests.
///
/// # Panics
///
/// As [`intervals`], and when `resamples` is zero.
#[must_use]
pub fn intervals_with(
    columns: &[Vec<f64>],
    strata: &[u32],
    seed: u64,
    resamples: usize,
) -> Vec<Interval> {
    let n = strata.len();
    assert!(n > 0, "an interval over no queries");
    assert!(resamples > 0, "an interval from no resamples");
    for column in columns {
        assert_eq!(column.len(), n, "a column and the strata disagree");
    }
    // Each stratum's members, strata in label order, members in query
    // order: the draw order is the file's, never a hash map's.
    let mut labels: Vec<u32> = strata.to_vec();
    labels.sort_unstable();
    labels.dedup();
    let members: Vec<Vec<usize>> = labels
        .iter()
        .map(|label| (0..n).filter(|&q| strata[q] == *label).collect())
        .collect();

    let mut rng = SplitMix64::new(seed);
    let mut means: Vec<Vec<f64>> = vec![Vec::with_capacity(resamples); columns.len()];
    let mut sums = vec![0.0_f64; columns.len()];
    #[allow(clippy::cast_precision_loss)]
    let count = n as f64;
    for _ in 0..resamples {
        sums.fill(0.0);
        for group in &members {
            let size = group.len() as u64;
            for _ in 0..group.len() {
                // An index into `group`, so below its length.
                #[allow(clippy::cast_possible_truncation)]
                let q = group[rng.below(size) as usize];
                for (sum, column) in sums.iter_mut().zip(columns) {
                    *sum += column[q];
                }
            }
        }
        for (mean, sum) in means.iter_mut().zip(&sums) {
            mean.push(sum / count);
        }
    }
    let (low, high) = positions(resamples);
    means
        .into_iter()
        .map(|mut m| {
            m.sort_unstable_by(f64::total_cmp);
            Interval {
                low: m[low],
                high: m[high],
            }
        })
        .collect()
}

/// The positions of the 2.5th and 97.5th percentiles among `resamples`
/// sorted means: 250 and 9,749 of 10,000.
const fn positions(resamples: usize) -> (usize, usize) {
    let low = resamples * 25 / 1000;
    (low, resamples - 1 - low)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn splitmix_is_the_published_sequence() {
        // Vigna's splitmix64.c from a state of 0.
        let mut rng = SplitMix64::new(0);
        assert_eq!(rng.next_u64(), 0xe220_a839_7b1d_cdaf);
        assert_eq!(rng.next_u64(), 0x6e78_9e6a_a1b9_65f4);
        assert_eq!(rng.next_u64(), 0x06c4_5d18_8009_454f);
    }

    #[test]
    fn positions_are_the_percentiles() {
        assert_eq!(positions(10_000), (250, 9_749));
        assert_eq!(positions(1_000), (25, 974));
    }

    #[test]
    fn a_constant_column_has_no_width() {
        let got = intervals_with(&[vec![0.25; 7]], &[0; 7], SEED, 200);
        assert_eq!(
            got,
            vec![Interval {
                low: 0.25,
                high: 0.25
            }]
        );
    }

    #[test]
    fn the_same_inputs_give_the_same_interval() {
        let column: Vec<f64> = (0..50).map(|i| f64::from(i % 7) / 7.0).collect();
        let a = intervals(std::slice::from_ref(&column), &[0; 50]);
        let b = intervals(&[column], &[0; 50]);
        assert_eq!(a, b);
    }

    #[test]
    fn a_stratum_of_one_value_stays_that_value() {
        // Two strata, one all 0 and one all 1, half the queries each:
        // every resample keeps the halves, so the mean is always 0.5.
        let column = [vec![0.0; 10], vec![1.0; 10]].concat();
        let strata = [vec![0; 10], vec![1; 10]].concat();
        let got = intervals_with(&[column], &strata, SEED, 500);
        assert_eq!(
            got,
            vec![Interval {
                low: 0.5,
                high: 0.5
            }]
        );
    }

    proptest! {
        /// Draws stay inside the range for any bound.
        #[test]
        fn below_stays_below(seed: u64, n in 1_u64..) {
            let mut rng = SplitMix64::new(seed);
            for _ in 0..16 {
                prop_assert!(rng.below(n) < n);
            }
        }

        /// The interval lies within the values' range and is ordered.
        #[test]
        fn an_interval_lies_within_the_values(
            values in prop::collection::vec(0.0_f64..=1.0, 1..40),
            seed: u64,
        ) {
            let n = values.len();
            let lo = values.iter().copied().fold(f64::INFINITY, f64::min);
            let hi = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            let [got] = intervals_with(&[values], &vec![0; n], seed, 100)[..] else {
                unreachable!("one column in, one interval out");
            };
            prop_assert!(got.low <= got.high);
            prop_assert!(lo - 1e-12 <= got.low && got.high <= hi + 1e-12);
        }

        /// Columns drawn together see the same draws: a column and its
        /// double have intervals exactly double — exactly, since doubling
        /// is exact in binary, hence the float comparison.
        #[test]
        #[allow(clippy::float_cmp)]
        fn columns_share_their_draws(
            values in prop::collection::vec(0.0_f64..=1.0, 1..40),
            seed: u64,
        ) {
            let n = values.len();
            let doubled: Vec<f64> = values.iter().map(|v| v * 2.0).collect();
            let got = intervals_with(&[values, doubled], &vec![0; n], seed, 100);
            prop_assert_eq!(got[0].low * 2.0, got[1].low);
            prop_assert_eq!(got[0].high * 2.0, got[1].high);
        }
    }
}
