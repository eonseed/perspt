//! The platform prompt section library (PSP-10 systems 23 and 25).
//!
//! The five formerly inline actor prompts live here as base sections,
//! compiled at build time from `prompts/` by `perspt-prompt-macros`. The
//! composed rendering of each migrated stage is byte-identical to the
//! pre-move literal (`tests/prompt_matrix.rs` proves it). Construction is
//! lazy: nothing composes until an actor asks, so `simple-chat` links this
//! crate without ever building a program.

use perspt_sdk::error::Result;
use perspt_sdk::prompt::{
    ManifestEntry, PromptManifest, PromptMessageRole, PromptSection, PromptSectionId,
    PromptSectionVersion, RenderedSection, SectionProvenance, SectionTemplate,
};

mod generated {
    include!(concat!(env!("OUT_DIR"), "/prompt_sections.rs"));
}

pub use generated::{
    adjudicate, evidence_summarize, graph_plan, repository_explore, session_bootstrap,
};

/// One stage's composed text with its ledger-facing provenance.
#[derive(Debug, Clone, PartialEq)]
pub struct ComposedStageText {
    /// Rendered sections joined with the stage's declared separator.
    pub text: String,
    pub sections: Vec<SectionProvenance>,
    /// Content digest over the section identities and rendered text
    /// (`perspt-prompt-v1`); recorded per call, checked on resume.
    pub digest: String,
}

fn compose(separator: &str, rendered: Vec<RenderedSection>) -> ComposedStageText {
    let text = rendered
        .iter()
        .map(|section| section.content.as_str())
        .collect::<Vec<_>>()
        .join(separator);
    let sections: Vec<SectionProvenance> = rendered
        .iter()
        .map(|section| SectionProvenance {
            id: section.id.clone(),
            version: section.version,
            content_hash: section.content_hash.clone(),
            origin: perspt_sdk::prompt::OverrideOrigin::Base,
        })
        .collect();
    let mut encoder =
        perspt_sdk::canon::CanonicalEncoder::new(perspt_sdk::prompt::PROMPT_DIGEST_TAG);
    encoder.text("stage-text").text(separator);
    for section in &sections {
        encoder
            .text(&section.id.0)
            .u64(u64::from(section.version.0))
            .text(&section.content_hash);
    }
    encoder.text(&text);
    let digest = encoder.digest();
    ComposedStageText {
        text,
        sections,
        digest,
    }
}

/// The compiled platform envelope, one composer per stage.
#[derive(Debug)]
pub struct PlatformPromptLibrary;

impl PlatformPromptLibrary {
    /// The worker's session bootstrap. The domain is a variable — the old
    /// literal hardcoded "coding" for every domain.
    pub fn session_bootstrap(domain_id: &str) -> Result<ComposedStageText> {
        let role = session_bootstrap::Role {
            domain_id: perspt_sdk::prompt::BoundedText::new(domain_id)?,
        };
        Ok(compose(
            session_bootstrap::SEPARATOR,
            vec![role.render()?, session_bootstrap::Governance {}.render()?],
        ))
    }

    /// The architect's planning turn. The revision shape is rendered from
    /// the single source of truth beside `PlanSpec`, so the schema shown to
    /// the model cannot drift from the schema enforced on it.
    pub fn graph_plan(revision_shape: &str) -> Result<ComposedStageText> {
        let protocol = graph_plan::UpdateProtocol {
            revision_shape: perspt_sdk::prompt::BoundedText::new(revision_shape)?,
        };
        Ok(compose(
            graph_plan::SEPARATOR,
            vec![graph_plan::Role {}.render()?, protocol.render()?],
        ))
    }

    pub fn repository_explore() -> Result<ComposedStageText> {
        Ok(compose(
            repository_explore::SEPARATOR,
            vec![
                repository_explore::Role {}.render()?,
                repository_explore::Protocol {}.render()?,
            ],
        ))
    }

    pub fn adjudicate() -> Result<ComposedStageText> {
        Ok(compose(
            adjudicate::SEPARATOR,
            vec![
                adjudicate::Role {}.render()?,
                adjudicate::Protocol {}.render()?,
            ],
        ))
    }

    pub fn evidence_summarize() -> Result<ComposedStageText> {
        Ok(compose(
            evidence_summarize::SEPARATOR,
            vec![
                evidence_summarize::Role {}.render()?,
                evidence_summarize::Protocol {}.render()?,
            ],
        ))
    }

    /// The library manifest, for the session-provenance digest.
    pub fn manifest() -> PromptManifest {
        let stages: [(&str, Vec<SectionTemplate>); 5] = [
            ("adjudicate", adjudicate::templates()),
            ("evidence_summarize", evidence_summarize::templates()),
            ("graph_plan", graph_plan::templates()),
            ("repository_explore", repository_explore::templates()),
            ("session_bootstrap", session_bootstrap::templates()),
        ];
        let mut entries = Vec::new();
        for (stage, templates) in stages {
            for template in templates {
                entries.push(ManifestEntry {
                    id: PromptSectionId(template.schema.id.0.clone()),
                    version: PromptSectionVersion(template.schema.version.0),
                    stage: stage.to_string(),
                    role: match template.schema.role {
                        PromptMessageRole::System => "system".into(),
                        PromptMessageRole::User => "user".into(),
                    },
                    required: template.schema.required,
                    priority: template.schema.priority,
                    max_bytes: template.schema.max_bytes,
                    owner: "perspt-core".into(),
                    content_hash: template.content_hash.clone(),
                    activation: None,
                    change_record: None,
                });
            }
        }
        PromptManifest { entries }
    }
}
