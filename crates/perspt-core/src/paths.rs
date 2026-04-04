//! Centralized platform-aware path helpers for Perspt.
//!
//! Three-tier model:
//!   - **Config**: `dirs::config_dir()/perspt/` — config.toml, policy rules
//!   - **Data**:   `dirs::data_local_dir()/perspt/` — perspt.db
//!   - **Project**: `<working_dir>/.perspt/` — sandboxes, scratch state

use std::path::PathBuf;

/// Platform config directory: `~/.config/perspt/` (Linux) or `~/Library/Application Support/perspt/` (macOS).
pub fn config_dir() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("perspt"))
}

/// Path to the main configuration file: `<config_dir>/config.toml`.
pub fn config_file() -> Option<PathBuf> {
    config_dir().map(|d| d.join("config.toml"))
}

/// Path to the policy rules directory: `<config_dir>/rules/`.
pub fn policy_dir() -> Option<PathBuf> {
    config_dir().map(|d| d.join("rules"))
}

/// Platform data directory: `~/.local/share/perspt/` (Linux) or `~/Library/Application Support/perspt/` (macOS).
pub fn data_dir() -> Option<PathBuf> {
    dirs::data_local_dir().map(|d| d.join("perspt"))
}

/// Path to the DuckDB database file: `<data_dir>/perspt.db`.
pub fn database_path() -> Option<PathBuf> {
    data_dir().map(|d| d.join("perspt.db"))
}

/// Returns the legacy `~/.perspt` directory if it exists on disk.
pub fn legacy_dir() -> Option<PathBuf> {
    dirs::home_dir()
        .map(|h| h.join(".perspt"))
        .filter(|p| p.is_dir())
}

/// Check for legacy paths and log migration warnings.
///
/// Call once at startup. If `~/.perspt/config.toml` or `~/.perspt/rules/` exist
/// while the new platform directory is empty, warn the user.
pub fn check_legacy_migration() {
    let Some(legacy) = legacy_dir() else {
        return;
    };

    let legacy_config = legacy.join("config.toml");
    let legacy_rules = legacy.join("rules");

    let new_config = config_file();
    let new_rules = policy_dir();

    if legacy_config.exists() {
        let target = new_config
            .as_deref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "<config_dir>/perspt/config.toml".into());
        if new_config.as_ref().is_none_or(|p| !p.exists()) {
            log::warn!(
                "Legacy config found at {}. Consider moving it to {}",
                legacy_config.display(),
                target
            );
        }
    }

    if legacy_rules.is_dir() {
        let target = new_rules
            .as_deref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "<config_dir>/perspt/rules/".into());
        if new_rules.as_ref().is_none_or(|p| !p.is_dir()) {
            log::warn!(
                "Legacy rules found at {}. Consider moving them to {}",
                legacy_rules.display(),
                target
            );
        }
    }
}

/// Resolve the config file path, falling back to legacy `~/.perspt/config.toml` if
/// the platform path doesn't exist yet.
pub fn resolve_config_file() -> Option<PathBuf> {
    // Prefer platform path if it exists
    if let Some(path) = config_file() {
        if path.exists() {
            return Some(path);
        }
    }
    // Fall back to legacy
    legacy_dir()
        .map(|d| d.join("config.toml"))
        .filter(|p| p.exists())
}

/// Resolve the policy directory, falling back to legacy `~/.perspt/rules/` if
/// the platform path doesn't exist yet.
pub fn resolve_policy_dir() -> Option<PathBuf> {
    // Prefer platform path if it exists
    if let Some(path) = policy_dir() {
        if path.is_dir() {
            return Some(path);
        }
    }
    // Fall back to legacy
    legacy_dir().map(|d| d.join("rules")).filter(|p| p.is_dir())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_dir_ends_with_perspt() {
        if let Some(dir) = config_dir() {
            assert!(dir.ends_with("perspt"));
        }
    }

    #[test]
    fn config_file_ends_with_config_toml() {
        if let Some(path) = config_file() {
            assert_eq!(path.file_name().unwrap(), "config.toml");
        }
    }

    #[test]
    fn policy_dir_ends_with_rules() {
        if let Some(dir) = policy_dir() {
            assert!(dir.ends_with("rules"));
        }
    }

    #[test]
    fn data_dir_ends_with_perspt() {
        if let Some(dir) = data_dir() {
            assert!(dir.ends_with("perspt"));
        }
    }

    #[test]
    fn database_path_ends_with_db() {
        if let Some(path) = database_path() {
            assert_eq!(path.file_name().unwrap(), "perspt.db");
        }
    }
}
