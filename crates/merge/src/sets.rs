//! The normalized sets a merge reads: found under the store's normalized
//! layer, opened with their headers checked.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use ingest::{Kind, Reader, Source};

/// Every `<prefix>.<kind>.zst` below `dir`, sorted by path so a run reads
/// the same files in the same order every time; none when `dir` does not
/// exist.
///
/// # Errors
///
/// I/O while walking.
pub fn find(dir: &Path, kind: Kind) -> io::Result<Vec<PathBuf>> {
    let mut found = Vec::new();
    let suffix = format!(".{}.zst", kind.segment());
    walk(dir, &suffix, &mut found)?;
    found.sort();
    Ok(found)
}

fn walk(dir: &Path, suffix: &str, found: &mut Vec<PathBuf>) -> io::Result<()> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e),
    };
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            walk(&path, suffix, found)?;
        } else if path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.ends_with(suffix))
        {
            found.push(path);
        }
    }
    Ok(())
}

/// Opens one set file and checks that `S` wrote it, under this binary's
/// model version, holding `kind`.
///
/// # Errors
///
/// I/O, a file that is not the layout, or a header that says otherwise.
pub fn open<S: Source>(path: &Path, kind: Kind) -> io::Result<Reader> {
    let reader = Reader::open(path)?;
    let header = reader.header();
    let bad = |what: String| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{}: {what}", path.display()),
        )
    };
    if header.source != S::NAME {
        return Err(bad(format!(
            "written by {:?}, not {}",
            header.source,
            S::NAME
        )));
    }
    if header.model != model::VERSION {
        return Err(bad(format!(
            "model version {} is not this binary's {}",
            header.model,
            model::VERSION
        )));
    }
    if header.kind != kind {
        return Err(bad(format!("a {} set, not {kind}", header.kind)));
    }
    Ok(reader)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use ingest::Kind;

    use super::find;

    #[test]
    fn sets_are_found_below_a_directory_in_path_order() {
        let dir = std::env::temp_dir().join(format!("ol-rht-merge-sets-{}", std::process::id()));
        fs::create_dir_all(dir.join("b/deep")).unwrap();
        fs::create_dir_all(dir.join("a")).unwrap();
        for name in [
            "b/deep/y.works.zst",
            "a/x.works.zst",
            "a/x.references.zst",
            "a/x.symbols.zst",
            "a/not-a-set.txt",
        ] {
            fs::write(dir.join(name), b"").unwrap();
        }
        let works = find(&dir, Kind::Works).unwrap();
        assert_eq!(
            works,
            [dir.join("a/x.works.zst"), dir.join("b/deep/y.works.zst")]
        );
        assert_eq!(
            find(&dir, Kind::References).unwrap(),
            [dir.join("a/x.references.zst")]
        );
        assert!(find(&dir.join("missing"), Kind::Works).unwrap().is_empty());
        fs::remove_dir_all(dir).unwrap();
    }
}
