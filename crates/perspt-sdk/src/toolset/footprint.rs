//! Declarative footprints (PSP-9 systems 5 and 15).
//!
//! The footprint is **data, not an executable function pointer**. That
//! matters for serialization, replay, and external-tool registration: the
//! kernel resolves a [`FootprintSpec`] only after JSON-schema validation and
//! path canonicalization, and an unknown selector fails closed to
//! [`ResourceSelector::OpaqueWorkspace`].
//!
//! Theorem 8 is stated for events with `writes(e) ⊆ reads(e)`. A tool that
//! writes a resource it did not read is outside the commutation lemma, so a
//! write-mode selector contributes its resource to *both* sets — the
//! declarative form of the state-witness rule — and validation rejects
//! non-conforming specs at catalog-assembly time rather than at execution.

use serde::{Deserialize, Serialize};

use crate::error::{Result, SdkError};
use crate::scheduler::{Footprint, Resource};

/// Whether a selector reads or mutates its resource.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccessMode {
    Read,
    Write,
}

/// A declarative mapping from validated tool arguments to scheduler
/// resources.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum ResourceSelector {
    /// A fixed resource, independent of arguments.
    Literal {
        resource: Resource,
        access: AccessMode,
    },
    /// The canonicalized path found in the named (schema-validated) argument.
    PathArgument { field: String, access: AccessMode },
    /// The manifest owning the path in the named argument (e.g. the
    /// `Cargo.toml` above an edited source file).
    ParentManifestOf { field: String, access: AccessMode },
    /// The serving provider's rate limit — a throughput resource, never a
    /// required serialization (Gate P).
    ProviderRateLimit,
    /// Conflicts with everything: the fail-closed default for tools whose
    /// touched state cannot be described.
    OpaqueWorkspace,
}

/// The declared footprint of one tool.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FootprintSpec {
    pub selectors: Vec<ResourceSelector>,
}

impl FootprintSpec {
    /// A spec containing exactly the given selectors.
    pub fn new(selectors: Vec<ResourceSelector>) -> Self {
        Self { selectors }
    }

    /// The fail-closed spec: serialize against everything.
    pub fn opaque() -> Self {
        Self::new(vec![ResourceSelector::OpaqueWorkspace])
    }

    /// Validate the spec against the tool's argument schema
    /// (catalog-assembly time, Gate P).
    ///
    /// Every field-referencing selector must name a property the schema
    /// declares, and a mutating tool must declare at least one selector.
    pub fn validate(&self, schema: &serde_json::Value, mutates: bool) -> Result<()> {
        if mutates && self.selectors.is_empty() {
            return Err(SdkError::Domain(
                "a mutating tool must declare a footprint or opt into OpaqueWorkspace".into(),
            ));
        }
        let properties = schema.get("properties").and_then(|p| p.as_object());
        for selector in &self.selectors {
            let field = match selector {
                ResourceSelector::PathArgument { field, .. }
                | ResourceSelector::ParentManifestOf { field, .. } => field,
                _ => continue,
            };
            let declared = properties.map(|p| p.contains_key(field)).unwrap_or(false);
            if !declared {
                return Err(SdkError::Domain(format!(
                    "footprint selector references argument {field:?} that the tool schema \
                     does not declare"
                )));
            }
        }
        Ok(())
    }

    /// Resolve into a concrete [`Footprint`] from schema-validated,
    /// canonicalized arguments.
    ///
    /// `writes(e) ⊆ reads(e)` holds by construction: every written resource
    /// is also read. A field-referencing selector whose argument is absent
    /// resolves to nothing (the argument was optional and unused); an
    /// argument that is present but not a string fails closed to opaque.
    pub fn resolve(&self, arguments: &serde_json::Value, provider: &str) -> Footprint {
        let mut footprint = Footprint::new();
        for selector in &self.selectors {
            match selector {
                ResourceSelector::Literal { resource, access } => {
                    footprint = add(footprint, resource.clone(), *access);
                }
                ResourceSelector::PathArgument { field, access } => {
                    match string_argument(arguments, field) {
                        FieldValue::Present(path) => {
                            footprint = add(footprint, Resource::File(path), *access);
                        }
                        FieldValue::Absent => {}
                        FieldValue::NotAString => return opaque_footprint(),
                    }
                }
                ResourceSelector::ParentManifestOf { field, access } => {
                    match string_argument(arguments, field) {
                        FieldValue::Present(path) => {
                            let manifest = parent_manifest(&path);
                            footprint = add(footprint, Resource::Manifest(manifest), *access);
                        }
                        FieldValue::Absent => {}
                        FieldValue::NotAString => return opaque_footprint(),
                    }
                }
                ResourceSelector::ProviderRateLimit => {
                    footprint = add(
                        footprint,
                        Resource::ProviderRateLimit(provider.into()),
                        AccessMode::Read,
                    );
                }
                ResourceSelector::OpaqueWorkspace => return opaque_footprint(),
            }
        }
        footprint
    }
}

