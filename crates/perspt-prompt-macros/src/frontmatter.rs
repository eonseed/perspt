//! Hand-rolled parser for the closed section front-matter grammar.
//!
//! The grammar is seven keys plus a flat `vars` map; a general YAML
//! dependency would add an audit surface for no expressiveness this
//! grammar needs, and hand-rolling lets every error name the offending
//! file and line.

use std::collections::BTreeMap;

use perspt_sdk::prompt::ListStyle;

use crate::types::VarType;
use crate::PromptBuildError;

/// One parsed variable declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedVar {
    pub name: String,
    pub declared_type: String,
    pub var_type: VarType,
    pub style: Option<ListStyle>,
}

/// The parsed front matter of one section file.
#[derive(Debug, Clone, PartialEq)]
pub struct FrontMatter {
    pub id: String,
    pub version: u32,
    pub role: String,
    pub required: bool,
    pub priority: u16,
    pub max_bytes: usize,
    pub vars: Vec<ParsedVar>,
}

/// Split a section file into front matter and body, then parse the matter.
pub fn parse_section_file(
    file: &str,
    source: &str,
) -> Result<(FrontMatter, String), PromptBuildError> {
    let err = |line: usize, message: String| PromptBuildError {
        file: file.to_string(),
        line: Some(line),
        message,
    };
    let mut lines = source.lines().enumerate();
    match lines.next() {
        Some((_, "---")) => {}
        _ => {
            return Err(err(
                1,
                "section must open with a `---` front-matter fence".into(),
            ))
        }
    }
    let mut matter_lines = Vec::new();
    let mut body_start = None;
    for (index, line) in lines {
        if line.trim_end() == "---" {
            body_start = Some(index + 1);
            break;
        }
        matter_lines.push((index + 1, line.to_string()));
    }
    let Some(body_start) = body_start else {
        return Err(err(1, "front matter is missing its closing `---`".into()));
    };
    let body: String = source
        .lines()
        .skip(body_start)
        .collect::<Vec<_>>()
        .join("\n");
    let matter = parse_matter(file, &matter_lines)?;
    Ok((matter, body))
}

/// Split the raw front-matter lines into scalar entries and parsed vars.
#[allow(clippy::type_complexity)]
fn split_scalars_and_vars(
    file: &str,
    lines: &[(usize, String)],
) -> Result<(BTreeMap<String, (usize, String)>, Vec<ParsedVar>), PromptBuildError> {
    let mut scalars: BTreeMap<String, (usize, String)> = BTreeMap::new();
    let mut vars = Vec::new();
    let mut in_vars = false;
    for (line_no, line) in lines {
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        if line.trim_end() == "vars:" {
            in_vars = true;
            continue;
        }
        if in_vars && line.starts_with("  ") {
            vars.push(parse_var_line(file, *line_no, line)?);
            continue;
        }
        in_vars = false;
        let Some((key, value)) = line.split_once(':') else {
            return Err(PromptBuildError {
                file: file.to_string(),
                line: Some(*line_no),
                message: format!("unparseable line {line:?}"),
            });
        };
        scalars.insert(
            key.trim().to_string(),
            (*line_no, value.trim().trim_matches('"').to_string()),
        );
    }
    Ok((scalars, vars))
}

