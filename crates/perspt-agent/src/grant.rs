//! Persistent grant signing-key resolution.

use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::path::PathBuf;

use anyhow::{Context, Result};
use rand_core::{OsRng, RngCore};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrantKeySource {
    OperatorEnvironment,
    OsCredentialStore,
    InstallationFile,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrantSigningKey {
    pub bytes: [u8; 32],
    pub source: GrantKeySource,
}

impl GrantSigningKey {
    pub fn resolve() -> Result<Self> {
        if let Ok(value) = std::env::var("PERSPT_GRANT_SIGNING_KEY") {
            return Ok(Self {
                bytes: decode_key(value.trim())?,
                source: GrantKeySource::OperatorEnvironment,
            });
        }
        if let Some(bytes) = credential_store_key() {
            return Ok(Self {
                bytes,
                source: GrantKeySource::OsCredentialStore,
            });
        }
        let path = key_file_path()?;
        if path.is_file() {
            let mut value = String::new();
            std::fs::File::open(&path)?.read_to_string(&mut value)?;
            return Ok(Self {
                bytes: decode_key(value.trim())?,
                source: GrantKeySource::InstallationFile,
            });
        }
        let parent = path.parent().context("grant key path has no parent")?;
        std::fs::create_dir_all(parent)?;
        let mut bytes = [0u8; 32];
        OsRng.fill_bytes(&mut bytes);
        write_private_key(&path, &encode_key(&bytes))?;
        log::warn!(
            "OS credential store unavailable; persistent grant key stored in mode-0600 file {}",
            path.display()
        );
        Ok(Self {
            bytes,
            source: GrantKeySource::InstallationFile,
        })
    }
}

fn key_file_path() -> Result<PathBuf> {
    perspt_core::paths::config_dir()
        .map(|directory| directory.join("grant-signing.key"))
        .context("platform configuration directory is unavailable")
}

#[cfg(unix)]
fn write_private_key(path: &std::path::Path, value: &str) -> Result<()> {
    use std::os::unix::fs::OpenOptionsExt;
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(value.as_bytes())?;
    Ok(())
}

#[cfg(not(unix))]
fn write_private_key(path: &std::path::Path, value: &str) -> Result<()> {
    let mut file = OpenOptions::new().create_new(true).write(true).open(path)?;
    file.write_all(value.as_bytes())?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn credential_store_key() -> Option<[u8; 32]> {
    let output = std::process::Command::new("/usr/bin/security")
        .args([
            "find-generic-password",
            "-s",
            "perspt-grant-signing-key",
            "-w",
        ])
        .output()
        .ok()?;
    if output.status.success() {
        decode_key(String::from_utf8_lossy(&output.stdout).trim()).ok()
    } else {
        None
    }
}

#[cfg(target_os = "linux")]
fn credential_store_key() -> Option<[u8; 32]> {
    let output = std::process::Command::new("secret-tool")
        .args([
            "lookup",
            "application",
            "perspt",
            "purpose",
            "grant-signing",
        ])
        .output()
        .ok()?;
    if output.status.success() {
        decode_key(String::from_utf8_lossy(&output.stdout).trim()).ok()
    } else {
        None
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn credential_store_key() -> Option<[u8; 32]> {
    None
}

fn decode_key(value: &str) -> Result<[u8; 32]> {
    anyhow::ensure!(
        value.len() == 64,
        "grant signing key must be 64 hex characters"
    );
    let mut bytes = [0u8; 32];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16)
            .context("grant signing key contains invalid hex")?;
    }
    Ok(bytes)
}

fn encode_key(bytes: &[u8; 32]) -> String {
    use std::fmt::Write as _;
    let mut value = String::with_capacity(64);
    for byte in bytes {
        let _ = write!(&mut value, "{byte:02x}");
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_encoding_round_trips() {
        let bytes = [19u8; 32];
        assert_eq!(decode_key(&encode_key(&bytes)).unwrap(), bytes);
    }
}
