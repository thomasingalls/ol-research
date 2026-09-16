//! The invariants of the graph plan's §8, and the byte-for-byte checks
//! beside them: the permutation a bijection; offsets from zero,
//! non-decreasing, summing to the arc count; every list strictly sorted
//! by position, every neighbour a work, every label in the table; every
//! forward arc mirrored in the transpose with the same label; every file
//! the digest its manifest says; two builds the same bytes.

use std::path::Path;

use merge::WorkId;

use crate::base::{Base, Lists, u32_at};
use crate::error::Error;
use crate::layout::{self, FILES, Manifest};

/// What [`invariants`] verified.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Checked {
    #[allow(missing_docs)]
    pub nodes: u32,
    #[allow(missing_docs)]
    pub arcs: u64,
}

fn broken(what: String) -> Error {
    Error::Invariant { what }
}

/// Every invariant, over every arc.
///
/// # Errors
///
/// The first one that does not hold.
pub fn invariants(base: &Base) -> Result<Checked, Error> {
    permutation(base)?;
    offsets(&base.forward(), "forward", base.arcs())?;
    offsets(&base.transpose(), "transpose", base.arcs())?;
    lists(base, &base.forward(), "forward")?;
    lists(base, &base.transpose(), "transpose")?;
    mirrored(base)?;
    Ok(Checked {
        nodes: base.nodes(),
        arcs: base.arcs(),
    })
}

fn permutation(base: &Base) -> Result<(), Error> {
    let n = usize::try_from(base.nodes()).expect("a u32 fits");
    let mut seen = vec![false; n];
    for ix in 0..n {
        let pos = u32_at(base.perm(), ix);
        let p = usize::try_from(pos).expect("a u32 fits");
        if p >= n {
            return Err(broken(format!("perm[{ix}] is {pos}, past the base")));
        }
        if seen[p] {
            return Err(broken(format!("position {pos} given twice")));
        }
        seen[p] = true;
        let back = u32_at(base.inv(), p);
        if usize::try_from(back).expect("a u32 fits") != ix + 1 {
            return Err(broken(format!("inv[{pos}] is #{back}, not #{}", ix + 1)));
        }
    }
    Ok(())
}

fn offsets(lists: &Lists<'_>, name: &str, arcs: u64) -> Result<(), Error> {
    if lists.offset(0) != 0 {
        return Err(broken(format!("{name} offsets do not start at zero")));
    }
    let rows = u64::from(lists.rows());
    for i in 0..rows {
        if lists.offset(i + 1) < lists.offset(i) {
            return Err(broken(format!("{name} offsets decrease at position {i}")));
        }
    }
    if lists.offset(rows) != arcs {
        return Err(broken(format!(
            "{name} degrees sum to {}, not {arcs}",
            lists.offset(rows)
        )));
    }
    Ok(())
}

fn lists(base: &Base, lists: &Lists<'_>, name: &str) -> Result<(), Error> {
    let n = base.nodes();
    let table = base.asserters().len();
    for pos in 0..lists.rows() {
        let mut last: Option<u32> = None;
        for (w, label) in lists.raw(pos) {
            if w == 0 || w > n {
                return Err(broken(format!(
                    "{name} list at position {pos} names #{w}, not a work of the base"
                )));
            }
            let p = WorkId::new(w)
                .and_then(|id| base.position(id))
                .expect("checked above");
            if last.is_some_and(|l| p <= l) {
                return Err(broken(format!(
                    "{name} list at position {pos} is not strictly sorted by position"
                )));
            }
            last = Some(p);
            if usize::from(label) >= table {
                return Err(broken(format!(
                    "{name} list at position {pos} has label {label}, outside a table of {table}"
                )));
            }
        }
    }
    Ok(())
}

/// Every forward arc `u → v` with label `l` is a transpose arc `v ← u`
/// with label `l`; the counts being equal, the transpose holds nothing
/// else.
fn mirrored(base: &Base) -> Result<(), Error> {
    let forward = base.forward();
    let transpose = base.transpose();
    for pos in 0..forward.rows() {
        let u = base.work_at(pos).expect("a position of the base");
        for (w, label) in forward.raw(pos) {
            let v = WorkId::new(w).expect("checked by lists");
            let vpos = base.position(v).expect("checked by lists");
            let lo = transpose.offset(u64::from(vpos));
            let hi = transpose.offset(u64::from(vpos) + 1);
            match find(&transpose, base, lo, hi, pos) {
                Some(l) if l == label => {}
                Some(l) => {
                    return Err(broken(format!(
                        "{u} -> {v} has label {label} forward and {l} in the transpose"
                    )));
                }
                None => return Err(broken(format!("{u} -> {v} is missing from the transpose"))),
            }
        }
    }
    Ok(())
}

