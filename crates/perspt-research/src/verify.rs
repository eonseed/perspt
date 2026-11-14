//! Deterministic workspace verification for the research domain (`citation-check`).

use std::collections::{BTreeMap, BTreeSet};

use perspt_sdk::{
    AffectedSet, CorrectionOperator, CorrectionPacket, DomainEvaluation, EvidencePayload,
    FootprintSpec, IndependenceRoute, MissingSensorDecl, ResidualClass, ResidualClusterRef,
    ResidualEvent, ResidualSeverity, SensorRef, StructuredDiagnosticRef, SymbolRef,
    WorkspaceSnapshot,
};

use crate::ResearchDomain;

const SENSOR_ID: &str = "perspt-research/citation-check";

/// Verify bibliography definitions and citations across a research workspace.
pub(crate) fn evaluate_research_workspace(
    snapshot: &WorkspaceSnapshot,
    node_id: &str,
    generation: u32,
) -> Option<DomainEvaluation> {
    let has_research_files = snapshot
        .files
        .iter()
        .any(|p| is_bib_file(p) || is_source_index(p) || is_prose_doc(p));
    if !has_research_files {
        return None;
    }

    let mut defined_keys = BTreeSet::new();
    let mut syntax_errors: BTreeMap<String, Vec<StructuredDiagnosticRef>> = BTreeMap::new();

    for path in &snapshot.files {
        let Some(text) = snapshot.read_text(path) else {
            continue;
        };
        if is_bib_file(path) {
            let diags = parse_bib_definitions(path, &text, &mut defined_keys);
            if !diags.is_empty() {
                syntax_errors.insert(path.clone(), diags);
            }
        } else if is_source_index(path) {
            parse_markdown_sources(&text, &mut defined_keys);
        }
    }

    let mut residuals = build_syntax_residuals(node_id, generation, syntax_errors);
    for path in &snapshot.files {
        if !is_prose_doc(path) || is_source_index(path) {
            continue;
        }
        let Some(text) = snapshot.read_text(path) else {
            continue;
        };
        if let Some(ev) =
            missing_citations_residual(node_id, generation, path, &text, &defined_keys)
        {
            residuals.push(ev);
        }
    }

    Some(DomainEvaluation {
        ran_stages: vec!["citation-check".to_string()],
        residuals,
        advisory: Vec::new(),
    })
}

/// Build a typed correction packet folding all active research residuals.
pub(crate) fn build_correction_packet(
    domain: &ResearchDomain,
    residuals: &[ResidualEvent],
) -> Option<CorrectionPacket> {
    let dominant = residuals
        .iter()
        .filter(|r| r.class != ResidualClass::SensorUnavailable)
        .max_by(|a, b| a.score.total_cmp(&b.score))?;
    let (diagnostics, affected) = fold_evidence(residuals);
    let operators: Vec<CorrectionOperator> =
        perspt_sdk::AgentDomainPackage::correction_directions(domain, residuals)
            .into_iter()
            .map(|dir| CorrectionOperator {
                operator_id: dir.direction_id,
                instruction: dir.instruction,
                addresses: dir.addresses,
                rationale: dir.rationale,
            })
            .collect();
    let uncertainty: Vec<MissingSensorDecl> = residuals
        .iter()
        .filter(|r| r.class == ResidualClass::SensorUnavailable)
        .map(|r| MissingSensorDecl {
            sensor: r.sensor.id.clone(),
            reason: r.evidence.summary.clone(),
        })
        .collect();
    if operators.is_empty() && diagnostics.is_empty() {
        return None;
    }
    let member_count = dominant
        .evidence
        .structured
        .as_ref()
        .and_then(|v| v.as_array().map(|a| a.len() as u32))
        .unwrap_or(1);
    Some(CorrectionPacket {
        dominant_cluster: ResidualClusterRef {
            cluster_id: dominant.residual_id.clone(),
            class: dominant.class,
            root_cause: dominant.evidence.summary.clone(),
            member_count,
            magnitude: dominant.score,
        },
        diagnostics,
        affected,
        operators,
        expected_footprint: FootprintSpec::new(Vec::new()),
        follow_up_stages: Vec::new(),
        no_goods: Vec::new(),
        uncertainty,
    })
}

fn fold_evidence(residuals: &[ResidualEvent]) -> (Vec<StructuredDiagnosticRef>, AffectedSet) {
    let mut diagnostics = Vec::new();
    let mut affected = AffectedSet::default();
    for residual in residuals {
        if let Some(refs) =
            residual.evidence.structured.as_ref().and_then(|v| {
                serde_json::from_value::<Vec<StructuredDiagnosticRef>>(v.clone()).ok()
            })
        {
            for item in &refs {
                if let (Some(path), Some(line)) = (&item.path, item.line) {
                    affected
                        .spans
                        .push(format!("{path}:{line}:{}", item.column.unwrap_or(1)));
                }
            }
            diagnostics.extend(refs);
        }
        affected
            .paths
            .extend(residual.affected_paths.iter().cloned());
        affected
            .symbols
            .extend(residual.affected_symbols.iter().cloned());
    }
    affected.paths.sort();
    affected.paths.dedup();
    (diagnostics, affected)
}

