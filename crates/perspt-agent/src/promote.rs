//! Workspace promotion with platform-specific hardening.
//!
//! Unix checks and acts through a held directory descriptor, walking relative
//! paths with `O_NOFOLLOW | O_DIRECTORY`; an ancestor swap cannot redirect a
//! write. Native Windows supports the explicitly reduced-isolation release
//! path with reparse-point checks and write-through replacement, but does not
//! claim the same descriptor-relative race guarantee (Gate L).

use anyhow::{bail, Context, Result};
#[cfg(not(windows))]
use rustix::fs::{fsync, mkdirat, openat, renameat, unlinkat, AtFlags, Mode, OFlags};
#[cfg(windows)]
use std::ffi::OsString;
#[cfg(not(windows))]
use std::fs::File;
#[cfg(windows)]
use std::fs::OpenOptions;
#[cfg(windows)]
use std::io::{ErrorKind, Write};
#[cfg(not(windows))]
use std::io::{Read, Write};
#[cfg(not(windows))]
use std::os::fd::OwnedFd;
#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;
#[cfg(windows)]
use std::os::windows::fs::MetadataExt;
#[cfg(not(windows))]
use std::path::{Component, Path};
#[cfg(windows)]
use std::path::{Component, Path, PathBuf};

/// A workspace root held open as a directory descriptor.
#[cfg(not(windows))]
pub struct WorkspaceRoot {
    fd: OwnedFd,
}

/// The parent directory of a promotion target, held open, plus the final
/// file name. All reads and writes for the target go through this pair.
#[derive(Debug)]
#[cfg(not(windows))]
pub struct TargetDir {
    fd: OwnedFd,
    name: String,
}

#[cfg(not(windows))]
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

#[cfg(not(windows))]
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

#[cfg(all(test, not(windows)))]
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

/// Native Windows promotion used by the explicit reduced-isolation mode.
///
/// Windows paths are checked for reparse points component by component and
/// replacement uses the native write-through APIs. Unlike the descriptor-held
/// Unix implementation, these checks cannot close every ancestor swap race;
/// callers must not describe this mode as governed OS isolation.
#[cfg(windows)]
pub struct WorkspaceRoot {
    root: PathBuf,
}

#[cfg(windows)]
#[derive(Debug)]
pub struct TargetDir {
    parent: PathBuf,
    name: OsString,
}

#[cfg(windows)]
impl WorkspaceRoot {
    pub fn open(root: &Path) -> Result<Self> {
        let root = root
            .canonicalize()
            .with_context(|| format!("opening workspace root {}", root.display()))?;
        anyhow::ensure!(root.is_dir(), "workspace root is not a directory");
        Ok(Self { root })
    }

    pub fn target_dir(&self, relative: &str, create: bool) -> Result<TargetDir> {
        let mut components = Vec::new();
        for component in Path::new(relative).components() {
            match component {
                Component::Normal(part) => components.push(part.to_os_string()),
                Component::CurDir => {}
                _ => bail!("promotion path escapes the workspace: {relative}"),
            }
        }
        let name = components
            .pop()
            .context("promotion path has no file name")?;
        let mut parent = self.root.clone();
        for part in components {
            parent.push(part);
            match std::fs::symlink_metadata(&parent) {
                Ok(metadata) => {
                    ensure_plain(&parent, &metadata)?;
                    anyhow::ensure!(
                        metadata.is_dir(),
                        "promotion ancestor is not a directory: {}",
                        parent.display()
                    );
                }
                Err(error) if error.kind() == ErrorKind::NotFound && create => {
                    std::fs::create_dir(&parent)
                        .with_context(|| format!("creating directory {}", parent.display()))?;
                    let metadata = std::fs::symlink_metadata(&parent)?;
                    ensure_plain(&parent, &metadata)?;
                }
                Err(error) => {
                    return Err(error)
                        .with_context(|| format!("descending into {}", parent.display()))
                }
            }
        }
        Ok(TargetDir { parent, name })
    }

    pub fn read_if_present(&self, relative: &str) -> Result<Option<Vec<u8>>> {
        match self.target_dir(relative, false) {
            Ok(target) => target.read_optional(),
            Err(error) if io_not_found(&error) => Ok(None),
            Err(error) => Err(error),
        }
    }
}

#[cfg(windows)]
impl TargetDir {
    fn path(&self) -> PathBuf {
        self.parent.join(&self.name)
    }

