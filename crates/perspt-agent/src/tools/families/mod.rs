//! First-party tool families shipped through the open execution plane.
//!
//! Each family is registered by the composition root exactly like a
//! third-party one — catalog entries via `Psp9AgentRuntime::with_tool_family`
//! and handlers via [`CandidateHandlerRegistry::register`] — so they double
//! as the standing proof that the plane stays open. A future family is a
//! crate depending on this handler contract; nothing in the loop, the
//! candidate, or the node assembly names these tools.

pub mod db;
pub mod system;

use anyhow::Result;
use perspt_sdk::ToolEntry;

use super::handlers::CandidateHandlerRegistry;

/// Catalog entries for every shipped family.
pub fn standard_family_entries() -> Vec<ToolEntry> {
    let mut entries = system::entries();
    entries.extend(db::entries());
    entries
}

/// Register every shipped family's handlers.
pub fn register_standard_families(registry: &mut CandidateHandlerRegistry) -> Result<()> {
    system::register(registry)?;
    db::register(registry)?;
    Ok(())
}

/// One shared entry constructor for family modules.
pub(crate) fn family_entry(
    name: &str,
    description: &str,
    effect: perspt_sdk::EffectKind,
    schema: serde_json::Value,
    footprint: perspt_sdk::FootprintSpec,
) -> ToolEntry {
    ToolEntry {
        name: name.into(),
        description: description.into(),
        effect,
        risk: perspt_sdk::RiskClass::Low,
        schema,
        footprint,
        proposal_bindings: Vec::new(),
        durable: false,
        origin: perspt_sdk::ToolOrigin::Builtin,
    }
}

/// Object schema helper matching the base catalog's shape.
pub(crate) fn object_schema(fields: &[(&str, &str, &str, bool)]) -> serde_json::Value {
    let mut properties = serde_json::Map::new();
    let mut required = Vec::new();
    for (name, kind, description, is_required) in fields {
        properties.insert(
            (*name).to_string(),
            serde_json::json!({"type": kind, "description": description}),
        );
        if *is_required {
            required.push(serde_json::Value::String((*name).to_string()));
        }
    }
    serde_json::json!({
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": false,
    })
}