fn parse_matter(file: &str, lines: &[(usize, String)]) -> Result<FrontMatter, PromptBuildError> {
    let err = |line: usize, message: String| PromptBuildError {
        file: file.to_string(),
        line: Some(line),
        message,
    };
    let (scalars, vars) = split_scalars_and_vars(file, lines)?;
    let take = |key: &str| -> Result<(usize, String), PromptBuildError> {
        scalars
            .get(key)
            .cloned()
            .ok_or_else(|| err(1, format!("front matter is missing `{key}`")))
    };
    let (line, id) = take("id")?;
    if id.split('/').count() != 2 || id.contains(char::is_whitespace) {
        return Err(err(line, format!("id {id:?} must be `stage/section`")));
    }
    let (line, version) = take("version")?;
    let version: u32 = version
        .parse()
        .map_err(|_| err(line, format!("version {version:?} is not a u32")))?;
    let (line, role) = take("role")?;
    if role != "system" && role != "user" {
        return Err(err(line, format!("role {role:?} must be system or user")));
    }
    let (line, required) = take("required")?;
    let required: bool = required
        .parse()
        .map_err(|_| err(line, format!("required {required:?} is not a bool")))?;
    let priority: u16 = match scalars.get("priority") {
        Some((line, value)) => value
            .parse()
            .map_err(|_| err(*line, format!("priority {value:?} is not a u16")))?,
        None if required => 0,
        None => return Err(err(1, "an optional section must declare `priority`".into())),
    };
    let (line, max_bytes) = take("max_bytes")?;
    let max_bytes: usize = max_bytes
        .parse()
        .map_err(|_| err(line, format!("max_bytes {max_bytes:?} is not a usize")))?;
    Ok(FrontMatter {
        id,
        version,
        role,
        required,
        priority,
        max_bytes,
        vars,
    })
}

/// Parse one `  name: { type: "...", style: bullet_list }` line.
fn parse_var_line(file: &str, line_no: usize, line: &str) -> Result<ParsedVar, PromptBuildError> {
    let err = |message: String| PromptBuildError {
        file: file.to_string(),
        line: Some(line_no),
        message,
    };
    let trimmed = line.trim();
    let (name, rest) = trimmed
        .split_once(':')
        .ok_or_else(|| err(format!("unparseable var line {trimmed:?}")))?;
    let name = name.trim().to_string();
    let inner = rest
        .trim()
        .strip_prefix('{')
        .and_then(|value| value.strip_suffix('}'))
        .ok_or_else(|| err(format!("var {name} must use `{{ type: \"...\" }}`")))?;
    let mut declared_type = None;
    let mut style = None;
    for pair in split_attributes(inner) {
        let pair = pair.as_str();
        let Some((key, value)) = pair.split_once(':') else {
            return Err(err(format!("unparseable var attribute {pair:?}")));
        };
        let value = value.trim().trim_matches('"');
        match key.trim() {
            "type" => declared_type = Some(value.to_string()),
            "style" => {
                style = Some(match value {
                    "bullet_list" => ListStyle::BulletList,
                    "comma_list" => ListStyle::CommaList,
                    "numbered_list" => ListStyle::NumberedList,
                    other => return Err(err(format!("unknown list style {other:?}"))),
                })
            }
            other => return Err(err(format!("unknown var attribute {other:?}"))),
        }
    }
    let declared_type = declared_type.ok_or_else(|| err(format!("var {name} declares no type")))?;
    let var_type = VarType::parse(&declared_type).ok_or_else(|| {
        err(format!(
            "var {name}: type {declared_type:?} is not on the supported list \
             (BoundedText<N>, ObservationText<N>, BoundedList<C,B>, \
             Option<...>, i64, u64, f64)"
        ))
    })?;
    if matches!(var_type, VarType::List { .. }) && style.is_none() {
        return Err(err(format!("list var {name} declares no style")));
    }
    Ok(ParsedVar {
        name,
        declared_type,
        var_type,
        style,
    })
}

/// Split `type: "BoundedList<64,128>", style: bullet_list` on commas that
/// sit outside quotes and angle brackets.
fn split_attributes(inner: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut depth = 0i32;
    let mut quoted = false;
    for ch in inner.chars() {
        match ch {
            '"' => {
                quoted = !quoted;
                current.push(ch);
            }
            '<' if !quoted => {
                depth += 1;
                current.push(ch);
            }
            '>' if !quoted => {
                depth -= 1;
                current.push(ch);
            }
            ',' if !quoted && depth == 0 => {
                parts.push(std::mem::take(&mut current));
            }
            _ => current.push(ch),
        }
    }
    if !current.trim().is_empty() {
        parts.push(current);
    }
    parts
}