enum FieldValue {
    Present(String),
    Absent,
    NotAString,
}

fn string_argument(arguments: &serde_json::Value, field: &str) -> FieldValue {
    match arguments.get(field) {
        None => FieldValue::Absent,
        Some(value) => match value.as_str() {
            Some(s) => FieldValue::Present(s.to_string()),
            None => FieldValue::NotAString,
        },
    }
}

fn add(footprint: Footprint, resource: Resource, access: AccessMode) -> Footprint {
    match access {
        AccessMode::Read => footprint.read(resource),
        // The state-witness rule in declarative form: a write also reads.
        AccessMode::Write => footprint.read(resource.clone()).write(resource),
    }
}

fn opaque_footprint() -> Footprint {
    Footprint::new()
        .read(Resource::OpaqueWorkspace)
        .write(Resource::OpaqueWorkspace)
}

/// The directory-level manifest a path belongs to (simple heuristic: the
/// nearest ancestor directory, as a manifest key).
fn parent_manifest(path: &str) -> String {
    match path.rsplit_once('/') {
        Some((dir, _)) => dir.to_string(),
        None => ".".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn schema() -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {"path": {"type": "string"}},
            "required": ["path"],
        })
    }

    #[test]
    fn writes_are_contained_in_reads_by_construction() {
        let spec = FootprintSpec::new(vec![ResourceSelector::PathArgument {
            field: "path".into(),
            access: AccessMode::Write,
        }]);
        let footprint = spec.resolve(&serde_json::json!({"path": "src/lib.rs"}), "p");
        assert!(footprint.writes.iter().all(|w| footprint.reads.contains(w)));
        assert!(footprint
            .writes
            .contains(&Resource::File("src/lib.rs".into())));
    }

    #[test]
    fn validation_rejects_a_selector_for_an_undeclared_argument() {
        let spec = FootprintSpec::new(vec![ResourceSelector::PathArgument {
            field: "nonexistent".into(),
            access: AccessMode::Read,
        }]);
        assert!(spec.validate(&schema(), false).is_err());
    }

    #[test]
    fn a_mutating_tool_may_not_declare_an_empty_footprint() {
        assert!(FootprintSpec::default().validate(&schema(), true).is_err());
        assert!(FootprintSpec::opaque().validate(&schema(), true).is_ok());
    }

    #[test]
    fn a_non_string_path_argument_fails_closed_to_opaque() {
        let spec = FootprintSpec::new(vec![ResourceSelector::PathArgument {
            field: "path".into(),
            access: AccessMode::Write,
        }]);
        let footprint = spec.resolve(&serde_json::json!({"path": ["not", "a", "string"]}), "p");
        assert!(footprint.writes.contains(&Resource::OpaqueWorkspace));
    }

    #[test]
    fn rate_limit_is_a_read_never_a_serializing_write() {
        let spec = FootprintSpec::new(vec![ResourceSelector::ProviderRateLimit]);
        let footprint = spec.resolve(&serde_json::json!({}), "anthropic");
        assert!(footprint.writes.is_empty());
        assert!(footprint
            .reads
            .contains(&Resource::ProviderRateLimit("anthropic".into())));
    }

    #[test]
    fn two_rate_limited_turns_commute() {
        let spec = FootprintSpec::new(vec![ResourceSelector::ProviderRateLimit]);
        let a = spec.resolve(&serde_json::json!({}), "anthropic");
        let b = spec.resolve(&serde_json::json!({}), "anthropic");
        assert!(
            a.commutes_with(&b),
            "a rate-limit delay is not a serialization"
        );
    }
}
