//! Compiled prompt programs and their digests (PSP-10 system 23).
//!
//! Digests use the length-prefixed canonical encoding under the
//! `perspt-prompt-v1` domain tag. Serde output is never treated as
//! canonical. Identical inputs — dialect id and version, ordered resolved
//! sections with ids, versions, and hashes, inclusion outcomes, canonical
//! variable values (already baked into rendered bytes), token budget —
//! yield identical rendered bytes, an identical dropped-section set, and
//! identical digests.

use serde::{Deserialize, Serialize};

use crate::canon::CanonicalEncoder;

use super::dialect::DialectRef;
use super::route::PromptRoute;
use super::section::{PromptMessageRole, PromptSectionId, SectionProvenance, PROMPT_DIGEST_TAG};
use super::stage::PromptStage;

/// One transport-facing message compiled from sections.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompiledPromptMessage {
    pub role: PromptMessageRole,
    pub content: String,
    /// The sections this message renders, in order.
    pub sections: Vec<SectionProvenance>,
    /// SHA-256 of the rendered content under the prompt digest tag.
    pub rendered_hash: String,
}

impl CompiledPromptMessage {
    pub fn hash_content(content: &str) -> String {
        let mut encoder = CanonicalEncoder::new(PROMPT_DIGEST_TAG);
        encoder.text("rendered-message").text(content);
        encoder.digest()
    }
}

/// One compiled program: the platform envelope or the domain stage.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompiledPromptProgram {
    pub route: PromptRoute,
    pub stage: PromptStage,
    pub dialect: DialectRef,
    pub messages: Vec<CompiledPromptMessage>,
    /// Sections dropped by deterministic budget fitting, in drop order.
    pub dropped_sections: Vec<PromptSectionId>,
    pub tool_spec_hash: String,
    pub program_digest: String,
}

impl CompiledPromptProgram {
    /// The program digest over canonical bytes of the identity-bearing
    /// fields. Rendered content participates through each message's
    /// rendered hash.
    #[allow(clippy::too_many_arguments)]
    pub fn compute_digest(
        route: &PromptRoute,
        stage: PromptStage,
        dialect: &DialectRef,
        accountant_id: &str,
        token_budget: u64,
        messages: &[CompiledPromptMessage],
        dropped: &[PromptSectionId],
        tool_spec_hash: &str,
    ) -> String {
        let mut encoder = CanonicalEncoder::new(PROMPT_DIGEST_TAG);
        encoder
            .text("program")
            .text(&route.adapter)
            .text(&format!("{:?}", route.family))
            .text(route.exact_model.as_deref().unwrap_or(""))
            .text(stage.dir_name())
            .text(&dialect.id)
            .u64(u64::from(dialect.version))
            .text(accountant_id)
            .u64(token_budget);
        for message in messages {
            encoder.text(match message.role {
                PromptMessageRole::System => "system",
                PromptMessageRole::User => "user",
            });
            encoder.text(&message.rendered_hash);
            for section in &message.sections {
                encoder
                    .text(&section.id.0)
                    .u64(u64::from(section.version.0))
                    .text(&section.content_hash)
                    .text(&format!("{:?}", section.origin));
            }
        }
        encoder.list(dropped.iter().map(|id| id.0.as_str()));
        encoder.text(tool_spec_hash);
        encoder.digest()
    }
}

impl CompiledPromptProgram {
    /// The transport-facing system text: system-role messages in layout
    /// order. Under a single-slot dialect this is exactly the one
    /// concatenated slot.
    pub fn system_text(&self) -> String {
        self.messages
            .iter()
            .filter(|message| message.role == PromptMessageRole::System)
            .map(|message| message.content.as_str())
            .collect::<Vec<_>>()
            .join("\n\n")
    }
}

/// The content hash of the exact tool surface offered on one model call:
/// name, description, and schema of every spec, in offered order. Enters
/// the compiled program digest, so a discovery-activated tool visibly
/// changes the program identity.
pub fn tool_surface_hash(specs: &[crate::model::ToolSpec]) -> String {
    let mut encoder = CanonicalEncoder::new(PROMPT_DIGEST_TAG);
    encoder.text("tool-surface");
    for spec in specs {
        encoder
            .text(&spec.name)
            .text(&spec.description)
            .text(&spec.schema.to_string())
            .bool(spec.strict);
    }
    encoder.digest()
}

/// The two programs of one model call, hashed together in order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompiledPromptInvocation {
    pub platform: CompiledPromptProgram,
    pub domain: CompiledPromptProgram,
    pub invocation_digest: String,
}

impl CompiledPromptInvocation {
    pub fn new(platform: CompiledPromptProgram, domain: CompiledPromptProgram) -> Self {
        let mut encoder = CanonicalEncoder::new(PROMPT_DIGEST_TAG);
        encoder
            .text("invocation")
            .text(&platform.program_digest)
            .text(&domain.program_digest);
        let invocation_digest = encoder.digest();
        Self {
            platform,
            domain,
            invocation_digest,
        }
    }
}
