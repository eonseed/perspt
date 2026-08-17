use super::*;
/// PSP-5: Node class distinguishing interface, implementation, and integration nodes
///
/// - **Interface** nodes define exported signatures, schemas, and verifier scope.
/// - **Implementation** nodes operate on node-owned files plus sealed interfaces.
/// - **Integration** nodes reconcile cross-owner or cross-plugin boundaries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum NodeClass {
    /// Defines exported signatures, schemas, ownership manifests
    Interface,
    /// Operates on node-owned files plus adjacent sealed interfaces
    #[default]
    Implementation,
    /// Reconciles cross-owner or cross-plugin boundaries
    Integration,
}

impl std::fmt::Display for NodeClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NodeClass::Interface => write!(f, "interface"),
            NodeClass::Implementation => write!(f, "implementation"),
            NodeClass::Integration => write!(f, "integration"),
        }
    }
}

/// PSP-5: Verifier strictness presets
///
/// Controls which verification stages are required for stability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum VerifierStrictness {
    /// Default: compilation + tests required, warnings allowed
    #[default]
    Default,
    /// Strict: compilation + tests + linting (e.g. clippy -D warnings)
    Strict,
    /// Minimal: syntax/parse check only, no tests required
    Minimal,
}
