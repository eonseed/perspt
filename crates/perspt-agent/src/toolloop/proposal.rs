//! Canonicalizing provider tool calls into kernel-facing effect proposals
//! (PSP-9 system 12), plus the shared mutating-effect predicate and the
//! capability debit applied when a certified effect commits.

use anyhow::{Context, Result};
use perspt_sdk::{
    promote, CandidateStateWitness, Capability, EffectProposal, FullAdmissibilityWitness,
    ProviderToolCall, StateWitness, ToolEntry,
};

pub(crate) fn proposal_scope(call: &ProviderToolCall, entry: &ToolEntry) -> Vec<String> {
    if entry.proposal_bindings.is_empty() {
        return ["path", "to", "from"]
            .iter()
            .filter_map(|field| call.arguments.get(*field).and_then(|v| v.as_str()))
            .map(str::to_string)
            .collect();
    }
    let mut scope = Vec::new();
    for binding in &entry.proposal_bindings {
        match binding {
            perspt_sdk::ProposalBinding::Path { field } => {
                if let Some(path) = call.arguments.get(field).and_then(|v| v.as_str()) {
                    scope.push(path.to_string());
                }
            }
            perspt_sdk::ProposalBinding::MultiValue { field, target }
                if *target == perspt_sdk::MultiValueTarget::Path =>
            {
                scope.extend(string_array(call, field));
            }
            _ => {}
        }
    }
    scope
}

/// The string elements of a schema-validated scalar array argument.
fn string_array(call: &ProviderToolCall, field: &str) -> Vec<String> {
    call.arguments
        .get(field)
        .and_then(|v| v.as_array())
        .map(|values| {
            values
                .iter()
                .filter_map(|v| v.as_str())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// The one definition of "this effect mutates the candidate overlay", shared
/// by the loop's accounting and the executor's journal so they cannot drift.
pub fn candidate_mutating_effect(effect: perspt_sdk::EffectKind) -> bool {
    matches!(
        effect,
        perspt_sdk::EffectKind::WriteArtifact
            | perspt_sdk::EffectKind::ApplyPatch
            | perspt_sdk::EffectKind::MoveFile
            | perspt_sdk::EffectKind::DeleteFile
            | perspt_sdk::EffectKind::MutateDependencies
    )
}

/// `Some(reason)` when a witness does not certify autonomous commitment.
pub(crate) fn uncertified_reason(witness: &FullAdmissibilityWitness) -> Option<String> {
    if witness.allows() && witness.profile == perspt_sdk::AdmissibilityProfile::SrbnCertified {
        return None;
    }
    Some(if witness.allows() {
        format!(
            "admissibility profile {:?} is not autonomously committable",
            witness.profile
        )
    } else {
        format!("{:?}", witness.base.decision)
    })
}

pub(crate) fn promote_matching_capability(
    capabilities: &mut [Capability],
    witness: &FullAdmissibilityWitness,
) -> Result<()> {
    let capability_id = witness.base.capability_id.as_ref();
    let capability = capabilities
        .iter_mut()
        .find(|capability| Some(&capability.capability_id) == capability_id)
        .context("admissibility witness references a missing capability")?;
    let mut promoted = capability.clone();
    promote(&mut promoted, witness).map_err(|error| anyhow::anyhow!(error.to_string()))?;
    *capability = promoted;
    Ok(())
}

/// Canonicalize one provider call into an effect proposal (PSP-9 system 12).
/// Provenance (which model proposed it) is recorded in the ledger, never in
/// the kernel's input, so no admissibility decision can depend on a vendor.
pub(crate) fn proposal_from(
    call: &ProviderToolCall,
    entry: &ToolEntry,
    node_id: &str,
    generation: u32,
    before: &CandidateStateWitness,
) -> EffectProposal {
    let mut proposal =
        EffectProposal::new(perspt_sdk::ActorId::new("toolloop"), node_id, entry.effect)
            .with_generation(generation)
            .with_risk_class(entry.risk)
            .with_idempotency_key(format!("{}:{}", call.name, call.arguments))
            .with_preconditions(vec![StateWitness {
                resource: "__candidate_root".into(),
                content_hash: before.state_root.clone(),
            }]);
    if entry.proposal_bindings.is_empty() {
        // Builtins: conventional field names.
        if let Some(path) = call.arguments.get("path").and_then(|v| v.as_str()) {
            proposal = proposal.with_path(path);
        }
        if let Some(command) = call.arguments.get("command").and_then(|v| v.as_str()) {
            proposal = proposal.with_command(perspt_sdk::canonicalize(command, "."));
        }
        if let Some(path) = call.arguments.get("to").and_then(|v| v.as_str()) {
            proposal = proposal.with_additional_paths(vec![path.to_string()]);
        }
        if let Some(url) = call.arguments.get("url").and_then(|v| v.as_str()) {
            proposal = proposal.with_network_target(url);
        }
        return proposal;
    }
    bind_declared(proposal, call, entry)
}

/// Bind a registered entry's declared proposal channels; the loop holds no
/// per-tool field knowledge.
fn bind_declared(
    mut proposal: EffectProposal,
    call: &ProviderToolCall,
    entry: &ToolEntry,
) -> EffectProposal {
    let mut primary_path_bound = false;
    for binding in &entry.proposal_bindings {
        match binding {
            perspt_sdk::ProposalBinding::Path { field } => {
                if let Some(path) = call.arguments.get(field).and_then(|v| v.as_str()) {
                    if primary_path_bound {
                        proposal = proposal.with_additional_paths(vec![path.to_string()]);
                    } else {
                        proposal = proposal.with_path(path);
                        primary_path_bound = true;
                    }
                }
            }
            perspt_sdk::ProposalBinding::Command { field } => {
                if let Some(command) = call.arguments.get(field).and_then(|v| v.as_str()) {
                    proposal = proposal.with_command(perspt_sdk::canonicalize(command, "."));
                }
            }
            perspt_sdk::ProposalBinding::Url { field } => {
                if let Some(url) = call.arguments.get(field).and_then(|v| v.as_str()) {
                    proposal = proposal.with_network_target(url);
                }
            }
            perspt_sdk::ProposalBinding::MultiValue { field, target } => {
                let values = string_array(call, field);
                match target {
                    perspt_sdk::MultiValueTarget::Path => {
                        proposal = proposal.with_additional_paths(values);
                    }
                    perspt_sdk::MultiValueTarget::Command => {
                        for value in values {
                            proposal = proposal.with_command(perspt_sdk::canonicalize(&value, "."));
                        }
                    }
                    perspt_sdk::MultiValueTarget::Url => {
                        for value in values {
                            proposal = proposal.with_network_target(&value);
                        }
                    }
                }
            }
        }
    }
    proposal
}
