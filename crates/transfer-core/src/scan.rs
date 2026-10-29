use bolt_protocol::{path::sanitize_component, MAX_FILES_PER_OFFER};
use std::{
    collections::HashSet,
    fs, io,
    path::{Path, PathBuf},
    sync::Arc,
    time::UNIX_EPOCH,
};

/// Opens a readable handle to a file on demand. Lets callers supply data that
/// has no filesystem path (Android SAF `content://` documents are opened as
/// file descriptors by the platform layer).
pub trait FileOpener: Send + Sync + std::fmt::Debug {
    fn open(&self) -> io::Result<fs::File>;
}

#[derive(Debug, Clone)]
pub struct SendItem {
    /// Path on disk. Ignored when `opener` is set.
    pub abs_path: PathBuf,
    /// Platform-supplied way to open the data (e.g. a SAF document); wins over `abs_path`.
    pub opener: Option<Arc<dyn FileOpener>>,
    /// `/`-separated path the receiver will recreate (folders keep their structure).
    pub rel_path: String,
    pub size: u64,
    /// Identity of this file *version* (size + mtime); gates resume.
    pub file_id: String,
}

impl SendItem {
    /// An item backed by a platform opener rather than a path. `rel_path` must
    /// already use `/` separators; run the list through [`finalize_items`].
    pub fn from_opener(
        rel_path: String,
        size: u64,
        file_id: String,
        opener: Arc<dyn FileOpener>,
    ) -> Self {
        Self {
            abs_path: PathBuf::new(),
            opener: Some(opener),
            rel_path,
            size,
            file_id,
        }
    }

    pub(crate) fn open_blocking(&self) -> io::Result<fs::File> {
        match &self.opener {
            Some(o) => o.open(),
            None => fs::File::open(&self.abs_path),
        }
    }
}

/// Sanitise and de-duplicate the relative paths of externally built items.
pub fn finalize_items(mut items: Vec<SendItem>) -> io::Result<Vec<SendItem>> {
    if items.len() > MAX_FILES_PER_OFFER {
        return Err(io::Error::other(format!(
            "too many files (limit {MAX_FILES_PER_OFFER})"
        )));
    }
    for it in &mut items {
        it.rel_path = it
            .rel_path
            .split('/')
            .filter(|c| !c.is_empty())
            .map(sanitize_component)
            .collect::<Vec<_>>()
            .join("/");
        if it.rel_path.is_empty() {
            it.rel_path = "_".into();
        }
    }
    dedupe(&mut items);
    Ok(items)
}

fn file_id(meta: &fs::Metadata) -> String {
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{}-{}", meta.len(), mtime)
}

/// Expand files and folders (recursively) into a flat, validated send list.
///
/// Symlinks are skipped rather than followed, so a link can never pull in
/// content from outside the selected folder.
pub fn scan(paths: &[PathBuf]) -> io::Result<Vec<SendItem>> {
    let mut out = Vec::new();
    for p in paths {
        let meta = fs::symlink_metadata(p)?;
        let name = p
            .file_name()
            .map(|n| sanitize_component(&n.to_string_lossy()))
            .unwrap_or_else(|| "_".to_string());
        if meta.is_file() {
            out.push(SendItem {
                abs_path: p.clone(),
                opener: None,
                rel_path: name,
                size: meta.len(),
                file_id: file_id(&meta),
            });
        } else if meta.is_dir() {
            walk(p, &name, &mut out)?;
        }
        if out.len() > MAX_FILES_PER_OFFER {
            return Err(io::Error::other(format!(
                "too many files (limit {MAX_FILES_PER_OFFER})"
            )));
        }
    }
    dedupe(&mut out);
    Ok(out)
}