fn build_syntax_residuals(
    node_id: &str,
    generation: u32,
    by_path: BTreeMap<String, Vec<StructuredDiagnosticRef>>,
) -> Vec<ResidualEvent> {
    by_path
        .into_iter()
        .filter_map(|(path, diags)| {
            let summary = format!(
                "{}: {} ({} BibTeX syntax issue{})",
                path,
                diags[0].message,
                diags.len(),
                if diags.len() == 1 { "" } else { "s" }
            );
            let mut ev = ResidualEvent::new(
                node_id,
                generation,
                ResidualClass::Syntax,
                ResidualSeverity::Error,
                diags.len() as f64,
                SensorRef::new(SENSOR_ID, IndependenceRoute::DeterministicTool),
            )
            .ok()?;
            ev.affected_paths = vec![path];
            ev.evidence = EvidencePayload {
                summary,
                raw: None,
                structured: serde_json::to_value(&diags).ok(),
            };
            Some(ev)
        })
        .collect()
}

fn missing_citations_residual(
    node_id: &str,
    generation: u32,
    path: &str,
    text: &str,
    defined: &BTreeSet<String>,
) -> Option<ResidualEvent> {
    let missing = find_missing_citations(text, defined);
    if missing.is_empty() {
        return None;
    }
    let diags: Vec<StructuredDiagnosticRef> = missing
        .iter()
        .map(|(key, line)| StructuredDiagnosticRef {
            code: Some("MISSING_CITATION".to_string()),
            message: format!("unresolved citation key @{key}"),
            path: Some(path.to_string()),
            line: Some(*line),
            column: None,
        })
        .collect();
    let keys_preview: Vec<&str> = missing.iter().map(|(k, _)| k.as_str()).collect();
    let summary = format!(
        "{path}: unresolved citation(s): {}",
        keys_preview.join(", ")
    );
    let mut ev = ResidualEvent::new(
        node_id,
        generation,
        ResidualClass::ImportGraph,
        ResidualSeverity::Error,
        missing.len() as f64,
        SensorRef::new(SENSOR_ID, IndependenceRoute::DeterministicTool),
    )
    .ok()?;
    ev.affected_paths = vec![path.to_string()];
    ev.affected_symbols = missing
        .into_iter()
        .map(|(key, _)| SymbolRef {
            name: key,
            container: Some(path.to_string()),
        })
        .collect();
    ev.evidence = EvidencePayload {
        summary,
        raw: None,
        structured: serde_json::to_value(&diags).ok(),
    };
    Some(ev)
}

fn parse_bib_definitions(
    path: &str,
    text: &str,
    keys: &mut BTreeSet<String>,
) -> Vec<StructuredDiagnosticRef> {
    let mut diags = Vec::new();
    let bytes = text.as_bytes();
    let mut i = 0;
    let mut depth: i32 = 0;
    let mut line: u32 = 1;

    while i < bytes.len() {
        match bytes[i] {
            b'\n' => {
                line += 1;
                i += 1;
            }
            b'%' if depth == 0 => {
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
            }
            b'@' if depth == 0 => {
                i = consume_bib_entry(path, text, i, &mut line, &mut depth, keys, &mut diags);
            }
            b'{' => {
                depth += 1;
                i += 1;
            }
            b'}' => {
                depth -= 1;
                if depth < 0 {
                    diags.push(bib_diag(path, line, "unexpected closing brace '}'"));
                    depth = 0;
                }
                i += 1;
            }
            _ => i += 1,
        }
    }
    if depth != 0 {
        diags.push(bib_diag(path, line, "unclosed '{' in BibTeX file"));
    }
    diags
}

