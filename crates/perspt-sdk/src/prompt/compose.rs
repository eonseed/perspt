//! Stage composition (PSP-10 system 24).
//!
//! A stage composer is ordinary typed Rust: inclusion predicates are code
//! (`section_if`), variables are typed struct fields, and every section
//! renders eagerly, so object safety never arises. Compilation fits the
//! dialect's token budget deterministically and lays the surviving
//! sections out per the dialect's system-slot policy.

use crate::error::{Result, SdkError};

use super::budget::fit_budget;
use super::dialect::{ModelDialect, SystemSlotPolicy};
use super::digest::{CompiledPromptMessage, CompiledPromptProgram};
use super::route::PromptRoute;
use super::section::{
    OverrideOrigin, PromptMessageRole, PromptSection, RenderedSection, SectionProvenance,
};
use super::stage::PromptStage;

/// An in-progress composition for one stage.
#[derive(Debug)]
pub struct StageComposition {
    stage: PromptStage,
    sections: Vec<(RenderedSection, OverrideOrigin)>,
    error: Option<SdkError>,
    system_separator: Option<String>,
}

impl StageComposition {
    pub fn new(stage: PromptStage) -> Self {
        Self {
            stage,
            sections: Vec::new(),
            error: None,
            system_separator: None,
        }
    }

    /// Use the stage's declared section separator when the dialect
    /// concatenates system sections into one slot (the migrated platform
    /// literals' byte identity depends on it). Without this, the dialect's
    /// boundary marker joins.
    pub fn system_separator(mut self, separator: &str) -> Self {
        self.system_separator = Some(separator.to_string());
        self
    }

    /// Append a typed base section, rendering it eagerly.
    pub fn section(mut self, section: impl PromptSection) -> Self {
        if self.error.is_some() {
            return self;
        }
        match section.render() {
            Ok(rendered) => self.sections.push((rendered, OverrideOrigin::Base)),
            Err(error) => self.error = Some(error),
        }
        self
    }

    /// Typed inclusion predicate: append only when `include` holds.
    pub fn section_if(self, include: bool, section: impl PromptSection) -> Self {
        if include {
            self.section(section)
        } else {
            self
        }
    }

    /// Append an already-resolved section (the template-resolution path;
    /// carries its override origin).
    pub fn resolved(mut self, rendered: RenderedSection, origin: OverrideOrigin) -> Self {
        if self.error.is_none() {
            self.sections.push((rendered, origin));
        }
        self
    }

    /// Compile: deterministic budget fit, dialect layout, digests.
    pub fn compile(
        self,
        route: &PromptRoute,
        dialect: &ModelDialect,
        tool_spec_hash: &str,
    ) -> Result<CompiledPromptProgram> {
        if let Some(error) = self.error {
            return Err(error);
        }
        let token_budget = dialect.context_window_tokens;
        let mut origins: std::collections::BTreeMap<String, OverrideOrigin> =
            std::collections::BTreeMap::new();
        let mut rendered = Vec::with_capacity(self.sections.len());
        for (section, origin) in self.sections {
            origins.insert(section.id.0.clone(), origin);
            rendered.push(section);
        }
        let fit = fit_budget(rendered, &dialect.token_accountant, token_budget)?;
        let separator = self
            .system_separator
            .as_deref()
            .unwrap_or(&dialect.boundary_marker);
        let messages = layout_messages(&fit.kept, dialect, separator, &origins);
        let total_bytes: usize = messages.iter().map(|message| message.content.len()).sum();
        if total_bytes as u64 > dialect.max_prompt_bytes {
            return Err(SdkError::Domain(format!(
                "compiled program of {total_bytes} bytes exceeds the dialect's \
                 {}-byte limit",
                dialect.max_prompt_bytes
            )));
        }
        let program_digest = CompiledPromptProgram::compute_digest(
            route,
            self.stage,
            &dialect.dialect_ref(),
            &dialect.token_accountant.id,
            token_budget,
            &messages,
            &fit.dropped,
            tool_spec_hash,
        );
        Ok(CompiledPromptProgram {
            route: route.clone(),
            stage: self.stage,
            dialect: dialect.dialect_ref(),
            messages,
            dropped_sections: fit.dropped,
            tool_spec_hash: tool_spec_hash.to_string(),
            program_digest,
        })
    }
}

fn provenance(
    section: &RenderedSection,
    origins: &std::collections::BTreeMap<String, OverrideOrigin>,
) -> SectionProvenance {
    SectionProvenance {
        id: section.id.clone(),
        version: section.version,
        content_hash: section.content_hash.clone(),
        origin: origins
            .get(&section.id.0)
            .cloned()
            .unwrap_or(OverrideOrigin::Base),
    }
}

