//! External tool providers under the kernel (PSP-9 system 13).
//!
//! A platform must accept tools it did not write — and doing that *after*
//! the kernel exists is the difference between an extension point and a
//! hole. Three properties external tools break by default in naive
//! implementations are enforced here:
//!
//! * **Registration never mints authority.** The session must already hold a
//!   capability for the declared effect; an external server can never exceed
//!   the user's own grant (Theorem 1, and Theorem 2 for a server that
//!   rewrites its own manifest).
//! * **An undeclared footprint fails closed**: the tool is classified as
//!   `RunShell` with an opaque footprint, so it serializes against
//!   everything and requires an explicit capability.
//! * Tool descriptions are untrusted text; they enter conversations as
//!   observations and cannot alter capabilities, policy, or the catalog's
//!   effect mapping — the caller records them, this module only validates.

use super::entry::{ToolEntry, ToolOrigin};
use super::footprint::FootprintSpec;
use crate::capability::{Capability, EffectKind, RiskClass};
use crate::error::{Result, SdkError};

/// A tool declaration received from an external server, before admission.
#[derive(Debug, Clone, PartialEq)]
pub struct ExternalToolDeclaration {
    pub name: String,
    pub description: String,
    pub effect: EffectKind,
    pub risk: RiskClass,
    pub schema: serde_json::Value,
    /// `None` means the server declared nothing: fail closed.
    pub footprint: Option<FootprintSpec>,
}

/// Admit one external declaration into catalog entries the kernel governs.
///
/// The session's held capabilities bound what may be registered: a
/// declaration whose effect no capability grants is rejected outright —
/// there is no "register now, ask later" path.
pub fn admit_external_tool(
    server_id: &str,
    declaration: ExternalToolDeclaration,
    session_capabilities: &[Capability],
) -> Result<ToolEntry> {
    // Fail closed on an undeclared footprint: RunShell + opaque.
    let (effect, risk, footprint) = match declaration.footprint {
        Some(footprint) => (declaration.effect, declaration.risk, footprint),
        None => (
            EffectKind::RunShell,
            RiskClass::High,
            FootprintSpec::opaque(),
        ),
    };

    let granted = session_capabilities
        .iter()
        .any(|c| c.effects.contains(&effect));
    if !granted {
        return Err(SdkError::Domain(format!(
            "external server {server_id:?} declared tool {:?} with effect {effect:?}, \
             which no session capability grants; registration never mints authority",
            declaration.name
        )));
    }

    let entry = ToolEntry {
        name: declaration.name,
        description: declaration.description,
        discovery_summary: String::new(),
        description_templates: None,
        effect,
        risk,
        schema: declaration.schema,
        footprint,
        proposal_bindings: Vec::new(),
        // External calls are external effects: bracketed in the
        // write-ahead external-effect log (R5).
        durable: true,
        origin: ToolOrigin::External(server_id.to_string()),
    };
    entry.validate()?;
    Ok(entry)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability::ActorId;
    use crate::scheduler::Resource;
    use crate::toolset::footprint::{AccessMode, ResourceSelector};

    fn declaration(
        effect: EffectKind,
        footprint: Option<FootprintSpec>,
    ) -> ExternalToolDeclaration {
        ExternalToolDeclaration {
            name: "company_search".into(),
            description: "Search the internal index".into(),
            effect,
            risk: RiskClass::Low,
            schema: serde_json::json!({
                "type": "object",
                "properties": {"query": {"type": "string"}},
                "required": ["query"],
            }),
            footprint,
        }
    }

    fn session_with(effects: Vec<EffectKind>) -> Vec<Capability> {
        vec![Capability::new(ActorId::new("session"), effects)]
    }

    #[test]
    fn a_declared_tool_within_session_authority_registers() {
        let entry = admit_external_tool(
            "mcp-1",
            declaration(EffectKind::Search, Some(FootprintSpec::default())),
            &session_with(vec![EffectKind::Search]),
        )
        .unwrap();
        assert_eq!(entry.origin, ToolOrigin::External("mcp-1".into()));
        assert_eq!(entry.effect, EffectKind::Search);
    }

    #[test]
    fn registration_never_exceeds_session_authority() {
        // The server declares a network effect the session does not hold.
        let result = admit_external_tool(
            "mcp-1",
            declaration(EffectKind::NetworkFetch, Some(FootprintSpec::opaque())),
            &session_with(vec![EffectKind::Search]),
        );
        assert!(result.is_err(), "an external server cannot widen authority");
    }

    #[test]
    fn an_undeclared_footprint_classifies_as_run_shell_and_opaque() {
        // Even though the server *claims* Search, no footprint means the
        // declaration fails closed to RunShell — which this session does not
        // hold, so it is rejected.
        let rejected = admit_external_tool(
            "mcp-1",
            declaration(EffectKind::Search, None),
            &session_with(vec![EffectKind::Search]),
        );
        assert!(rejected.is_err());

        // A session explicitly holding RunShell may register it, and the
        // footprint serializes against everything.
        let entry = admit_external_tool(
            "mcp-1",
            declaration(EffectKind::Search, None),
            &session_with(vec![EffectKind::RunShell]),
        )
        .unwrap();
        assert_eq!(entry.effect, EffectKind::RunShell);
        let resolved = entry.footprint.resolve(&serde_json::json!({}), "p");
        assert!(resolved.writes.contains(&Resource::OpaqueWorkspace));
    }

    #[test]
    fn an_invalid_declared_footprint_is_rejected_at_admission() {
        let bad_footprint = FootprintSpec::new(vec![ResourceSelector::PathArgument {
            field: "undeclared_arg".into(),
            access: AccessMode::Write,
        }]);
        let result = admit_external_tool(
            "mcp-1",
            declaration(EffectKind::Search, Some(bad_footprint)),
            &session_with(vec![EffectKind::Search]),
        );
        assert!(result.is_err());
    }
}
