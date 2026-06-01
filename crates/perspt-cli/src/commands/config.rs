//! Config command - configuration management

use anyhow::Result;
use perspt_core::Config;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Manage configuration.
///
/// `config_override` is the resolved `--config <PATH>` value, if the user passed one.
pub async fn run(
    show: bool,
    set: Option<String>,
    edit: bool,
    config_override: Option<PathBuf>,
) -> Result<()> {
    let config_path = resolve_config_path(config_override);

    if show {
        show_config(&config_path)?;
    } else if let Some(kv) = set {
        set_config_value(&config_path, &kv)?;
    } else if edit {
        edit_config(&config_path)?;
    } else {
        println!("Configuration file: {}", config_path.display());
        println!();
        println!("Usage:");
        println!("  perspt config --show           Show effective configuration");
        println!("  perspt config --set KEY=VALUE  Set a value");
        println!("  perspt config --edit           Open in $EDITOR");
    }

    Ok(())
}

/// Resolve the config path: an explicit `--config` always wins, otherwise use
/// the platform path (with legacy fallback for reads).
fn resolve_config_path(config_override: Option<PathBuf>) -> PathBuf {
    if let Some(path) = config_override {
        return path;
    }
    perspt_core::paths::resolve_config_file()
        .or_else(perspt_core::paths::config_file)
        .unwrap_or_else(|| Path::new(".perspt/config.toml").to_path_buf())
}

fn show_config(path: &Path) -> Result<()> {
    let config = Config::load_from_path(path)?;
    let exists = path.exists();

    println!("Config file: {}", path.display());
    if exists {
        println!("Status: loaded from file");
    } else {
        println!("Status: no file yet (showing effective defaults)");
        println!("Tip: create one with `perspt config --set provider=openai` or `--edit`.");
    }
    println!();

    // Show the effective configuration with the API key masked.
    let rendered = config.masked().to_toml_string()?;
    if rendered.trim().is_empty() {
        println!("# No values set; built-in defaults and environment detection apply.");
    } else {
        print!("{}", rendered);
    }
    Ok(())
}

fn set_config_value(path: &Path, kv: &str) -> Result<()> {
    let parts: Vec<&str> = kv.splitn(2, '=').collect();
    if parts.len() != 2 {
        anyhow::bail!("Invalid format. Use KEY=VALUE");
    }
    let (key, value) = (parts[0].trim(), parts[1]);

    // Structured read-modify-write so repeated --set never corrupts the file.
    let mut config = Config::load_from_path(path)?;
    config.set_value(key, value)?;
    let content = config.to_toml_string()?;

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, content)?;

    println!("✓ Set {} = {}", key, value);
    Ok(())
}

fn edit_config(path: &Path) -> Result<()> {
    if !path.exists() {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, "# Perspt configuration (TOML)\n")?;
    }

    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());
    Command::new(editor).arg(path).status()?;

    Ok(())
}