/// Lay the surviving sections out per the dialect: separate system messages
/// under [`SystemSlotPolicy::Many`], or one deterministic concatenation with
/// the boundary marker under [`SystemSlotPolicy::SingleConcatenated`].
/// User-role sections are always separate messages.
fn layout_messages(
    kept: &[RenderedSection],
    dialect: &ModelDialect,
    separator: &str,
    origins: &std::collections::BTreeMap<String, OverrideOrigin>,
) -> Vec<CompiledPromptMessage> {
    let mut messages = Vec::new();
    let system: Vec<&RenderedSection> = kept
        .iter()
        .filter(|section| section.role == PromptMessageRole::System)
        .collect();
    match dialect.system_slots {
        SystemSlotPolicy::Many => {
            for section in &system {
                messages.push(message(
                    PromptMessageRole::System,
                    section.content.clone(),
                    vec![provenance(section, origins)],
                ));
            }
        }
        SystemSlotPolicy::SingleConcatenated => {
            if !system.is_empty() {
                let content = system
                    .iter()
                    .map(|section| section.content.as_str())
                    .collect::<Vec<_>>()
                    .join(separator);
                let sections = system
                    .iter()
                    .map(|section| provenance(section, origins))
                    .collect();
                messages.push(message(PromptMessageRole::System, content, sections));
            }
        }
    }
    for section in kept
        .iter()
        .filter(|section| section.role == PromptMessageRole::User)
    {
        messages.push(message(
            PromptMessageRole::User,
            section.content.clone(),
            vec![provenance(section, origins)],
        ));
    }
    messages
}

fn message(
    role: PromptMessageRole,
    content: String,
    sections: Vec<SectionProvenance>,
) -> CompiledPromptMessage {
    let rendered_hash = CompiledPromptMessage::hash_content(&content);
    CompiledPromptMessage {
        role,
        content,
        sections,
        rendered_hash,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ModelFamily;
    use crate::prompt::section::{PromptSectionId, PromptSectionVersion};

    fn rendered(id: &str, role: PromptMessageRole, body: &str) -> RenderedSection {
        RenderedSection {
            id: PromptSectionId(id.into()),
            version: PromptSectionVersion(1),
            role,
            required: true,
            priority: 0,
            content_hash: format!("sha256:{id}"),
            content: body.into(),
        }
    }

    fn route() -> PromptRoute {
        PromptRoute {
            adapter: "scripted".into(),
            family: ModelFamily::Other("scripted".into()),
            exact_model: None,
        }
    }

    #[test]
    fn the_two_dialects_lay_out_the_same_program_differently() {
        let build = || {
            StageComposition::new(PromptStage::BranchCorrect)
                .resolved(
                    rendered("branch_correct/role", PromptMessageRole::System, "You fix."),
                    OverrideOrigin::Base,
                )
                .resolved(
                    rendered(
                        "branch_correct/output_contract",
                        PromptMessageRole::System,
                        "Reply with JSON.",
                    ),
                    OverrideOrigin::Base,
                )
        };
        let many = build()
            .compile(&route(), &ModelDialect::native_tools_v1(), "sha256:tools")
            .unwrap();
        let single = build()
            .compile(
                &route(),
                &ModelDialect::json_single_slot_v1(),
                "sha256:tools",
            )
            .unwrap();
        assert_eq!(many.messages.len(), 2);
        assert_eq!(single.messages.len(), 1);
        assert!(single.messages[0].content.contains("perspt:boundary"));
        assert_ne!(many.program_digest, single.program_digest);
    }

    #[test]
    fn identical_inputs_yield_identical_digests() {
        let build = || {
            StageComposition::new(PromptStage::BranchCorrect).resolved(
                rendered("branch_correct/role", PromptMessageRole::System, "You fix."),
                OverrideOrigin::Base,
            )
        };
        let dialect = ModelDialect::native_tools_v1();
        let first = build().compile(&route(), &dialect, "sha256:tools").unwrap();
        let second = build().compile(&route(), &dialect, "sha256:tools").unwrap();
        assert_eq!(first.program_digest, second.program_digest);
        assert_eq!(first.messages, second.messages);
        assert_eq!(first.dropped_sections, second.dropped_sections);
    }

    #[test]
    fn oversize_programs_fail_the_dialect_byte_limit() {
        let mut dialect = ModelDialect::native_tools_v1();
        dialect.max_prompt_bytes = 4;
        let composition = StageComposition::new(PromptStage::BranchCorrect).resolved(
            rendered("branch_correct/role", PromptMessageRole::System, "too long"),
            OverrideOrigin::Base,
        );
        assert!(composition
            .compile(&route(), &dialect, "sha256:tools")
            .is_err());
    }
}