fn walk(dir: &Path, rel: &str, out: &mut Vec<SendItem>) -> io::Result<()> {
    let mut entries: Vec<_> = fs::read_dir(dir)?.collect::<Result<_, _>>()?;
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let meta = fs::symlink_metadata(e.path())?;
        let child = format!(
            "{rel}/{}",
            sanitize_component(&e.file_name().to_string_lossy())
        );
        if meta.is_file() {
            out.push(SendItem {
                abs_path: e.path(),
                opener: None,
                rel_path: child,
                size: meta.len(),
                file_id: file_id(&meta),
            });
        } else if meta.is_dir() {
            walk(&e.path(), &child, out)?;
        }
        if out.len() > MAX_FILES_PER_OFFER {
            return Err(io::Error::other(format!(
                "too many files (limit {MAX_FILES_PER_OFFER})"
            )));
        }
    }
    Ok(())
}

/// Make rel paths unique case-insensitively ("a.txt", "a (1).txt", ...).
fn dedupe(items: &mut [SendItem]) {
    let mut seen = HashSet::new();
    for it in items.iter_mut() {
        let mut candidate = it.rel_path.clone();
        let mut n = 1;
        while !seen.insert(candidate.to_lowercase()) {
            let (dir, file) = it
                .rel_path
                .rsplit_once('/')
                .map_or(("", it.rel_path.as_str()), |(d, f)| (d, f));
            let (stem, ext) = match file.rfind('.') {
                Some(i) if i > 0 => (&file[..i], &file[i..]),
                _ => (file, ""),
            };
            let renamed = format!("{stem} ({n}){ext}");
            candidate = if dir.is_empty() {
                renamed
            } else {
                format!("{dir}/{renamed}")
            };
            n += 1;
        }
        it.rel_path = candidate;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folders_keep_structure_and_duplicates_get_unique_names() {
        let d = tempfile::tempdir().unwrap();
        fs::create_dir_all(d.path().join("album/sub")).unwrap();
        fs::write(d.path().join("album/a.txt"), b"1").unwrap();
        fs::write(d.path().join("album/sub/b.txt"), b"22").unwrap();
        fs::write(d.path().join("a.txt"), b"x").unwrap();
        let other = tempfile::tempdir().unwrap();
        fs::write(other.path().join("a.txt"), b"y").unwrap();

        let items = scan(&[
            d.path().join("album"),
            d.path().join("a.txt"),
            other.path().join("a.txt"),
        ])
        .unwrap();
        let rels: Vec<_> = items.iter().map(|i| i.rel_path.as_str()).collect();
        assert_eq!(
            rels,
            ["album/a.txt", "album/sub/b.txt", "a.txt", "a (1).txt"]
        );
    }

    #[derive(Debug)]
    struct PathOpener(PathBuf);
    impl FileOpener for PathOpener {
        fn open(&self) -> io::Result<fs::File> {
            fs::File::open(&self.0)
        }
    }

    #[test]
    fn opener_items_are_sanitised_and_deduped() {
        let d = tempfile::tempdir().unwrap();
        fs::write(d.path().join("f"), b"abc").unwrap();
        let op: Arc<dyn FileOpener> = Arc::new(PathOpener(d.path().join("f")));
        let mk = |p: &str| SendItem::from_opener(p.into(), 3, "x".into(), op.clone());
        let items = finalize_items(vec![mk("a/b.txt"), mk("a/b.txt"), mk("../x")]).unwrap();
        let rels: Vec<_> = items.iter().map(|i| i.rel_path.as_str()).collect();
        assert_eq!(rels[0], "a/b.txt");
        assert_eq!(rels[1], "a/b (1).txt");
        assert!(!rels[2].contains(".."));
        let mut buf = String::new();
        std::io::Read::read_to_string(&mut items[0].open_blocking().unwrap(), &mut buf).unwrap();
        assert_eq!(buf, "abc");
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_are_not_followed() {
        let d = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("secret"), b"s").unwrap();
        fs::create_dir(d.path().join("f")).unwrap();
        std::os::unix::fs::symlink(outside.path(), d.path().join("f/link")).unwrap();
        assert!(scan(&[d.path().join("f")]).unwrap().is_empty());
    }
}
