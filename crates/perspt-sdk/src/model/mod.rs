//! Model plane identity and contract types (PSP-9 systems 1–2, Layer A).
//!
//! Everything here is pure data: no transport, no vendor types, no I/O. The
//! transport *driver* lives in `perspt-core`; the adapter joining the two is
//! `perspt-agent::transport` — the only place both worlds are in scope. That
//! layering is what makes Gate S (provider portability) structural rather
//! than aspirational.

mod capabilities;
mod conversation;
mod ensemble;
mod family;
mod id;
mod tool;
mod transport;

pub use capabilities::{CapabilityDegradation, ProviderCapabilities, ProviderCapabilityMask};
pub use conversation::{Conversation, Message};
pub use ensemble::{EnsemblePolicy, EnsembleTrigger};
pub use family::ModelFamily;
pub use id::ModelId;
pub use tool::{ProviderToolCall, ToolChoicePolicy, ToolSpec, TurnOutput};
pub use transport::{ModelTransport, TransportFuture};
