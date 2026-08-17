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
        validate_schema_profile(&self.name, &self.schema)?;
        // `AskUser` is interactive, not mutating: it touches no workspace
        // state, so it carries no footprint obligation.
        let mutates = !self.effect.is_read_only() && self.effect != EffectKind::AskUser;
        self.footprint
            .validate(&self.schema, mutates)
            .map_err(|e| SdkError::Domain(format!("tool {:?}: {e}", self.name)))
    }

    /// Validate provider-supplied arguments locally. Provider strict-schema
    /// support is only an optimization; this is the harness trust boundary.
    /// Catalog assembly guarantees the schema belongs to the profile this
    /// validator implements completely.
    pub fn validate_arguments(&self, arguments: &serde_json::Value) -> Result<()> {
        let object = arguments.as_object().ok_or_else(|| {
            SdkError::Domain(format!("tool {:?}: arguments must be an object", self.name))
        })?;
        let properties = self
            .schema
            .get("properties")
            .and_then(serde_json::Value::as_object)
            .expect("catalog validation requires an object properties map");
        if let Some(required) = self
            .schema
            .get("required")
            .and_then(serde_json::Value::as_array)
        {
            for field in required.iter().filter_map(serde_json::Value::as_str) {
                if !object.contains_key(field) {
                    return Err(SdkError::Domain(format!(
                        "tool {:?}: missing required argument {field:?}",
                        self.name
                    )));
                }
            }
        }
        if self.schema.get("additionalProperties") == Some(&serde_json::Value::Bool(false)) {
            if let Some(field) = object.keys().find(|field| !properties.contains_key(*field)) {
                return Err(SdkError::Domain(format!(
                    "tool {:?}: unknown argument {field:?}",
                    self.name
                )));
            }
        }
        for (field, value) in object {
            let Some(field_schema) = properties.get(field) else {
                continue;
            };
            let kind = field_schema
                .get("type")
                .and_then(serde_json::Value::as_str)
                .expect("catalog validation requires a property type");
            if !value_matches_type(value, kind) {
                return Err(SdkError::Domain(format!(
                    "tool {:?}: argument {field:?} must be {kind}",
                    self.name
                )));
            }
            if let Some(allowed) = field_schema
                .get("enum")
                .and_then(serde_json::Value::as_array)
            {
                if !allowed.contains(value) {
                    return Err(SdkError::Domain(format!(
                        "tool {:?}: argument {field:?} is outside its enum",
                        self.name
                    )));
                }
            }
        }
        Ok(())
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

fn validate_schema_profile(tool: &str, schema: &serde_json::Value) -> Result<()> {
    const ROOT_KEYS: &[&str] = &[
        "$schema",
        "title",
        "description",
        "type",
        "properties",
        "required",
        "additionalProperties",
    ];
    const PROPERTY_KEYS: &[&str] = &["title", "description", "type", "enum", "default"];
    let root = schema
        .as_object()
        .expect("object schema checked by ToolEntry::validate");
    if let Some(key) = root.keys().find(|key| !ROOT_KEYS.contains(&key.as_str())) {
        return Err(SdkError::Domain(format!(
            "tool {tool:?}: unsupported argument-schema keyword {key:?}"
        )));
    }
    let properties = root
        .get("properties")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| {
            SdkError::Domain(format!(
                "tool {tool:?}: schema must declare object properties"
            ))
        })?;
    if let Some(additional) = root.get("additionalProperties") {
        if !additional.is_boolean() {
            return Err(SdkError::Domain(format!(
                "tool {tool:?}: additionalProperties must be boolean"
            )));
        }
    }
    if let Some(required) = root.get("required") {
        let required = required
            .as_array()
            .ok_or_else(|| SdkError::Domain(format!("tool {tool:?}: required must be an array")))?;
        for field in required {
            let field = field.as_str().ok_or_else(|| {
                SdkError::Domain(format!("tool {tool:?}: required entries must be strings"))
            })?;
            if !properties.contains_key(field) {
                return Err(SdkError::Domain(format!(
                    "tool {tool:?}: required argument {field:?} is not a property"
                )));
            }
        }
    }
    for (field, property) in properties {
        let property = property.as_object().ok_or_else(|| {
            SdkError::Domain(format!(
                "tool {tool:?}: property {field:?} must be a schema object"
            ))
        })?;
        if let Some(key) = property
            .keys()
            .find(|key| !PROPERTY_KEYS.contains(&key.as_str()))
        {
            return Err(SdkError::Domain(format!(
                "tool {tool:?}: property {field:?} uses unsupported keyword {key:?}"
            )));
        }
        let kind = property
            .get("type")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                SdkError::Domain(format!("tool {tool:?}: property {field:?} has no type"))
            })?;
        if !matches!(
            kind,
            "string" | "integer" | "number" | "boolean" | "array" | "object" | "null"
        ) {
            return Err(SdkError::Domain(format!(
                "tool {tool:?}: property {field:?} has unsupported type {kind:?}"
            )));
        }
        if matches!(kind, "array" | "object") {
            return Err(SdkError::Domain(format!(
                "tool {tool:?}: nested {kind} property {field:?} is outside the admitted schema profile"
            )));
        }
        if let Some(values) = property.get("enum") {
            let values = values.as_array().ok_or_else(|| {
                SdkError::Domain(format!(
                    "tool {tool:?}: property {field:?} enum must be an array"
                ))
            })?;
            if values.iter().any(|value| !value_matches_type(value, kind)) {
                return Err(SdkError::Domain(format!(
                    "tool {tool:?}: property {field:?} enum contains a value outside type {kind}"
                )));
            }
        }
    }
    Ok(())
}

fn value_matches_type(value: &serde_json::Value, kind: &str) -> bool {
    match kind {
        "string" => value.is_string(),
        "integer" => value.is_i64() || value.is_u64(),
        "number" => value.is_number(),
        "boolean" => value.is_boolean(),
        "array" => value.is_array(),
        "object" => value.is_object(),
        "null" => value.is_null(),
        _ => false,
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

    #[test]
    fn runtime_arguments_are_checked_against_the_admitted_profile() {
        let mut entry = entry();
        entry.schema["additionalProperties"] = serde_json::Value::Bool(false);
        assert!(entry
            .validate_arguments(&serde_json::json!({"path": "src/lib.rs"}))
            .is_ok());
        assert!(entry
            .validate_arguments(&serde_json::json!({"path": 7}))
            .is_err());
        assert!(entry
            .validate_arguments(&serde_json::json!({"path": "src/lib.rs", "extra": true}))
            .is_err());
    }

    #[test]
    fn unsupported_schema_constructs_fail_at_catalog_assembly() {
        let mut nested = entry();
        nested.schema["properties"]["path"]["pattern"] = serde_json::json!("^src/");
        assert!(nested.validate().is_err());

        let mut array = entry();
        array.schema["properties"]["path"]["type"] = serde_json::json!("array");
        assert!(array.validate().is_err());
    }
}
