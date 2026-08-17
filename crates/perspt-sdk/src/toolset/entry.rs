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

/// How validated arguments bind to a Paper III effect proposal. Bindings are
/// locally trusted catalog data; tool descriptions and server annotations do
/// not create them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum ProposalBinding {
    Path {
        field: String,
    },
    Command {
        field: String,
    },
    Url {
        field: String,
    },
    /// A bounded scalar array whose values bind to one proposal channel.
    MultiValue {
        field: String,
        target: MultiValueTarget,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MultiValueTarget {
    Path,
    Command,
    Url,
}

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
    /// Deterministic argument-to-proposal bindings.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub proposal_bindings: Vec<ProposalBinding>,
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
        validate_proposal_bindings(&self.name, &self.schema, &self.proposal_bindings)?;
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
        validate_object_value(&self.name, "arguments", arguments, &self.schema)
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
        validate_property_schema(tool, field, property, 0)?;
    }
    Ok(())
}

fn validate_property_schema(
    tool: &str,
    field: &str,
    schema: &serde_json::Value,
    object_depth: u8,
) -> Result<()> {
    const KEYS: &[&str] = &[
        "title",
        "description",
        "type",
        "enum",
        "default",
        "items",
        "minItems",
        "maxItems",
        "properties",
        "required",
        "additionalProperties",
    ];
    let property = schema.as_object().ok_or_else(|| {
        SdkError::Domain(format!(
            "tool {tool:?}: property {field:?} must be a schema object"
        ))
    })?;
    if let Some(key) = property.keys().find(|key| !KEYS.contains(&key.as_str())) {
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
    match kind {
        "string" | "integer" | "number" | "boolean" | "null" => {
            validate_enum(tool, field, property, kind)?;
        }
        "array" => validate_array_schema(tool, field, property, object_depth)?,
        "object" if object_depth == 0 => {
            validate_object_schema(tool, field, property, object_depth + 1)?;
        }
        "object" => {
            return Err(SdkError::Domain(format!(
                "tool {tool:?}: property {field:?} exceeds one nested object level"
            )));
        }
        _ => {
            return Err(SdkError::Domain(format!(
                "tool {tool:?}: property {field:?} has unsupported type {kind:?}"
            )));
        }
    }
    Ok(())
}

fn validate_array_schema(
    tool: &str,
    field: &str,
    property: &serde_json::Map<String, serde_json::Value>,
    object_depth: u8,
) -> Result<()> {
    let maximum = property
        .get("maxItems")
        .and_then(serde_json::Value::as_u64)
        .filter(|maximum| *maximum > 0)
        .ok_or_else(|| {
            SdkError::Domain(format!(
                "tool {tool:?}: array property {field:?} requires a positive maxItems"
            ))
        })?;
    if maximum > 256 {
        return Err(SdkError::Domain(format!(
            "tool {tool:?}: array property {field:?} maxItems exceeds 256"
        )));
    }
    let items = property.get("items").ok_or_else(|| {
        SdkError::Domain(format!(
            "tool {tool:?}: array property {field:?} requires items"
        ))
    })?;
    let item_kind = items.get("type").and_then(serde_json::Value::as_str);
    if !matches!(
        item_kind,
        Some("string" | "integer" | "number" | "boolean" | "null")
    ) {
        return Err(SdkError::Domain(format!(
            "tool {tool:?}: array property {field:?} must contain scalars"
        )));
    }
    validate_property_schema(tool, &format!("{field}[]"), items, object_depth)
}

fn validate_object_schema(
    tool: &str,
    field: &str,
    property: &serde_json::Map<String, serde_json::Value>,
    depth: u8,
) -> Result<()> {
    let children = property
        .get("properties")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| {
            SdkError::Domain(format!(
                "tool {tool:?}: object property {field:?} requires properties"
            ))
        })?;
    validate_required(tool, field, property, children)?;
    for (child, schema) in children {
        validate_property_schema(tool, &format!("{field}.{child}"), schema, depth)?;
    }
    Ok(())
}

fn validate_required(
    tool: &str,
    field: &str,
    schema: &serde_json::Map<String, serde_json::Value>,
    properties: &serde_json::Map<String, serde_json::Value>,
) -> Result<()> {
    let Some(required) = schema.get("required") else {
        return Ok(());
    };
    let required = required.as_array().ok_or_else(|| {
        SdkError::Domain(format!(
            "tool {tool:?}: {field:?} required must be an array"
        ))
    })?;
    for name in required {
        let name = name.as_str().ok_or_else(|| {
            SdkError::Domain(format!(
                "tool {tool:?}: {field:?} required entries must be strings"
            ))
        })?;
        if !properties.contains_key(name) {
            return Err(SdkError::Domain(format!(
                "tool {tool:?}: required property {field}.{name} is not declared"
            )));
        }
    }
    Ok(())
}

fn validate_enum(
    tool: &str,
    field: &str,
    property: &serde_json::Map<String, serde_json::Value>,
    kind: &str,
) -> Result<()> {
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
    Ok(())
}

