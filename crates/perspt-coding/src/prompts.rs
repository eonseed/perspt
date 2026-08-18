//! The coding domain's prompt section library (PSP-10 systems 23 and 26).
//!
//! `branch_correct` renders a typed `CorrectionPacket` for the model:
//! paths, symbols, spans, and rationale all survive — the correction
//! channel no longer flattens to one instruction string.

use perspt_sdk::error::Result;
use perspt_sdk::prompt::{BoundedList, BoundedText, PromptSection};
use perspt_sdk::CorrectionPacket;

mod generated {
    include!(concat!(env!("OUT_DIR"), "/prompt_sections.rs"));
}

pub use generated::branch_correct;

fn bounded_items(
    items: impl IntoIterator<Item = String>,
    count: usize,
    bytes: usize,
) -> Vec<String> {
    items
        .into_iter()
        .map(|item| {
            let mut item = item;
            if item.len() > bytes {
                let mut end = bytes.saturating_sub(1);
                while end > 0 && !item.is_char_boundary(end) {
                    end -= 1;
                }
                item.truncate(end);
                item.push('…');
            }
            item
        })
        .take(count)
        .collect()
}

/// Render a correction packet through the `branch_correct` section program.
pub fn render_correction(packet: &CorrectionPacket) -> Result<String> {
    let diagnostics = bounded_items(
        packet.diagnostics.iter().map(|diagnostic| {
            let location = match (&diagnostic.path, diagnostic.line) {
                (Some(path), Some(line)) => format!("{path}:{line}: "),
                (Some(path), None) => format!("{path}: "),
                _ => String::new(),
            };
            let code = diagnostic
                .code
                .as_deref()
                .map(|code| format!("[{code}] "))
                .unwrap_or_default();
            format!("{location}{code}{}", diagnostic.message)
        }),
        32,
        500,
    );
    let operators = bounded_items(
        packet.operators.iter().map(|operator| {
            if operator.rationale.is_empty() {
                operator.instruction.clone()
            } else {
                format!("{} ({})", operator.instruction, operator.rationale)
            }
        }),
        8,
        1000,
    );
    let mut summary = packet.dominant_cluster.root_cause.clone();
    summary.truncate(1000);
    let sections = [
        branch_correct::Role {}.render()?,
        branch_correct::Correction {
            cluster_summary: BoundedText::new(summary)?,
            diagnostics: BoundedList::new(diagnostics)?,
            paths: BoundedList::new(bounded_items(
                packet.affected.paths.iter().cloned(),
                32,
                250,
            ))?,
            symbols: BoundedList::new(bounded_items(
                packet
                    .affected
                    .symbols
                    .iter()
                    .map(|symbol| symbol.name.clone()),
                32,
                120,
            ))?,
            operators: BoundedList::new(operators)?,
        }
        .render()?,
        branch_correct::OutputContract {}.render()?,
    ];
    Ok(sections
        .iter()
        .map(|section| section.content.as_str())
        .collect::<Vec<_>>()
        .join(branch_correct::SEPARATOR))
}
