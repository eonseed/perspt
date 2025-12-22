//! Config command - configuration management

use anyhow::Result;
use std::fs;
use std::path::Path;
use std::process::Command;

/// Manage configuration
pub async fn run(show: bool, set: Option<String>, edit: bool) -> Result<()> {
    let config_path = get_config_path();

    if show {
        show_config(&config_path)?;
    } else if let Some(kv) = set {
        set_config_value(&config_path, &kv)?;
    } else if edit {
        edit_config(&config_path)?;
    } else {
        println!("Configuration file: {:?}", config_path);
        println!();
        println!("Usage:");
        println!("  perspt config --show     Show current configuration");
        println!("  perspt config --set KEY=VALUE  Set a value");
        println!("  perspt config --edit     Open in $EDITOR");
    }

    Ok(())
}

fn get_config_path() -> std::path::PathBuf {
    dirs::home_dir()
        .map(|h| h.join(".perspt").join("config.toml"))
        .unwrap_or_else(|| Path::new(".perspt/config.toml").to_path_buf())
}

fn show_config(path: &Path) -> Result<()> {
    if path.exists() {
        let content = fs::read_to_string(path)?;
        println!("{}", content);
    } else {
        println!("No configuration file found at {:?}", path);
        println!("Run `perspt init` to create default configuration.");
    }
    Ok(())
}

fn set_config_value(path: &Path, kv: &str) -> Result<()> {
    let parts: Vec<&str> = kv.splitn(2, '=').collect();
    if parts.len() != 2 {
        anyhow::bail!("Invalid format. Use KEY=VALUE");
    }

    let (key, value) = (parts[0], parts[1]);

    // Read existing or create new
    let mut content = if path.exists() {
        fs::read_to_string(path)?
    } else {
        String::new()
    };

    // Simple append (in real impl, parse TOML properly)
    content.push_str(&format!("\n{} = \"{}\"\n", key, value));

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, content)?;

    println!("✓ Set {} = {}", key, value);
    Ok(())
}

fn edit_config(path: &Path) -> Result<()> {
    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());

    Command::new(editor).arg(path).status()?;

    Ok(())
}
