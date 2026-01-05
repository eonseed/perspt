//! Language Plugin Architecture
//!
//! Provides a trait-based plugin system for polyglot support.
//! Each language (Rust, Python, JS, etc.) implements this trait.

use serde::{Deserialize, Serialize};
use std::path::Path;

/// LSP Configuration for a language
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspConfig {
    /// LSP server binary name
    pub server_binary: String,
    /// Arguments to pass to the server
    pub args: Vec<String>,
    /// Language ID for textDocument/didOpen
    pub language_id: String,
}

/// Options for project initialization
#[derive(Debug, Clone, Default)]
pub struct InitOptions {
    /// Project name
    pub name: String,
    /// Whether to use a specific package manager
    pub package_manager: Option<String>,
    /// Additional flags
    pub flags: Vec<String>,
}

/// A plugin for a specific programming language
pub trait LanguagePlugin: Send + Sync {
    /// Name of the language
    fn name(&self) -> &str;

    /// File extensions this plugin handles
    fn extensions(&self) -> &[&str];

    /// Key files that identify this language (e.g., Cargo.toml, pyproject.toml)
    fn key_files(&self) -> &[&str];

    /// Detect if this plugin should handle the given project directory
    fn detect(&self, path: &Path) -> bool {
        // Check for key files
        for key_file in self.key_files() {
            if path.join(key_file).exists() {
                return true;
            }
        }

        // Check for files with handled extensions
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                if let Some(ext) = entry.path().extension() {
                    let ext_str = ext.to_string_lossy();
                    if self.extensions().iter().any(|e| *e == ext_str) {
                        return true;
                    }
                }
            }
        }

        false
    }

    /// Get the LSP configuration for this language
    fn get_lsp_config(&self) -> LspConfig;

    /// Get the command to initialize a new project
    fn init_command(&self, opts: &InitOptions) -> String;

    /// Get the command to run tests
    fn test_command(&self) -> String;

    /// Get the command to run the project (for verification)
    fn run_command(&self) -> String;
}

/// Rust language plugin
pub struct RustPlugin;

impl LanguagePlugin for RustPlugin {
    fn name(&self) -> &str {
        "rust"
    }

    fn extensions(&self) -> &[&str] {
        &["rs"]
    }

    fn key_files(&self) -> &[&str] {
        &["Cargo.toml", "Cargo.lock"]
    }

    fn get_lsp_config(&self) -> LspConfig {
        LspConfig {
            server_binary: "rust-analyzer".to_string(),
            args: vec![],
            language_id: "rust".to_string(),
        }
    }

    fn init_command(&self, opts: &InitOptions) -> String {
        if opts.name == "." || opts.name == "./" {
            "cargo init .".to_string()
        } else {
            format!("cargo new {}", opts.name)
        }
    }

    fn test_command(&self) -> String {
        "cargo test".to_string()
    }

    fn run_command(&self) -> String {
        "cargo run".to_string()
    }
}

/// Python language plugin (uses ty via uvx)
pub struct PythonPlugin;

impl LanguagePlugin for PythonPlugin {
    fn name(&self) -> &str {
        "python"
    }

    fn extensions(&self) -> &[&str] {
        &["py"]
    }

    fn key_files(&self) -> &[&str] {
        &["pyproject.toml", "setup.py", "requirements.txt", "uv.lock"]
    }

    fn get_lsp_config(&self) -> LspConfig {
        // Prefer ty (via uvx) as the native Python support
        // Falls back to pyright if ty is not available
        LspConfig {
            server_binary: "uvx".to_string(),
            args: vec!["ty".to_string(), "server".to_string()],
            language_id: "python".to_string(),
        }
    }

    fn init_command(&self, opts: &InitOptions) -> String {
        if opts.package_manager.as_deref() == Some("poetry") {
            if opts.name == "." || opts.name == "./" {
                "poetry init".to_string()
            } else {
                format!("poetry new {}", opts.name)
            }
        } else {
            // uv init supports "." for current directory
            format!("uv init {}", opts.name)
        }
    }

    fn test_command(&self) -> String {
        "pytest".to_string()
    }

    fn run_command(&self) -> String {
        "python -m main".to_string()
    }
}

/// JavaScript/TypeScript language plugin
pub struct JsPlugin;

impl LanguagePlugin for JsPlugin {
    fn name(&self) -> &str {
        "javascript"
    }

    fn extensions(&self) -> &[&str] {
        &["js", "ts", "jsx", "tsx"]
    }

    fn key_files(&self) -> &[&str] {
        &["package.json", "tsconfig.json"]
    }

    fn get_lsp_config(&self) -> LspConfig {
        LspConfig {
            server_binary: "typescript-language-server".to_string(),
            args: vec!["--stdio".to_string()],
            language_id: "typescript".to_string(),
        }
    }

    fn init_command(&self, opts: &InitOptions) -> String {
        format!("npm init -y && mv package.json {}/", opts.name)
    }

    fn test_command(&self) -> String {
        "npm test".to_string()
    }

    fn run_command(&self) -> String {
        "npm start".to_string()
    }
}

/// Plugin registry for dynamic language detection
pub struct PluginRegistry {
    plugins: Vec<Box<dyn LanguagePlugin>>,
}

impl PluginRegistry {
    /// Create a new registry with all built-in plugins
    pub fn new() -> Self {
        Self {
            plugins: vec![
                Box::new(RustPlugin),
                Box::new(PythonPlugin),
                Box::new(JsPlugin),
            ],
        }
    }

    /// Detect which plugin should handle the given path
    pub fn detect(&self, path: &Path) -> Option<&dyn LanguagePlugin> {
        self.plugins
            .iter()
            .find(|p| p.detect(path))
            .map(|p| p.as_ref())
    }

    /// Get a plugin by name
    pub fn get(&self, name: &str) -> Option<&dyn LanguagePlugin> {
        self.plugins
            .iter()
            .find(|p| p.name() == name)
            .map(|p| p.as_ref())
    }

    /// Get all registered plugins
    pub fn all(&self) -> &[Box<dyn LanguagePlugin>] {
        &self.plugins
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}
