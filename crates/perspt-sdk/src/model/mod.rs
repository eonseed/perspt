//! Model plane identity and contract types (PSP-9 systems 1–2, Layer A).
//!
//! Everything here is pure data: no transport, no vendor types, no I/O. The
//! transport *driver* lives in `perspt-core`; the adapter joining the two is
//! `perspt-agent::transport` — the only place both worlds are in scope. That
//! layering is what makes Gate S (provider portability) structural rather
//! than aspirational.

mod capabilities;
mod family;
mod id;

pub use capabilities::{CapabilityDegradation, ProviderCapabilities, ProviderCapabilityMask};
pub use family::ModelFamily;
pub use id::ModelId;