fn validate_object_value(
    tool: &str,
    field: &str,
    value: &serde_json::Value,
    schema: &serde_json::Value,
) -> Result<()> {
    let object = value.as_object().ok_or_else(|| {
        SdkError::Domain(format!("tool {tool:?}: argument {field:?} must be object"))
    })?;
    let properties = schema
        .get("properties")
        .and_then(serde_json::Value::as_object)
        .expect("catalog validation requires object properties");
    if let Some(required) = schema.get("required").and_then(serde_json::Value::as_array) {
        for required_field in required.iter().filter_map(serde_json::Value::as_str) {
            if !object.contains_key(required_field) {
                return Err(SdkError::Domain(format!(
                    "tool {tool:?}: missing required argument {field}.{required_field}"
                )));
            }
        }
    }
    if schema.get("additionalProperties") == Some(&serde_json::Value::Bool(false)) {
        if let Some(unknown) = object.keys().find(|name| !properties.contains_key(*name)) {
            return Err(SdkError::Domain(format!(
                "tool {tool:?}: unknown argument {field}.{unknown}"
            )));
        }
    }
    for (name, child) in object {
        if let Some(child_schema) = properties.get(name) {
            validate_argument_value(tool, &format!("{field}.{name}"), child, child_schema)?;
        }
    }
    Ok(())
}

fn validate_argument_value(
    tool: &str,
    field: &str,
    value: &serde_json::Value,
    schema: &serde_json::Value,
) -> Result<()> {
    let kind = schema
        .get("type")
        .and_then(serde_json::Value::as_str)
        .expect("catalog validation requires property types");
    if !value_matches_type(value, kind) {
        return Err(SdkError::Domain(format!(
            "tool {tool:?}: argument {field:?} must be {kind}"
        )));
    }
    if let Some(allowed) = schema.get("enum").and_then(serde_json::Value::as_array) {
        if !allowed.contains(value) {
            return Err(SdkError::Domain(format!(
                "tool {tool:?}: argument {field:?} is outside its enum"
            )));
        }
    }
    match kind {
        "array" => {
            let values = value.as_array().expect("array type checked");
            let maximum = schema
                .get("maxItems")
                .and_then(serde_json::Value::as_u64)
                .expect("array schema validated") as usize;
            let minimum = schema
                .get("minItems")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0) as usize;
            if values.len() < minimum || values.len() > maximum {
                return Err(SdkError::Domain(format!(
                    "tool {tool:?}: argument {field:?} has an invalid item count"
                )));
            }
            let item_schema = schema.get("items").expect("array schema validated");
            for (index, item) in values.iter().enumerate() {
                validate_argument_value(tool, &format!("{field}[{index}]"), item, item_schema)?;
            }
        }
        "object" => validate_object_value(tool, field, value, schema)?,
        _ => {}
    }
    Ok(())
}

fn validate_proposal_bindings(
    tool: &str,
    schema: &serde_json::Value,
    bindings: &[ProposalBinding],
) -> Result<()> {
    let properties = schema
        .get("properties")
        .and_then(serde_json::Value::as_object)
        .expect("schema profile validates properties first");
    for binding in bindings {
        let (field, expected) = match binding {
            ProposalBinding::Path { field }
            | ProposalBinding::Command { field }
            | ProposalBinding::Url { field } => (field, "string"),
            ProposalBinding::MultiValue { field, .. } => (field, "array"),
        };
        let actual = properties
            .get(field)
            .and_then(|property| property.get("type"))
            .and_then(serde_json::Value::as_str);
        if actual != Some(expected) {
            return Err(SdkError::Domain(format!(
                "tool {tool:?}: proposal binding {field:?} requires {expected} arguments"
            )));
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
            proposal_bindings: vec![ProposalBinding::Path {
                field: "path".into(),
            }],
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

    #[test]
    fn bounded_scalar_arrays_and_one_nested_object_are_supported() {
        let mut composite = entry();
        composite.effect = EffectKind::SystemProbe;
        composite.footprint = FootprintSpec::default();
        composite.proposal_bindings.clear();
        composite.schema = serde_json::json!({
            "type": "object",
            "properties": {
                "names": {
                    "type": "array",
                    "items": {"type": "string"},
                    "maxItems": 2
                },
                "options": {
                    "type": "object",
                    "properties": {"enabled": {"type": "boolean"}},
                    "required": ["enabled"],
                    "additionalProperties": false
                }
            },
            "required": ["names", "options"],
            "additionalProperties": false
        });
        assert!(composite.validate().is_ok());
        assert!(composite
            .validate_arguments(&serde_json::json!({
                "names": ["one", "two"],
                "options": {"enabled": true}
            }))
            .is_ok());
        assert!(composite
            .validate_arguments(&serde_json::json!({
                "names": ["one", "two", "three"],
                "options": {"enabled": true}
            }))
            .is_err());
    }
}
