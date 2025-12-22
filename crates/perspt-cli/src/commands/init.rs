//! Init command - project initialization

use anyhow::Result;
use std::fs;
use std::path::Path;

/// Initialize project configuration
pub async fn run(memory: bool, rules: bool) -> Result<()> {
    if memory {
        create_project_memory().await?;
    }

    if rules {
        create_default_rules().await?;
    }

    if !memory && !rules {
        println!("Usage: perspt init [--memory] [--rules]");
        println!();
        println!("Options:");
        println!("  --memory    Create PERSPT.md project memory file");
        println!("  --rules     Create default Starlark policy rules");
    }

    Ok(())
}

/// Create PERSPT.md project memory file
async fn create_project_memory() -> Result<()> {
    let path = Path::new("PERSPT.md");

    if path.exists() {
        println!("⚠ PERSPT.md already exists");
        return Ok(());
    }

    let template = r#"# Project Memory

## Project Name
[Your Project Name]

## Description
[Brief description of the project]

## Tech Stack
- Language: [e.g., Rust, Python]
- Framework: [e.g., Axum, FastAPI]
- Database: [e.g., PostgreSQL, SQLite]

## Design Patterns
- [List important architectural patterns]

## Constraints
- [List important constraints or requirements]

## Ignore
- target/
- node_modules/
- .git/
"#;

    fs::write(path, template)?;
    println!("✓ Created PERSPT.md");

    Ok(())
}

/// Create default Starlark rules
async fn create_default_rules() -> Result<()> {
    let rules_dir = dirs::home_dir()
        .map(|h| h.join(".perspt").join("rules"))
        .unwrap_or_else(|| Path::new(".perspt/rules").to_path_buf());

    fs::create_dir_all(&rules_dir)?;

    let default_rules = r#"# Default Perspt Security Rules
# Language: Starlark

def evaluate_command(command):
    """Evaluate a command and return a decision."""
    
    # Block destructive commands
    dangerous = ["rm -rf /", "sudo rm", "mkfs", "dd if="]
    for pattern in dangerous:
        if pattern in command:
            return {"decision": "deny", "reason": "Dangerous command pattern: " + pattern}
    
    # Prompt for network access
    network = ["curl", "wget", "ssh", "scp"]
    for pattern in network:
        if pattern in command:
            return {"decision": "prompt", "reason": "Network access required"}
    
    # Prompt for git push
    if "git push" in command:
        return {"decision": "prompt", "reason": "Git push requires confirmation"}
    
    # Allow safe commands
    return {"decision": "allow", "reason": ""}
"#;

    let rules_path = rules_dir.join("default.star");
    fs::write(&rules_path, default_rules)?;
    println!("✓ Created {:?}", rules_path);

    Ok(())
}
