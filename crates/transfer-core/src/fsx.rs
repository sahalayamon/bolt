//! Receiver-side file staging: partial files, resume manifests and safe
//! finalisation into the download directory.
//!
//! Layout under the download root:
//! ```text
//! .bolt-partial/<key>.part   data received so far
//! .bolt-partial/<key>.json   manifest binding the part file to sender + source file
//! .bolt-partial/<key>.done   marker: file already finalised (for batch resume)
//! ```
//! `<key>` = BLAKE3(sender id, relative path, size, file_id), so a partial
//! file can only ever be resumed by the same authenticated sender offering
//! the same source file version.

#[cfg(test)]
use bolt_protocol::ItemKind;
use bolt_protocol::{path::validate_rel_path, FileOffer};
use bolt_security::DeviceId;
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

const PARTIAL_DIR: &str = ".bolt-partial";

#[derive(Debug, Clone)]
pub struct Staging {
    root: PathBuf,
}

#[derive(Serialize, Deserialize, PartialEq, Eq)]
struct Manifest {
    sender: String,
    path: String,
    size: u64,
    file_id: String,
}

#[derive(Serialize, Deserialize)]
struct Done {
    final_path: String,
    size: u64,
}

/// Read-only classification of an offered file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Probe {
    Complete,
    Resume { offset: u64 },
    Fresh,
}

/// Outcome of opening one incoming file.
pub enum Prepared {
    /// Already received and finalised in an earlier attempt.
    AlreadyComplete,
    /// Open partial file positioned for appending at `offset`.
    Partial {
        file: File,
        offset: u64,
        key: String,
    },
}

