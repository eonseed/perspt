//! Structured diagnostics and their conversion to residual evidence
//! (PSP-10 system 26).
//!
//! Language adapters preserve as much source structure as each tool
//! provides; `EvidencePayload::raw` and `::structured` carry the actual
//! evidence — the historic summary-only, score-1.0 residuals are gone.

use perspt_sdk::{
    EvidencePayload, ResidualClass, ResidualEvent, ResidualSeverity, SensorRef,
    StructuredDiagnosticRef,
};

/// One diagnostic normalized from a tool's native output.
#[derive(Debug, Clone, PartialEq)]
pub struct StructuredDiagnostic {
    pub class: ResidualClass,
    pub code: Option<String>,
    pub message: String,
    pub path: Option<String>,
    pub line: Option<u32>,
    pub column: Option<u32>,
    /// Whether the tool marks this as a primary diagnostic (cascade
    /// children are secondary).
    pub primary: bool,
    /// Applicable fix suggestion, when the tool provides one.
    pub suggestion: Option<String>,
}

impl StructuredDiagnostic {
    pub fn to_ref(&self) -> StructuredDiagnosticRef {
        StructuredDiagnosticRef {
            code: self.code.clone(),
            message: self.message.clone(),
            path: self.path.clone(),
            line: self.line,
            column: self.column,
        }
    }
}

/// Build one cluster-level residual event carrying full evidence.
#[allow(clippy::too_many_arguments)]
pub fn cluster_residual(
    node_id: &str,
    generation: u32,
    class: ResidualClass,
    magnitude: f64,
    sensor: SensorRef,
    summary: &str,
    raw: &str,
    members: &[StructuredDiagnostic],
) -> anyhow::Result<ResidualEvent> {
    let mut event = ResidualEvent::new(
        node_id,
        generation,
        class,
        ResidualSeverity::Error,
        magnitude,
        sensor,
    )
    .map_err(|e| anyhow::anyhow!("{e}"))?;
    event.evidence = EvidencePayload {
        summary: summary.to_string(),
        raw: Some(bounded(raw, 16 * 1024)),
        structured: Some(serde_json::to_value(
            members
                .iter()
                .map(StructuredDiagnostic::to_ref)
                .collect::<Vec<_>>(),
        )?),
    };
    event.affected_paths = members
        .iter()
        .filter_map(|member| member.path.clone())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    Ok(event)
}

fn bounded(raw: &str, cap: usize) -> String {
    if raw.len() <= cap {
        return raw.to_string();
    }
    let mut end = cap;
    while !raw.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}\n[truncated {} bytes]", &raw[..end], raw.len() - end)
}