    pub fn read_optional(&self) -> Result<Option<Vec<u8>>> {
        let path = self.path();
        match std::fs::symlink_metadata(&path) {
            Ok(metadata) => ensure_plain(&path, &metadata)?,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error).with_context(|| format!("opening {}", path.display())),
        }
        std::fs::read(&path)
            .map(Some)
            .with_context(|| format!("reading {}", path.display()))
    }

    pub fn write(&self, bytes: &[u8]) -> Result<()> {
        let target = self.path();
        let existed = match std::fs::symlink_metadata(&target) {
            Ok(metadata) => {
                ensure_plain(&target, &metadata)?;
                true
            }
            Err(error) if error.kind() == ErrorKind::NotFound => false,
            Err(error) => {
                return Err(error).with_context(|| format!("opening {}", target.display()))
            }
        };
        let staged = self
            .parent
            .join(format!(".perspt-promote-{}", uuid::Uuid::new_v4()));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&staged)
            .with_context(|| format!("staging {}", target.display()))?;
        if let Err(error) = file.write_all(bytes).and_then(|()| file.sync_all()) {
            let _ = std::fs::remove_file(&staged);
            return Err(error).with_context(|| format!("writing staged {}", target.display()));
        }
        drop(file);
        if let Err(error) = replace_file(&staged, &target, existed) {
            let _ = std::fs::remove_file(&staged);
            return Err(error).with_context(|| format!("promoting {}", target.display()));
        }
        Ok(())
    }

    pub fn remove(&self) -> Result<()> {
        let target = self.path();
        match std::fs::symlink_metadata(&target) {
            Ok(metadata) => ensure_plain(&target, &metadata)?,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
            Err(error) => {
                return Err(error).with_context(|| format!("opening {}", target.display()))
            }
        }
        std::fs::remove_file(&target).with_context(|| format!("removing {}", target.display()))
    }

    pub fn apply(&self, bytes: Option<&[u8]>) -> Result<()> {
        match bytes {
            Some(content) => self.write(content),
            None => self.remove(),
        }
    }
}

#[cfg(windows)]
fn ensure_plain(path: &Path, metadata: &std::fs::Metadata) -> Result<()> {
    const REPARSE_POINT: u32 = 0x400;
    anyhow::ensure!(
        metadata.file_attributes() & REPARSE_POINT == 0,
        "promotion path contains a reparse point: {}",
        path.display()
    );
    Ok(())
}

#[cfg(windows)]
fn io_not_found(error: &anyhow::Error) -> bool {
    error
        .chain()
        .find_map(|cause| cause.downcast_ref::<std::io::Error>())
        .is_some_and(|cause| cause.kind() == ErrorKind::NotFound)
}

#[cfg(windows)]
fn replace_file(staged: &Path, target: &Path, existed: bool) -> std::io::Result<()> {
    use windows_sys::Win32::Storage::FileSystem::{
        MoveFileExW, ReplaceFileW, MOVEFILE_WRITE_THROUGH, REPLACEFILE_WRITE_THROUGH,
    };

    let staged = staged
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let target = target
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let success = unsafe {
        if existed {
            ReplaceFileW(
                target.as_ptr(),
                staged.as_ptr(),
                std::ptr::null(),
                REPLACEFILE_WRITE_THROUGH,
                std::ptr::null(),
                std::ptr::null(),
            )
        } else {
            MoveFileExW(staged.as_ptr(), target.as_ptr(), MOVEFILE_WRITE_THROUGH)
        }
    };
    if success == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(all(test, windows))]
mod windows_tests {
    use super::*;

    #[test]
    fn native_write_replace_read_and_remove() {
        let dir = tempfile::tempdir().unwrap();
        let root = WorkspaceRoot::open(dir.path()).unwrap();
        let target = root.target_dir("nested/file.txt", true).unwrap();
        target.write(b"first").unwrap();
        target.write(b"second").unwrap();
        assert_eq!(target.read_optional().unwrap().unwrap(), b"second");
        target.remove().unwrap();
        assert!(target.read_optional().unwrap().is_none());
    }

    #[test]
    fn native_parent_escapes_are_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let root = WorkspaceRoot::open(dir.path()).unwrap();
        assert!(root.target_dir("../outside.txt", false).is_err());
        assert!(root.target_dir(r"C:\outside.txt", false).is_err());
    }
}
