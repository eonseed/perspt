//! The typed tool catalog (PSP-9 system 5, Layer B).
//!
//! Catalog assembly is a three-way join, and each side keeps its existing
//! job: the SDK contributes the base entries and the effect/risk/footprint
//! contract; the active `AgentDomainPackage` contributes domain entries; and
//! the runtime's plugin registry binds verifier tools to actual commands for
//! this workspace. A workspace with no `cargo` on `PATH` simply does not
//! offer `run_build` — and says why.

mod base;
mod catalog;
mod entry;
mod external;
mod footprint;

pub use base::base_entries;
pub use catalog::{StaticCatalog, ToolCatalog};
pub use entry::{ToolEntry, ToolOrigin};
pub use external::{admit_external_tool, ExternalToolDeclaration};
pub use footprint::{AccessMode, FootprintSpec, ResourceSelector};
