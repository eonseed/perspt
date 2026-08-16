//! Catalog entries (PSP-9 system 5).
//!
//! Each entry binds a tool name to its argument schema, its `EffectKind`,
//! its `RiskClass`, and its declarative scheduler footprint. The catalog is
//! data, so a domain package — or an external tool server (system 13) — can
//! extend it without forking the harness.

use serde::{Deserialize, Serialize};

use super::footprint::FootprintSpec;
use crate::capability::{EffectKind, RiskClass};
use crate::error::{Result, SdkError};
use crate::model::ToolSpec;

/// Where a tool entry came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolOrigin {
    /// The SDK's base catalog.
    Builtin,
    /// Contributed by the active `AgentDomainPackage`.
    Domain(String),
    /// Registered by an external tool server (system 13). Registration never
    /// mints authority: the session must already hold a capability for the
    /// declared effect.
    External(String),
}

/// One tool in the catalog.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolEntry {
    pub name: String,
    pub description: String,
    pub effect: EffectKind,
    pub risk: RiskClass,
    /// JSON Schema for the tool's arguments.
    pub schema: serde_json::Value,
    /// Declarative selectors mapping validated arguments to resources.
    pub footprint: FootprintSpec,
    /// Whether the tool mutates durable state (gates R5 bracketing).
    pub durable: bool,
    pub origin: ToolOrigin,
}

impl ToolEntry {
    /// Validate the entry at catalog-assembly time (Gates J and P).
    ///
    /// The schema must be an object schema; the footprint must reference
    /// only declared arguments; and a mutating effect must declare a
    /// footprint.
    pub fn validate(&self) -> Result<()> {
        if self.name.is_empty() {
            return Err(SdkError::Domain("tool entry with empty name".into()));
        }
        if self.schema.get("type").and_then(|t| t.as_str()) != Some("object") {
            return Err(SdkError::Domain(format!(
                "tool {:?}: argument schema must be an object schema",
                self.name
            )));
        }
        // `AskUser` is interactive, not mutating: it touches no workspace
        // state, so it carries no footprint obligation.
        let mutates = !self.effect.is_read_only() && self.effect != EffectKind::AskUser;
        self.footprint
            .validate(&self.schema, mutates)
            .map_err(|e| SdkError::Domain(format!("tool {:?}: {e}", self.name)))
    }

    /// The wire-neutral spec sent to a model.
    pub fn to_spec(&self, strict: bool) -> ToolSpec {
        ToolSpec {
            name: self.name.clone(),
            description: self.description.clone(),
            schema: self.schema.clone(),
            strict,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::toolset::footprint::{AccessMode, ResourceSelector};

    fn entry() -> ToolEntry {
        ToolEntry {
            name: "edit_file".into(),
            description: "Exact-string replace".into(),
            effect: EffectKind::ApplyPatch,
            risk: RiskClass::Medium,
            schema: serde_json::json!({
                "type": "object",
                "properties": {"path": {"type": "string"}},
                "required": ["path"],
            }),
            footprint: FootprintSpec::new(vec![ResourceSelector::PathArgument {
                field: "path".into(),
                access: AccessMode::Write,
            }]),
            durable: false,
            origin: ToolOrigin::Builtin,
        }
    }

    #[test]
    fn a_well_formed_entry_validates() {
        assert!(entry().validate().is_ok());
    }

    #[test]
    fn a_non_object_schema_is_rejected() {
        let mut bad = entry();
        bad.schema = serde_json::json!({"type": "string"});
        assert!(bad.validate().is_err());
    }

    #[test]
    fn a_mutating_entry_without_a_footprint_is_rejected_at_assembly() {
        let mut bad = entry();
        bad.footprint = FootprintSpec::default();
        assert!(bad.validate().is_err());
    }

    #[test]
    fn spec_carries_schema_and_strictness() {
        let spec = entry().to_spec(true);
        assert_eq!(spec.name, "edit_file");
        assert!(spec.strict);
    }
}
