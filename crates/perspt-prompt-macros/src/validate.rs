//! The codegen validation list (PSP-10 system 23). The same checks run at
//! build time, in the runtime bundle scanner, and under
//! `perspt prompts lint` — one implementation, three doors.

use std::collections::BTreeSet;

use perspt_sdk::prompt::SectionSchema;

use crate::types::VarType;
use crate::PromptBuildError;

/// Every `{{placeholder}}` occurrence in a body, with its line number and
/// whether it stands alone on its line.
fn placeholders(body: &str) -> Result<Vec<(usize, String, bool)>, String> {
    let mut found = Vec::new();
    for (index, line) in body.lines().enumerate() {
        let mut rest = line;
        let mut on_line = Vec::new();
        while let Some(open) = rest.find("{{") {
            let Some(close) = rest[open..].find("}}") else {
                return Err(format!("unterminated placeholder on line {}", index + 1));
            };
            on_line.push(rest[open + 2..open + close].trim().to_string());
            rest = &rest[open + close + 2..];
        }
        let alone = on_line.len() == 1 && {
            let stripped = line.replacen(&format!("{{{{{}}}}}", on_line[0]), "", 1);
            line.trim() == format!("{{{{{}}}}}", on_line[0]) || stripped.trim().is_empty()
        };
        for name in on_line {
            found.push((index + 1, name, alone));
        }
    }
    Ok(found)
}

/// Validate one body against a compiled section schema. Used by the build
/// step for base and override files and by the bundle scanner for
/// replacement bodies (which must use exactly the known placeholder set
/// and rendering rules — this is not a template engine).
pub fn validate_section_body(schema: &SectionSchema, body: &str) -> Result<(), PromptBuildError> {
    let file = schema.id.0.clone();
    let err = |line: Option<usize>, message: String| PromptBuildError {
        file: file.clone(),
        line,
        message,
    };
    let occurrences = placeholders(body).map_err(|message| err(None, message))?;
    let declared: BTreeSet<&str> = schema.vars.iter().map(|var| var.name.as_str()).collect();
    let mut used: BTreeSet<String> = BTreeSet::new();
    let mut worst_case = body.len();
    for (line, name, alone) in &occurrences {
        if !declared.contains(name.as_str()) {
            return Err(err(
                Some(*line),
                format!("placeholder {{{{{name}}}}} is not declared in vars"),
            ));
        }
        used.insert(name.clone());
        let spec = schema
            .vars
            .iter()
            .find(|var| var.name == *name)
            .expect("declared checked above");
        let var_type = VarType::parse(&spec.declared_type).ok_or_else(|| {
            err(
                Some(*line),
                format!("declared type {:?} is unsupported", spec.declared_type),
            )
        })?;
        if var_type.omits_line() && !alone {
            return Err(err(
                Some(*line),
                format!(
                    "optional/list placeholder {{{{{name}}}}} must be the only \
                     non-whitespace content on its line (absence removes the line)"
                ),
            ));
        }
        // The declared bounds cap each variable's expansion; the true
        // rendered size is enforced again at render time. The build check
        // covers the static prose, which variables cannot shrink.
        worst_case = worst_case.saturating_sub(name.len() + 4);
    }
    for var in &schema.vars {
        if !used.contains(&var.name) {
            return Err(err(
                None,
                format!(
                    "declared variable {} never appears in the body (orphan)",
                    var.name
                ),
            ));
        }
    }
    if worst_case > schema.max_bytes {
        return Err(err(
            None,
            format!(
                "static body of {worst_case} bytes exceeds max_bytes {}; \
                 variable expansion only adds to it",
                schema.max_bytes
            ),
        ));
    }
    Ok(())
}
