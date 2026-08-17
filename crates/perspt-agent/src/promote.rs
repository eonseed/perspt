//! Descriptor-relative workspace promotion.
//!
//! Every check and every act in this module goes through a held directory
//! descriptor obtained by walking the relative path one component at a time
//! with `O_NOFOLLOW | O_DIRECTORY`. An ancestor directory swapped for a
//! symlink after validation cannot redirect a write, because the write is
//! issued against the descriptor, not the path name. This closes the
//! check-then-act race the string-path promotion had (Gate L).

use anyhow::{bail, Context, Result};
use rustix::fs::{fsync, mkdirat, openat, renameat, unlinkat, AtFlags, Mode, OFlags};
use std::fs::File;
use std::io::{Read, Write};
use std::os::fd::OwnedFd;
use std::path::{Component, Path};

/// A workspace root held open as a directory descriptor.
pub struct WorkspaceRoot {
    fd: OwnedFd,
}

/// The parent directory of a promotion target, held open, plus the final
/// file name. All reads and writes for the target go through this pair.
#[derive(Debug)]
pub struct TargetDir {
    fd: OwnedFd,
    name: String,
}

impl WorkspaceRoot {
    /// Open the workspace root. The root itself may be reached through a
    /// symlink (the user chose it); everything below it may not.
    pub fn open(root: &Path) -> Result<Self> {
        let fd = rustix::fs::open(root, OFlags::RDONLY | OFlags::DIRECTORY, Mode::empty())
            .with_context(|| format!("opening workspace root {}", root.display()))?;
        Ok(Self { fd })
    }

    /// Walk to the parent directory of `relative`, refusing symlinks at
    /// every component. With `create`, missing intermediate directories are
    /// created through the descriptor.
    pub fn target_dir(&self, relative: &str, create: bool) -> Result<TargetDir> {
        let mut components = Vec::new();
        for component in Path::new(relative).components() {
            match component {
                Component::Normal(part) => components.push(
                    part.to_str()
                        .context("promotion path is not valid UTF-8")?
                        .to_string(),
                ),
                Component::CurDir => {}
                _ => bail!("promotion path escapes the workspace: {relative}"),
            }
        }
        let name = components
            .pop()
            .context("promotion path has no file name")?;
        let mut current = self
            .fd
            .try_clone()
            .context("cloning workspace root descriptor")?;
        let dir_flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW;
        for part in &components {
            if create {
                match mkdirat(&current, part.as_str(), Mode::from_raw_mode(0o777)) {
                    Ok(()) | Err(rustix::io::Errno::EXIST) => {}
                    Err(error) => {
                        return Err(error).with_context(|| format!("creating directory {part}"))
                    }
                }
            }
            current = openat(&current, part.as_str(), dir_flags, Mode::empty())
                .with_context(|| format!("descending into {part} under {relative}"))?;
        }
        Ok(TargetDir { fd: current, name })
    }

    /// Read `relative` through descriptors; `None` when the file or any
    /// ancestor directory is absent. Symlinks anywhere still error.
    pub fn read_if_present(&self, relative: &str) -> Result<Option<Vec<u8>>> {
        match self.target_dir(relative, false) {
            Ok(target) => target.read_optional(),
            Err(error)
                if error.downcast_ref::<rustix::io::Errno>() == Some(&rustix::io::Errno::NOENT) =>
            {
                Ok(None)
            }
            Err(error) => Err(error),
        }
    }
}

impl TargetDir {
    /// Read the target through the descriptor; `None` when absent.
    pub fn read_optional(&self) -> Result<Option<Vec<u8>>> {
        let flags = OFlags::RDONLY | OFlags::NOFOLLOW;
        match openat(&self.fd, self.name.as_str(), flags, Mode::empty()) {
            Ok(fd) => {
                let mut bytes = Vec::new();
                File::from(fd)
                    .read_to_end(&mut bytes)
                    .with_context(|| format!("reading {}", self.name))?;
                Ok(Some(bytes))
            }
            Err(rustix::io::Errno::NOENT) => Ok(None),
            Err(rustix::io::Errno::LOOP) | Err(rustix::io::Errno::MLINK) => {
                bail!("promotion target is a symlink: {}", self.name)
            }
            Err(error) => Err(error).with_context(|| format!("opening {}", self.name)),
        }
    }

    /// Write the target atomically: staged sibling created `O_EXCL` through
    /// the descriptor, fsynced, renamed over the target, directory fsynced.
    pub fn write(&self, bytes: &[u8]) -> Result<()> {
        let staged = format!(".perspt-promote-{}", uuid::Uuid::new_v4());
        let flags = OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW;
        let staged_fd = openat(&self.fd, staged.as_str(), flags, Mode::from_raw_mode(0o666))
            .with_context(|| format!("staging {}", self.name))?;
        let mut file = File::from(staged_fd);
        let written = file
            .write_all(bytes)
            .and_then(|()| file.sync_all())
            .with_context(|| format!("writing staged {}", self.name));
        if let Err(error) = written {
            let _ = unlinkat(&self.fd, staged.as_str(), AtFlags::empty());
            return Err(error);
        }
        renameat(&self.fd, staged.as_str(), &self.fd, self.name.as_str())
            .with_context(|| format!("promoting {}", self.name))?;
        fsync(&self.fd).with_context(|| format!("syncing parent of {}", self.name))?;
        Ok(())
    }

    /// Remove the target through the descriptor; absent targets are fine.
    pub fn remove(&self) -> Result<()> {
        match unlinkat(&self.fd, self.name.as_str(), AtFlags::empty()) {
            Ok(()) | Err(rustix::io::Errno::NOENT) => Ok(()),
            Err(error) => Err(error).with_context(|| format!("removing {}", self.name)),
        }
    }

    /// Apply a recorded state: bytes present means write, absent means remove.
    pub fn apply(&self, bytes: Option<&[u8]>) -> Result<()> {
        match bytes {
            Some(content) => self.write(content),
            None => self.remove(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_and_read_through_descriptor() {
        let dir = tempfile::tempdir().unwrap();
        let root = WorkspaceRoot::open(dir.path()).unwrap();
        let target = root.target_dir("nested/dir/file.txt", true).unwrap();
        target.write(b"content").unwrap();
        assert_eq!(target.read_optional().unwrap().unwrap(), b"content");
        assert_eq!(
            std::fs::read(dir.path().join("nested/dir/file.txt")).unwrap(),
            b"content"
        );
        target.remove().unwrap();
        assert!(target.read_optional().unwrap().is_none());
    }

    #[test]
    fn symlink_ancestor_is_refused_at_open_time() {
        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(outside.path(), dir.path().join("dir")).unwrap();
        let root = WorkspaceRoot::open(dir.path()).unwrap();
        let error = root.target_dir("dir/file.txt", false).unwrap_err();
        assert!(error.to_string().contains("descending into dir"));
    }

    #[test]
    fn symlink_target_file_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("real.txt"), b"real").unwrap();
        std::os::unix::fs::symlink(dir.path().join("real.txt"), dir.path().join("link.txt"))
            .unwrap();
        let root = WorkspaceRoot::open(dir.path()).unwrap();
        let target = root.target_dir("link.txt", false).unwrap();
        assert!(target.read_optional().is_err());
    }

    #[test]
    fn parent_escapes_are_rejected_structurally() {
        let dir = tempfile::tempdir().unwrap();
        let root = WorkspaceRoot::open(dir.path()).unwrap();
        assert!(root.target_dir("../outside.txt", false).is_err());
        assert!(root.target_dir("/etc/passwd", false).is_err());
    }
}