impl Staging {
    pub fn new(root: &Path) -> io::Result<Self> {
        fs::create_dir_all(root)?;
        Ok(Self {
            root: root.canonicalize()?,
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn pdir(&self) -> PathBuf {
        self.root.join(PARTIAL_DIR)
    }

    pub fn key(sender: &DeviceId, f: &FileOffer) -> String {
        let mut h = blake3::Hasher::new_derive_key("bolt partial key v1");
        h.update(sender.as_str().as_bytes());
        h.update(&[0]);
        h.update(f.path.as_bytes());
        h.update(&[0]);
        h.update(&f.size.to_le_bytes());
        h.update(f.file_id.as_bytes());
        h.finalize().to_hex()[..32].to_string()
    }

    fn part_path(&self, key: &str) -> PathBuf {
        self.pdir().join(format!("{key}.part"))
    }
    fn manifest_path(&self, key: &str) -> PathBuf {
        self.pdir().join(format!("{key}.json"))
    }
    fn done_path(&self, key: &str) -> PathBuf {
        self.pdir().join(format!("{key}.done"))
    }

    /// Read-only: what could be skipped or resumed for `f`? Never trusts a
    /// partial whose manifest differs from the current offer and sender.
    pub fn probe(&self, sender: &DeviceId, f: &FileOffer) -> io::Result<Probe> {
        let key = Self::key(sender, f);

        if let Ok(raw) = fs::read(self.done_path(&key)) {
            if let Ok(done) = serde_json::from_slice::<Done>(&raw) {
                let p = self.root.join(&done.final_path);
                let intact = fs::symlink_metadata(&p)
                    .map(|m| m.is_file() && m.len() == done.size)
                    .unwrap_or(false);
                if intact && done.size == f.size {
                    return Ok(Probe::Complete);
                }
            }
        }

        let want = Manifest {
            sender: sender.to_string(),
            path: f.path.clone(),
            size: f.size,
            file_id: f.file_id.clone(),
        };
        let manifest_ok = fs::read(self.manifest_path(&key))
            .ok()
            .and_then(|b| serde_json::from_slice::<Manifest>(&b).ok())
            .is_some_and(|m| m == want);
        let len = if manifest_ok {
            fs::metadata(self.part_path(&key)).map(|m| m.len()).ok()
        } else {
            None
        };
        Ok(match len {
            // Never report a "full" partial as resumable: re-send the last byte so
            // completion is always established by a verified FILE_COMPLETE.
            Some(len) if len <= f.size && len > 0 => Probe::Resume {
                offset: len.min(f.size.saturating_sub(1)),
            },
            _ => Probe::Fresh,
        })
    }

    /// Open the destination partial file for writing, per [`Staging::probe`].
    pub fn open(&self, sender: &DeviceId, f: &FileOffer) -> io::Result<Prepared> {
        fs::create_dir_all(self.pdir())?;
        let key = Self::key(sender, f);
        match self.probe(sender, f)? {
            Probe::Complete => Ok(Prepared::AlreadyComplete),
            Probe::Resume { offset } if offset > 0 => {
                let mut file = OpenOptions::new().write(true).open(self.part_path(&key))?;
                file.set_len(offset)?;
                file.seek(SeekFrom::Start(offset))?;
                Ok(Prepared::Partial { file, offset, key })
            }
            _ => {
                let m = Manifest {
                    sender: sender.to_string(),
                    path: f.path.clone(),
                    size: f.size,
                    file_id: f.file_id.clone(),
                };
                fs::write(
                    self.manifest_path(&key),
                    serde_json::to_vec(&m).map_err(io::Error::other)?,
                )?;
                let file = OpenOptions::new()
                    .write(true)
                    .create(true)
                    .truncate(true)
                    .open(self.part_path(&key))?;
                Ok(Prepared::Partial {
                    file,
                    offset: 0,
                    key,
                })
            }
        }
    }

    /// Hash the first `len` bytes of the partial file (to continue a resumed hash).
    pub fn hash_prefix(&self, key: &str, len: u64, hasher: &mut blake3::Hasher) -> io::Result<()> {
        let mut f = File::open(self.part_path(key))?;
        let mut buf = vec![0u8; 256 * 1024];
        let mut left = len;
        while left > 0 {
            let want = buf.len().min(left as usize);
            f.read_exact(&mut buf[..want])?;
            hasher.update(&buf[..want]);
            left -= want as u64;
        }
        Ok(())
    }

    /// Delete a partial file and its manifest (e.g. after an integrity failure).
    pub fn discard(&self, key: &str) {
        let _ = fs::remove_file(self.part_path(key));
        let _ = fs::remove_file(self.manifest_path(key));
    }

    /// Move the verified partial into place without ever overwriting an existing
    /// file, and without following symlinks out of the download root.
    pub fn finalize(&self, key: &str, rel_path: &str, size: u64) -> io::Result<PathBuf> {
        validate_rel_path(rel_path)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e.to_string()))?;
        let mut dir = self.root.clone();
        let mut comps: Vec<&str> = rel_path.split('/').collect();
        let file_name = comps.pop().expect("validated non-empty");

        for c in comps {
            dir.push(c);
            match fs::symlink_metadata(&dir) {
                Ok(m) if m.file_type().is_symlink() || !m.is_dir() => {
                    return Err(io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        "destination component is not a plain directory",
                    ));
                }
                Ok(_) => {}
                Err(e) if e.kind() == io::ErrorKind::NotFound => fs::create_dir(&dir)?,
                Err(e) => return Err(e),
            }
        }
        if !dir.canonicalize()?.starts_with(&self.root) {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "destination escapes download directory",
            ));
        }

        // Reserve a unique name atomically (create_new), then rename the part over our own placeholder.
        let (stem, ext) = match file_name.rfind('.') {
            Some(i) if i > 0 => (&file_name[..i], &file_name[i..]),
            _ => (file_name, ""),
        };
        let mut n = 0u32;
        let final_path = loop {
            let name = if n == 0 {
                file_name.to_string()
            } else {
                format!("{stem} ({n}){ext}")
            };
            let candidate = dir.join(&name);
            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&candidate)
            {
                Ok(_) => break candidate,
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => n += 1,
                Err(e) => return Err(e),
            }
        };
        if let Err(e) = fs::rename(self.part_path(key), &final_path) {
            let _ = fs::remove_file(&final_path);
            return Err(e);
        }
        let _ = fs::remove_file(self.manifest_path(key));
        let rel_final = final_path
            .strip_prefix(&self.root)
            .unwrap_or(&final_path)
            .to_string_lossy()
            .replace('\\', "/");
        let done = Done {
            final_path: rel_final,
            size,
        };
        fs::write(
            self.done_path(key),
            serde_json::to_vec(&done).map_err(io::Error::other)?,
        )?;
        Ok(final_path)
    }

    /// Remove the `.done` markers for a fully completed batch.
    pub fn clear_done(&self, keys: &[String]) {
        for k in keys {
            let _ = fs::remove_file(self.done_path(k));
        }
    }

    /// Delete partial state older than `max_age`. Call at startup.
    pub fn purge_stale(&self, max_age: Duration) -> usize {
        let Ok(rd) = fs::read_dir(self.pdir()) else {
            return 0;
        };
        let mut n = 0;
        for e in rd.flatten() {
            let old = e
                .metadata()
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| SystemTime::now().duration_since(t).ok())
                .is_some_and(|age| age > max_age);
            if old && fs::remove_file(e.path()).is_ok() {
                n += 1;
            }
        }
        n
    }

    /// Free space on the volume holding the download directory.
    pub fn available_space(&self) -> io::Result<u64> {
        fs4::available_space(&self.root)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn offer(path: &str, size: u64) -> FileOffer {
        FileOffer {
            index: 0,
            path: path.into(),
            size,
            file_id: "v1".into(),
            kind: ItemKind::File,
        }
    }
    fn sender() -> DeviceId {
        DeviceId::parse(&"a".repeat(64)).unwrap()
    }

    #[test]
    fn finalize_never_overwrites() {
        let d = tempfile::tempdir().unwrap();
        let st = Staging::new(d.path()).unwrap();
        for i in 0..3u8 {
            let f = FileOffer {
                file_id: format!("v{i}"),
                ..offer("dir/photo.jpg", 3)
            };
            let Prepared::Partial { mut file, key, .. } = st.open(&sender(), &f).unwrap() else {
                panic!()
            };
            file.write_all(&[i; 3]).unwrap();
            drop(file);
            st.finalize(&key, "dir/photo.jpg", 3).unwrap();
        }
        let root = st.root();
        assert_eq!(fs::read(root.join("dir/photo.jpg")).unwrap(), [0; 3]);
        assert_eq!(fs::read(root.join("dir/photo (1).jpg")).unwrap(), [1; 3]);
        assert_eq!(fs::read(root.join("dir/photo (2).jpg")).unwrap(), [2; 3]);
    }

    #[test]
    fn resume_requires_matching_manifest() {
        let d = tempfile::tempdir().unwrap();
        let st = Staging::new(d.path()).unwrap();
        let f = offer("a.bin", 10);
        let Prepared::Partial { mut file, .. } = st.open(&sender(), &f).unwrap() else {
            panic!()
        };
        file.write_all(b"12345").unwrap();
        drop(file);

        // same sender + same source version -> resumes at 5
        let Prepared::Partial { offset, .. } = st.open(&sender(), &f).unwrap() else {
            panic!()
        };
        assert_eq!(offset, 5);
        // different source version -> fresh
        let changed = FileOffer {
            file_id: "v2".into(),
            ..f.clone()
        };
        let Prepared::Partial { offset, .. } = st.open(&sender(), &changed).unwrap() else {
            panic!()
        };
        assert_eq!(offset, 0);
        // different sender -> fresh
        let other = DeviceId::parse(&"b".repeat(64)).unwrap();
        let Prepared::Partial { offset, .. } = st.open(&other, &f).unwrap() else {
            panic!()
        };
        assert_eq!(offset, 0);
    }

    #[test]
    fn completed_files_are_skipped_on_retry() {
        let d = tempfile::tempdir().unwrap();
        let st = Staging::new(d.path()).unwrap();
        let f = offer("x.txt", 2);
        let Prepared::Partial { mut file, key, .. } = st.open(&sender(), &f).unwrap() else {
            panic!()
        };
        file.write_all(b"hi").unwrap();
        drop(file);
        st.finalize(&key, "x.txt", 2).unwrap();
        assert!(matches!(
            st.open(&sender(), &f).unwrap(),
            Prepared::AlreadyComplete
        ));
    }

    #[test]
    fn traversal_is_refused_even_if_validation_were_bypassed() {
        let d = tempfile::tempdir().unwrap();
        let st = Staging::new(d.path()).unwrap();
        assert!(st.finalize("k", "../evil", 0).is_err());
        assert!(st.finalize("k", "a/../../evil", 0).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_directory_cannot_redirect_writes() {
        let d = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(outside.path(), d.path().join("link")).unwrap();
        let st = Staging::new(d.path()).unwrap();
        let f = offer("link/pwn.txt", 1);
        let Prepared::Partial { mut file, key, .. } = st.open(&sender(), &f).unwrap() else {
            panic!()
        };
        file.write_all(b"x").unwrap();
        drop(file);
        assert!(st.finalize(&key, "link/pwn.txt", 1).is_err());
        assert!(!outside.path().join("pwn.txt").exists());
    }

    #[test]
    fn purge_removes_only_stale() {
        let d = tempfile::tempdir().unwrap();
        let st = Staging::new(d.path()).unwrap();
        let _ = st.open(&sender(), &offer("a", 1)).unwrap();
        assert_eq!(st.purge_stale(Duration::from_secs(3600)), 0);
        assert!(st.purge_stale(Duration::ZERO) >= 1);
    }
}
