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
    PromptRoute, PromptSectionId, PromptSectionVersion, PromptStage, RenderedSection,
    SectionTemplate, StageComposition,
};

mod generated {
    include!(concat!(env!("OUT_DIR"), "/prompt_sections.rs"));
}

pub use generated::{
    adjudicate, evidence_summarize, graph_plan, repository_explore, session_bootstrap,
};

/// Experimental live section overrides loaded from validated
/// `[prompts].bundles` (system 25). A replacement body substitutes only
/// for a known compiled **platform** section, only under exactly that
/// section's schema, and only when the operator opted in with
/// `--allow-experimental-prompts`; the composition root ledgers every
/// active override at session start. Gate AE keeps divergent templates
/// experimental until a `PromptChangeRecord` passes the paired evaluation.
#[derive(Debug, Clone, Default)]
pub struct SectionOverrides {
    map: std::collections::BTreeMap<String, perspt_sdk::prompt::SectionTemplate>,
}

impl SectionOverrides {
    /// Register one replacement body for a known platform section. The
    /// compiled schema is authoritative — only the body and its content
    /// hash change. Unknown or non-platform ids refuse loudly.
    pub fn insert_replacement(&mut self, id: &str, body: &str) -> Result<()> {
        let base = compiled_platform_templates()
            .into_iter()
            .map(|(_, template)| template)
            .find(|template| template.schema.id.0 == id)
            .ok_or_else(|| {
                perspt_sdk::error::SdkError::Domain(format!(
                    "section {id:?} is not a compiled platform section; live \
                     substitution covers the five platform stages only"
                ))
            })?;
        let template = perspt_sdk::prompt::SectionTemplate {
            schema: base.schema,
            content_hash: perspt_sdk::prompt::SectionTemplate::hash_body(body),
            body: body.to_string(),
        };
        self.map.insert(id.to_string(), template);
        Ok(())
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// `(section id, replacement content hash)` pairs for the ledger.
    pub fn provenance(&self) -> Vec<(String, String)> {
        self.map
            .iter()
            .map(|(id, template)| (id.clone(), template.content_hash.clone()))
            .collect()
    }

    /// Render one section: the override body under the compiled schema when
    /// one is loaded, the base template otherwise. An override body renders
    /// with exactly the base section's typed values and fails closed on any
    /// undeclared placeholder.
    fn rendered(
        &self,
        base: perspt_sdk::prompt::SectionTemplate,
        values: &std::collections::BTreeMap<String, perspt_sdk::prompt::VarValue>,
    ) -> Result<RenderedSection> {
        match self.map.get(base.schema.id.0.as_str()) {
            Some(replacement) => replacement.render(values),
            None => base.render(values),
        }
    }
}

/// Every compiled platform base template with its stage, in manifest order.
pub fn compiled_platform_templates() -> Vec<(&'static str, SectionTemplate)> {
    let stages: [(&'static str, Vec<SectionTemplate>); 5] = [
        ("adjudicate", adjudicate::templates()),
        ("evidence_summarize", evidence_summarize::templates()),
        ("graph_plan", graph_plan::templates()),
        ("repository_explore", repository_explore::templates()),
        ("session_bootstrap", session_bootstrap::templates()),
    ];
    let mut templates = Vec::new();
    for (stage, list) in stages {
        for template in list {
            templates.push((stage, template));
        }
    }
    templates
}

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
    pub fn session_bootstrap(
        domain_id: &str,
        overrides: &SectionOverrides,
    ) -> Result<PlatformStage> {
        let role = session_bootstrap::Role {
            domain_id: perspt_sdk::prompt::BoundedText::new(domain_id)?,
        };
        Ok(PlatformStage {
            stage: PromptStage::SessionBootstrap,
            separator: session_bootstrap::SEPARATOR,
            sections: vec![
                overrides.rendered(session_bootstrap::Role::template(), &role.values())?,
                overrides.rendered(
                    session_bootstrap::Governance::template(),
                    &session_bootstrap::Governance {}.values(),
                )?,
            ],
        })
    }

    /// The architect's planning turn. The revision shape is rendered from
    /// the single source of truth beside `PlanSpec`, so the schema shown to
    /// the model cannot drift from the schema enforced on it.
    pub fn graph_plan(revision_shape: &str, overrides: &SectionOverrides) -> Result<PlatformStage> {
        let protocol = graph_plan::UpdateProtocol {
            revision_shape: perspt_sdk::prompt::BoundedText::new(revision_shape)?,
        };
        Ok(PlatformStage {
            stage: PromptStage::GraphPlan,
            separator: graph_plan::SEPARATOR,
            sections: vec![
                overrides.rendered(graph_plan::Role::template(), &graph_plan::Role {}.values())?,
                overrides.rendered(graph_plan::UpdateProtocol::template(), &protocol.values())?,
            ],
        })
    }

    pub fn repository_explore(overrides: &SectionOverrides) -> Result<PlatformStage> {
        Ok(PlatformStage {
            stage: PromptStage::RepositoryExplore,
            separator: repository_explore::SEPARATOR,
            sections: vec![
                overrides.rendered(
                    repository_explore::Role::template(),
                    &repository_explore::Role {}.values(),
                )?,
                overrides.rendered(
                    repository_explore::Protocol::template(),
                    &repository_explore::Protocol {}.values(),
                )?,
            ],
        })
    }

    pub fn adjudicate(overrides: &SectionOverrides) -> Result<PlatformStage> {
        Ok(PlatformStage {
            stage: PromptStage::Adjudicate,
            separator: adjudicate::SEPARATOR,
            sections: vec![
                overrides.rendered(adjudicate::Role::template(), &adjudicate::Role {}.values())?,
                overrides.rendered(
                    adjudicate::Protocol::template(),
                    &adjudicate::Protocol {}.values(),
                )?,
            ],
        })
    }

    pub fn evidence_summarize(overrides: &SectionOverrides) -> Result<PlatformStage> {
        Ok(PlatformStage {
            stage: PromptStage::EvidenceSummarize,
            separator: evidence_summarize::SEPARATOR,
            sections: vec![
                overrides.rendered(
                    evidence_summarize::Role::template(),
                    &evidence_summarize::Role {}.values(),
                )?,
                overrides.rendered(
                    evidence_summarize::Protocol::template(),
                    &evidence_summarize::Protocol {}.values(),
                )?,
            ],
        })
    }

    /// The library manifest, for the session-provenance digest.
    pub fn manifest() -> PromptManifest {
        let mut entries = Vec::new();
        for (stage, template) in compiled_platform_templates() {
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
        PromptManifest { entries }
    }
}
