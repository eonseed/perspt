//! Root-cause clustering and profile-normalized magnitudes
//! (PSP-10 system 26).
//!
//! Cascaded errors are grouped by root cause before scoring: two hundred
//! cascade errors from one missing import are one cluster, not two hundred
//! units of energy. The magnitude formula is part of the immutable sensor
//! profile [`CLUSTER_PROFILE_V1`]; measurements under different profiles
//! are never pooled or compared.

use std::collections::BTreeMap;

use perspt_sdk::ResidualClass;

use super::types::StructuredDiagnostic;

/// The clustering + normalization profile identity. Changing the grouping
/// key or the magnitude formula requires a new version.
pub const CLUSTER_PROFILE_V1: &str = "perspt-cluster-v1:log-damped";

/// One root-cause cluster of diagnostics.
#[derive(Debug, Clone, PartialEq)]
pub struct DiagnosticCluster {
    pub class: ResidualClass,
    /// The grouping code (tool code, or the class name when the tool has
    /// none).
    pub code: String,
    /// The primary (first) diagnostic — the root cause the cascade folds
    /// into.
    pub root: StructuredDiagnostic,
    pub members: Vec<StructuredDiagnostic>,
}

impl DiagnosticCluster {
    /// Log-damped magnitude: `1 + ln(member_count)`. One diagnostic scores
    /// 1.0; two hundred cascades score ≈ 6.3 instead of 200.
    pub fn magnitude(&self) -> f64 {
        1.0 + (self.members.len() as f64).ln()
    }
}

/// Group diagnostics by `(class, code)`. Secondary (non-primary)
/// diagnostics with no code of their own fold into the preceding primary's
/// cluster — that is the cascade rule.
pub fn cluster(diagnostics: Vec<StructuredDiagnostic>) -> Vec<DiagnosticCluster> {
    let mut order: Vec<(ResidualClass, String)> = Vec::new();
    let mut buckets: BTreeMap<(String, String), Vec<StructuredDiagnostic>> = BTreeMap::new();
    let mut last_primary_key: Option<(ResidualClass, String)> = None;
    for diagnostic in diagnostics {
        let key = if !diagnostic.primary && diagnostic.code.is_none() {
            last_primary_key
                .clone()
                .unwrap_or((diagnostic.class, format!("{:?}", diagnostic.class)))
        } else {
            let code = diagnostic
                .code
                .clone()
                .unwrap_or_else(|| format!("{:?}", diagnostic.class));
            let key = (diagnostic.class, code);
            if diagnostic.primary {
                last_primary_key = Some(key.clone());
            }
            key
        };
        let map_key = (format!("{:?}", key.0), key.1.clone());
        if !buckets.contains_key(&map_key) {
            order.push(key.clone());
        }
        buckets.entry(map_key).or_default().push(diagnostic);
    }
    order
        .into_iter()
        .map(|(class, code)| {
            let members = buckets
                .remove(&(format!("{class:?}"), code.clone()))
                .unwrap_or_default();
            DiagnosticCluster {
                class,
                code,
                root: members.first().cloned().expect("nonempty cluster"),
                members,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn diag(class: ResidualClass, code: Option<&str>, primary: bool) -> StructuredDiagnostic {
        StructuredDiagnostic {
            class,
            code: code.map(str::to_string),
            message: "m".into(),
            path: Some("src/lib.rs".into()),
            line: Some(1),
            column: Some(1),
            primary,
            suggestion: None,
        }
    }

    #[test]
    fn cascades_fold_into_their_root_cluster() {
        let mut diagnostics = vec![diag(ResidualClass::ImportGraph, Some("E0432"), true)];
        for _ in 0..199 {
            diagnostics.push(diag(ResidualClass::ImportGraph, None, false));
        }
        let clusters = cluster(diagnostics);
        assert_eq!(clusters.len(), 1);
        assert_eq!(clusters[0].members.len(), 200);
        let magnitude = clusters[0].magnitude();
        assert!(magnitude < 8.0, "log damping: {magnitude}");
        assert!(magnitude > 1.0);
    }

    #[test]
    fn distinct_codes_stay_distinct_and_singletons_score_one() {
        let clusters = cluster(vec![
            diag(ResidualClass::Type, Some("E0308"), true),
            diag(ResidualClass::ImportGraph, Some("E0432"), true),
        ]);
        assert_eq!(clusters.len(), 2);
        assert!((clusters[0].magnitude() - 1.0).abs() < 1e-9);
    }
}
