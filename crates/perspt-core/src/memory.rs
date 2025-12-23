//! PERSPT.md Parser - Project Memory
//!
//! Parses hierarchical project memory files inspired by CLAUDE.md.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Project memory configuration from PERSPT.md
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProjectMemory {
    /// Project name
    pub name: Option<String>,
    /// Project description
    pub description: Option<String>,
    /// Tech stack information
    pub tech_stack: Vec<String>,
    /// Design patterns to follow
    pub design_patterns: Vec<String>,
    /// Architectural constraints
    pub constraints: Vec<String>,
    /// Custom agent instructions
    pub agent_instructions: HashMap<String, String>,
    /// File patterns to ignore
    pub ignore_patterns: Vec<String>,
}

impl ProjectMemory {
    /// Load project memory from a PERSPT.md file
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            log::info!("No PERSPT.md found at {:?}, using defaults", path);
            return Ok(Self::default());
        }

        let content = std::fs::read_to_string(path)?;
        Self::parse(&content)
    }

    /// Parse PERSPT.md content
    pub fn parse(content: &str) -> Result<Self> {
        let mut memory = Self::default();
        let mut current_section: Option<&str> = None;

        for line in content.lines() {
            let trimmed = line.trim();

            // Skip empty lines
            if trimmed.is_empty() {
                continue;
            }

            // Detect section headers
            if let Some(name) = trimmed.strip_prefix("# ") {
                memory.name = Some(name.to_string());
                continue;
            }

            if trimmed.starts_with("## ") {
                current_section = Some(match trimmed.to_lowercase().as_str() {
                    s if s.contains("description") => "description",
                    s if s.contains("tech") || s.contains("stack") => "tech_stack",
                    s if s.contains("pattern") || s.contains("design") => "design_patterns",
                    s if s.contains("constraint") || s.contains("rule") => "constraints",
                    s if s.contains("ignore") => "ignore",
                    s if s.contains("agent") || s.contains("instruction") => "agent_instructions",
                    _ => "unknown",
                });
                continue;
            }

            // Parse content based on current section
            if let Some(section) = current_section {
                if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
                    let item = trimmed[2..].to_string();
                    match section {
                        "tech_stack" => memory.tech_stack.push(item),
                        "design_patterns" => memory.design_patterns.push(item),
                        "constraints" => memory.constraints.push(item),
                        "ignore" => memory.ignore_patterns.push(item),
                        _ => {}
                    }
                } else if section == "description" && memory.description.is_none() {
                    memory.description = Some(trimmed.to_string());
                }
            }
        }

        log::info!("Loaded project memory: {:?}", memory.name);
        Ok(memory)
    }

    /// Get the project name or a default
    pub fn get_name(&self) -> &str {
        self.name.as_deref().unwrap_or("Unnamed Project")
    }

    /// Build a context string for LLM prompts
    pub fn to_context_string(&self) -> String {
        let mut parts = Vec::new();

        if let Some(ref desc) = self.description {
            parts.push(format!("Project Description: {}", desc));
        }

        if !self.tech_stack.is_empty() {
            parts.push(format!("Tech Stack: {}", self.tech_stack.join(", ")));
        }

        if !self.design_patterns.is_empty() {
            parts.push(format!(
                "Design Patterns: {}",
                self.design_patterns.join(", ")
            ));
        }

        if !self.constraints.is_empty() {
            parts.push(format!("Constraints: {}", self.constraints.join("; ")));
        }

        parts.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_empty() {
        let memory = ProjectMemory::parse("").unwrap();
        assert!(memory.name.is_none());
    }

    #[test]
    fn test_parse_basic() {
        let content = r#"# My Project

## Description
A sample project for testing.

## Tech Stack
- Rust
- Tokio
- PostgreSQL

## Constraints
- No unsafe code
- Must be async
"#;
        let memory = ProjectMemory::parse(content).unwrap();
        assert_eq!(memory.name, Some("My Project".to_string()));
        assert_eq!(memory.tech_stack.len(), 3);
        assert_eq!(memory.constraints.len(), 2);
    }
}
