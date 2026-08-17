use super::*;

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

    /// An empty registry, for embedders that assemble their own plugin set.
    pub fn empty() -> Self {
        Self {
            plugins: Vec::new(),
        }
    }

    /// Register an additional language plugin. Detection order is
    /// registration order; built-ins come first in [`Self::new`].
    pub fn register(&mut self, plugin: Box<dyn LanguagePlugin>) {
        self.plugins.push(plugin);
    }

    /// Detect which plugin should handle the given path (first match)
    pub fn detect(&self, path: &Path) -> Option<&dyn LanguagePlugin> {
        self.plugins
            .iter()
            .find(|p| p.detect(path))
            .map(|p| p.as_ref())
    }

    /// PSP-5: Detect ALL plugins that match the given path (polyglot support)
    ///
    /// Returns all matching plugins instead of just the first, enabling
    /// multi-language verification in polyglot repositories.
    pub fn detect_all(&self, path: &Path) -> Vec<&dyn LanguagePlugin> {
        self.plugins
            .iter()
            .filter(|p| p.detect(path))
            .map(|p| p.as_ref())
            .collect()
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
