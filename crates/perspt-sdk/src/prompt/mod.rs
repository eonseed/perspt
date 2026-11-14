//! The model-conditioned prompt plane (PSP-10 systems 23–25).
//!
//! Prompt text is authored as small markdown section files with typed front
//! matter, compiled at build time (`perspt-prompt-macros`) into typed
//! structs. There is no template engine and no prompt database: dynamism is
//! typed inclusion predicates, typed variables, and injected turn blocks;
//! git reviews the text and the ledger records the exact program of every
//! model call.

pub mod accountant;
pub mod budget;
pub mod compose;
pub mod context;
pub mod dialect;
pub mod digest;
pub mod manifest;
pub mod plan;
pub mod route;
pub mod section;
pub mod stage;
pub mod vars;

pub use accountant::{AccountingMode, TokenAccountantRef};
pub use budget::{BudgetFit, fit_budget};
pub use compose::StageComposition;
pub use context::{
    ContextBudget, ContextPage, DependencyEnv, ResidentContext, ResidentOutcome, StateDependency,
    assemble_resident, mandatory_closure, resident_page_digest, select_working_set,
};
pub use dialect::{
    DialectRef, ModelDialect, ReasoningTracePolicy, SystemSlotPolicy, ToolCallConvention,
};
pub use digest::{
    CompiledPromptInvocation, CompiledPromptMessage, CompiledPromptProgram, tool_surface_hash,
};
pub use manifest::{
    ACTIVATION_BOOTSTRAP_RESAMPLES, ActivationBounds, ActivationState, ManifestEntry,
    PromptChangeRecord, PromptManifest,
};
pub use plan::{ContextRequest, CounterexampleRef, IssuePromptPlan};
pub use route::{PromptRoute, SectionOverride, SectionVariants};
pub use section::{
    OverrideOrigin, PROMPT_DIGEST_TAG, PromptMessageRole, PromptSection, PromptSectionId,
    PromptSectionVersion, RenderedSection, SectionProvenance, SectionSchema, SectionTemplate,
    VarSpec,
};
pub use stage::PromptStage;
pub use vars::{BoundedList, BoundedText, ListStyle, ObservationText, VarValue};
