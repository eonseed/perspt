//! The platform prompt section library (PSP-10 systems 23 and 25).
//!
//! The five formerly inline actor prompts live here as base sections,
//! compiled at build time from `prompts/` by `perspt-prompt-macros`. There
//! is exactly one composer: the SDK's `StageComposition` compiler, which
//! fits the dialect budget deterministically and digests the program.
//! Under a single-slot dialect the compiled system text of each migrated
//! stage is byte-identical to the pre-move literal
//! (`tests/prompt_matrix.rs` proves it). Construction is lazy: nothing
//! composes until an actor asks, so `simple-chat` links this crate without
//! ever building a program.

use perspt_sdk::error::Result;
use perspt_sdk::prompt::{
    CompiledPromptProgram, ManifestEntry, ModelDialect, PromptManifest, PromptMessageRole,
    PromptRoute, PromptSection, PromptSectionId, PromptSectionVersion, PromptStage,
    RenderedSection, SectionTemplate, StageComposition,
};

mod generated {
    include!(concat!(env!("OUT_DIR"), "/prompt_sections.rs"));
}

pub use generated::{
    adjudicate, evidence_summarize, graph_plan, repository_explore, session_bootstrap,
};

/// One platform stage's rendered sections, ready to compile for any route.
#[derive(Debug, Clone)]
pub struct PlatformStage {
    pub stage: PromptStage,
    /// The stage's declared section separator (identity-bearing for the
    /// migrated literals).
    pub separator: &'static str,
    pub sections: Vec<RenderedSection>,
}

impl PlatformStage {
    /// Compile this stage for one resolved route through the SDK compiler —
    /// the only composer. The stage separator joins system sections under a
    /// single-slot dialect, preserving the migrated literals' bytes.
    pub fn compile(
        &self,
        route: &PromptRoute,
        dialect: &ModelDialect,
        tool_spec_hash: &str,
    ) -> Result<CompiledPromptProgram> {
        let mut composition = StageComposition::new(self.stage).system_separator(self.separator);
        for section in &self.sections {
            composition =
                composition.resolved(section.clone(), perspt_sdk::prompt::OverrideOrigin::Base);
        }
        composition.compile(route, dialect, tool_spec_hash)
    }
}

/// Compile one call's two programs — the platform stage and the domain's
/// same-stage sections (possibly none) — into the invocation the ledger
/// records (spec: "Each model call compiles two programs").
pub fn compile_invocation(
    stage: &PlatformStage,
    domain_sections: &[RenderedSection],
    route: &PromptRoute,
    dialect: &ModelDialect,
    tool_spec_hash: &str,
) -> Result<perspt_sdk::prompt::CompiledPromptInvocation> {
    let platform = stage.compile(route, dialect, tool_spec_hash)?;
    let mut composition = StageComposition::new(stage.stage);
    for section in domain_sections {
        composition =
            composition.resolved(section.clone(), perspt_sdk::prompt::OverrideOrigin::Base);
    }
    let domain = composition.compile(route, dialect, tool_spec_hash)?;
    Ok(perspt_sdk::prompt::CompiledPromptInvocation::new(
        platform, domain,
    ))
}

/// The compiled platform envelope, one stage builder per actor.
#[derive(Debug)]
pub struct PlatformPromptLibrary;

impl PlatformPromptLibrary {
    /// The worker's session bootstrap. The domain is a variable — the old
    /// literal hardcoded "coding" for every domain.
    pub fn session_bootstrap(domain_id: &str) -> Result<PlatformStage> {
        let role = session_bootstrap::Role {
            domain_id: perspt_sdk::prompt::BoundedText::new(domain_id)?,
        };
        Ok(PlatformStage {
            stage: PromptStage::SessionBootstrap,
            separator: session_bootstrap::SEPARATOR,
            sections: vec![role.render()?, session_bootstrap::Governance {}.render()?],
        })
    }

    /// The architect's planning turn. The revision shape is rendered from
    /// the single source of truth beside `PlanSpec`, so the schema shown to
    /// the model cannot drift from the schema enforced on it.
    pub fn graph_plan(revision_shape: &str) -> Result<PlatformStage> {
        let protocol = graph_plan::UpdateProtocol {
            revision_shape: perspt_sdk::prompt::BoundedText::new(revision_shape)?,
        };
        Ok(PlatformStage {
            stage: PromptStage::GraphPlan,
            separator: graph_plan::SEPARATOR,
            sections: vec![graph_plan::Role {}.render()?, protocol.render()?],
        })
    }

    pub fn repository_explore() -> Result<PlatformStage> {
        Ok(PlatformStage {
            stage: PromptStage::RepositoryExplore,
            separator: repository_explore::SEPARATOR,
            sections: vec![
                repository_explore::Role {}.render()?,
                repository_explore::Protocol {}.render()?,
            ],
        })
    }

    pub fn adjudicate() -> Result<PlatformStage> {
        Ok(PlatformStage {
            stage: PromptStage::Adjudicate,
            separator: adjudicate::SEPARATOR,
            sections: vec![
                adjudicate::Role {}.render()?,
                adjudicate::Protocol {}.render()?,
            ],
        })
    }

    pub fn evidence_summarize() -> Result<PlatformStage> {
        Ok(PlatformStage {
            stage: PromptStage::EvidenceSummarize,
            separator: evidence_summarize::SEPARATOR,
            sections: vec![
                evidence_summarize::Role {}.render()?,
                evidence_summarize::Protocol {}.render()?,
            ],
        })
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