/// Binary search of one list, sorted by position, for the neighbour at
/// position `want`; its label when found.
fn find(lists: &Lists<'_>, base: &Base, mut lo: u64, mut hi: u64, want: u32) -> Option<u8> {
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        let (w, label) = lists.arc_at(mid);
        let p = base.position(WorkId::new(w)?)?;
        if p == want {
            return Some(label);
        }
        if p < want {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    None
}

/// Recomputes every file's digest against the manifest.
///
/// # Errors
///
/// I/O, or a file that is not what the manifest says.
pub fn digests(base: &Base) -> Result<usize, Error> {
    for name in FILES {
        let listed = &base.manifest().files[name];
        let actual = layout::digest(&base.dir().join(name))?;
        if actual != *listed {
            return Err(Error::Mismatch {
                what: format!(
                    "{}: {name} is {} bytes {}, the manifest says {} bytes {}",
                    base.dir().display(),
                    actual.bytes,
                    actual.sha256,
                    listed.bytes,
                    listed.sha256
                ),
            });
        }
    }
    Ok(FILES.len())
}

/// The files of two builds that differ byte for byte, the manifests
/// compared without their timestamps.
///
/// # Errors
///
/// I/O, or a manifest that does not parse.
pub fn differing(a: &Path, b: &Path) -> Result<Vec<String>, Error> {
    let mut differ = Vec::new();
    for name in FILES {
        if layout::digest(&a.join(name))? != layout::digest(&b.join(name))? {
            differ.push(name.to_owned());
        }
    }
    let (mut ma, mut mb) = (Manifest::read(a)?, Manifest::read(b)?);
    ma.built_at.clear();
    mb.built_at.clear();
    if ma != mb {
        differ.push(layout::MANIFEST.to_owned());
    }
    Ok(differ)
}

#[cfg(test)]
mod tests {
    use super::{differing, invariants};
    use crate::base::Base;
    use crate::build::tests::{sample, scratch};
    use crate::build::{Options, build};
    use crate::layout::{self, FORWARD_LABELS, Input, TRANSPOSE_NEIGHBOURS};

    #[test]
    fn corruption_is_found() {
        let (nodes, edges) = sample();
        let root = scratch("check");
        let dir = root.join(layout::name(false));
        let input = || Input {
            merged: "v1.1".into(),
            shards: 1,
        };
        build(&dir, &nodes, input(), Options::default(), edges.clone()).unwrap();
        // A label flipped in the forward arrays: the mirror check sees it.
        let path = dir.join(FORWARD_LABELS);
        let mut labels = std::fs::read(&path).unwrap();
        labels[0] ^= 1;
        std::fs::write(&path, &labels).unwrap();
        let base = Base::open(&dir).unwrap();
        let err = invariants(&base).unwrap_err().to_string();
        assert!(err.contains("label"), "{err}");
        assert!(super::digests(&base).is_err());
        drop(base);
        // A transpose list reordered: the sortedness check sees it.
        build(&dir, &nodes, input(), Options::default(), edges).unwrap();
        let path = dir.join(TRANSPOSE_NEIGHBOURS);
        let mut bytes = std::fs::read(&path).unwrap();
        // Citers of #4 are #1 then #2 at some contiguous pair; swap the
        // first two arcs of the file whatever they are, then expect either
        // sortedness or the mirror to fail.
        bytes.swap(0, 4);
        bytes.swap(1, 5);
        bytes.swap(2, 6);
        bytes.swap(3, 7);
        std::fs::write(&path, &bytes).unwrap();
        let base = Base::open(&dir).unwrap();
        assert!(invariants(&base).is_err());
        drop(base);
        let other = root.join(format!("{}.other", layout::name(false)));
        build(&other, &nodes, input(), Options::default(), sample().1).unwrap();
        assert_eq!(
            differing(&dir, &other).unwrap(),
            vec![TRANSPOSE_NEIGHBOURS.to_owned()]
        );
        std::fs::remove_dir_all(&root).unwrap();
    }
}