fn consume_bib_entry(
    path: &str,
    text: &str,
    at_idx: usize,
    line: &mut u32,
    depth: &mut i32,
    keys: &mut BTreeSet<String>,
    diags: &mut Vec<StructuredDiagnosticRef>,
) -> usize {
    let entry_line = *line;
    let mut i = at_idx + 1;
    let type_start = i;
    while i < text.len() && text.as_bytes()[i].is_ascii_alphabetic() {
        i += 1;
    }
    let entry_type = &text[type_start..i];
    while i < text.len() && matches!(text.as_bytes()[i], b' ' | b'\t' | b'\r') {
        i += 1;
    }
    if i >= text.len() || text.as_bytes()[i] != b'{' {
        diags.push(bib_diag(
            path,
            entry_line,
            "expected '{' after '@' entry type",
        ));
        return i;
    }
    *depth += 1;
    i += 1;
    if matches_bib_meta_entry(entry_type) {
        return i;
    }
    let key_start = i;
    while i < text.len() && !matches!(text.as_bytes()[i], b',' | b'}' | b'{' | b'\n') {
        i += 1;
    }
    let key = text[key_start..i].trim();
    if i >= text.len() || text.as_bytes()[i] != b',' || key.is_empty() || key.contains(' ') {
        diags.push(bib_diag(
            path,
            entry_line,
            &format!("malformed BibTeX citation key near '@{entry_type}'"),
        ));
    } else {
        keys.insert(key.to_string());
    }
    i
}

fn matches_bib_meta_entry(entry_type: &str) -> bool {
    entry_type.eq_ignore_ascii_case("comment")
        || entry_type.eq_ignore_ascii_case("preamble")
        || entry_type.eq_ignore_ascii_case("string")
}

fn bib_diag(path: &str, line: u32, message: &str) -> StructuredDiagnosticRef {
    StructuredDiagnosticRef {
        code: Some("BIB_SYNTAX".to_string()),
        message: message.to_string(),
        path: Some(path.to_string()),
        line: Some(line),
        column: None,
    }
}

fn parse_markdown_sources(text: &str, keys: &mut BTreeSet<String>) {
    for raw_line in text.lines() {
        let trimmed = raw_line.trim_start();
        let bullet = trimmed
            .strip_prefix("- ")
            .or_else(|| trimmed.strip_prefix("* "))
            .unwrap_or(trimmed);
        if let Some(rest) = bullet.strip_prefix("[@")
            && let Some((key, after)) = rest.split_once(']')
        {
            let trimmed_key = key.trim();
            if is_valid_key(trimmed_key)
                && (after.starts_with(':') || bullet != trimmed || after.starts_with(" "))
            {
                keys.insert(trimmed_key.to_string());
            }
        }
    }
}

fn find_missing_citations(text: &str, defined: &BTreeSet<String>) -> Vec<(String, u32)> {
    let mut missing = Vec::new();
    let mut seen = BTreeSet::new();
    let mut in_fence = false;

    for (idx, line) in text.lines().enumerate() {
        let line_no = (idx + 1) as u32;
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        for key in extract_bracket_citations(line)
            .into_iter()
            .chain(extract_latex_citations(line))
        {
            if !defined.contains(&key) && seen.insert(key.clone()) {
                missing.push((key, line_no));
            }
        }
    }
    missing
}

fn extract_bracket_citations(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = line;
    while let Some(open) = rest.find("[@") {
        let after_open = &rest[open + 1..];
        let Some(close) = after_open.find(']') else {
            break;
        };
        let trailing = &after_open[close + 1..];
        if !trailing.starts_with(':') {
            for part in after_open[..close].split(';') {
                if let Some(at) = part.trim().strip_prefix('@') {
                    let key: String = at.chars().take_while(|c| is_key_char(*c)).collect();
                    if is_valid_key(&key) {
                        out.push(key);
                    }
                }
            }
        }
        rest = &after_open[close + 1..];
    }
    out
}

fn extract_latex_citations(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    for cmd in [
        "\\citep",
        "\\citet",
        "\\autocite",
        "\\textcite",
        "\\citeauthor",
        "\\cite",
    ] {
        let mut rest = line;
        while let Some(pos) = rest.find(cmd) {
            let mut after = &rest[pos + cmd.len()..];
            if after.starts_with('[')
                && let Some(end_opt) = after.find(']')
            {
                after = &after[end_opt + 1..];
            }
            if let Some(after_brace) = after.strip_prefix('{')
                && let Some(close) = after_brace.find('}')
            {
                for raw in after_brace[..close].split(',') {
                    let key = raw.trim();
                    if is_valid_key(key) {
                        out.push(key.to_string());
                    }
                }
                rest = &after_brace[close + 1..];
            } else {
                rest = after;
            }
        }
    }
    out
}

fn is_key_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | ':' | '.' | '/')
}

fn is_valid_key(key: &str) -> bool {
    !key.is_empty() && key.as_bytes()[0].is_ascii_alphanumeric() && key.chars().all(is_key_char)
}

fn is_bib_file(path: &str) -> bool {
    path.ends_with(".bib")
}

fn is_source_index(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    matches!(name, "sources.md" | "references.md")
}

fn is_prose_doc(path: &str) -> bool {
    path.ends_with(".md") || path.ends_with(".rst") || path.ends_with(".tex")
}
