//! Prompt sections: identity, schema, and deterministic rendering
//! (PSP-10 system 23, Definition 4).
//!
//! A section is a small, single-concern unit with typed front matter. Its
//! body is prose plus `{{variable}}` placeholders and nothing else — no
//! conditionals, loops, includes, or filters. `perspt-prompt-macros`
//! validates the files at build time and generates one typed struct per
//! section; [`SectionTemplate::render`] here is the single implementation
//! of the substitution rules both the generated code and the external
//! bundle scanner use.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::canon::CanonicalEncoder;
use crate::error::{Result, SdkError};

use super::vars::{ListStyle, VarValue};

/// Domain tag for every prompt-plane digest.
pub const PROMPT_DIGEST_TAG: &[u8] = b"perspt-prompt-v1";

/// Section identity, `"stage/section"`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PromptSectionId(pub String);

impl std::fmt::Display for PromptSectionId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Monotone section version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PromptSectionVersion(pub u32);

/// The message slot a section renders into.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromptMessageRole {
    System,
    User,
}

/// One declared variable in a section's front matter.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VarSpec {
    /// The `{{name}}` the body uses.
    pub name: String,
    /// The declared Rust type, verbatim from the front matter (e.g.
    /// `"BoundedList<64,128>"`). Codegen maps it to a supported type; the
    /// runtime treats it as opaque schema identity.
    pub declared_type: String,
    /// Declared list style, when the type is a list.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<ListStyle>,
}

/// A compiled section's schema: what a replacement body must satisfy
/// (system 25 — external bundles may replace a known section only under
/// exactly this schema).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SectionSchema {
    pub id: PromptSectionId,
    pub version: PromptSectionVersion,
    pub role: PromptMessageRole,
    pub required: bool,
    /// Higher survives budget fitting longer; unused for required sections.
    pub priority: u16,
    pub max_bytes: usize,
    pub vars: Vec<VarSpec>,
}

/// A section template: schema plus the reviewed body text.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SectionTemplate {
    pub schema: SectionSchema,
    /// The body with `{{placeholders}}`; content-addressed.
    pub body: String,
    /// SHA-256 of the body bytes, `sha256:`-prefixed.
    pub content_hash: String,
}

impl SectionTemplate {
    /// Content hash of a section body under the prompt digest discipline.
    pub fn hash_body(body: &str) -> String {
        let mut encoder = CanonicalEncoder::new(PROMPT_DIGEST_TAG);
        encoder.text("section-body").text(body);
        encoder.digest()
    }

    /// Deterministic rendering: substitute every placeholder, dropping the
    /// whole line when an optional or list value is absent/empty. Fails
    /// closed on an undeclared placeholder, a missing value, or a rendered
    /// body over `max_bytes`.
    pub fn render(&self, values: &BTreeMap<String, VarValue>) -> Result<RenderedSection> {
        let mut lines = Vec::new();
        for line in self.body.lines() {
            match render_line(line, &self.schema, values)? {
                Some(rendered) => lines.push(rendered),
                None => continue,
            }
        }
        let content = lines.join("\n");
        if content.len() > self.schema.max_bytes {
            return Err(SdkError::Domain(format!(
                "section {} rendered to {} bytes over its {}-byte cap",
                self.schema.id,
                content.len(),
                self.schema.max_bytes
            )));
        }
        Ok(RenderedSection {
            id: self.schema.id.clone(),
            version: self.schema.version,
            role: self.schema.role,
            required: self.schema.required,
            priority: self.schema.priority,
            content_hash: self.content_hash.clone(),
            content,
        })
    }
}

/// Substitute one line, or drop it (`None`) when an omitting value appears.
fn render_line(
    line: &str,
    schema: &SectionSchema,
    values: &BTreeMap<String, VarValue>,
) -> Result<Option<String>> {
    let mut rendered = String::new();
    let mut rest = line;
    while let Some(open) = rest.find("{{") {
        let Some(close) = rest[open..].find("}}") else {
            return Err(SdkError::Domain(format!(
                "unterminated placeholder in line {line:?}"
            )));
        };
        let name = rest[open + 2..open + close].trim();
        let spec = schema
            .vars
            .iter()
            .find(|spec| spec.name == name)
            .ok_or_else(|| SdkError::Domain(format!("undeclared placeholder {{{{{name}}}}}")))?;
        let value = values
            .get(name)
            .ok_or_else(|| SdkError::Domain(format!("no value supplied for {{{{{name}}}}}")))?;
        if value.omits_line() {
            return Ok(None);
        }
        rendered.push_str(&rest[..open]);
        rendered.push_str(&value.render(spec.style)?);
        rest = &rest[open + close + 2..];
    }
    rendered.push_str(rest);
    Ok(Some(rendered))
}

/// A rendered section carrying its provenance-relevant identity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RenderedSection {
    pub id: PromptSectionId,
    pub version: PromptSectionVersion,
    pub role: PromptMessageRole,
    pub required: bool,
    pub priority: u16,
    /// Hash of the template body this rendering came from.
    pub content_hash: String,
    pub content: String,
}

/// Where a resolved section came from (Definition 4's override chain).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OverrideOrigin {
    Base,
    Family(String),
    ExactModel(String),
}

/// Ledger-facing provenance of one composed section.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SectionProvenance {
    pub id: PromptSectionId,
    pub version: PromptSectionVersion,
    pub content_hash: String,
    pub origin: OverrideOrigin,
}

/// The compile-time contract every generated section struct satisfies
/// (system 23's generated API).
pub trait PromptSection {
    const ID: &'static str;
    const VERSION: u32;
    const CONTENT_HASH: &'static str;
    const REQUIRED: bool;
    const PRIORITY: u16;

    /// Deterministic rendering of this section with its typed values.
    fn render(&self) -> Result<RenderedSection>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn template() -> SectionTemplate {
        let body = "Use only these tools:\n{{tool_names}}\n{{budget_note}}\nEnd.".to_string();
        SectionTemplate {
            schema: SectionSchema {
                id: PromptSectionId("branch_correct/tool_protocol".into()),
                version: PromptSectionVersion(3),
                role: PromptMessageRole::System,
                required: true,
                priority: 90,
                max_bytes: 4096,
                vars: vec![
                    VarSpec {
                        name: "tool_names".into(),
                        declared_type: "BoundedList<64,128>".into(),
                        style: Some(ListStyle::BulletList),
                    },
                    VarSpec {
                        name: "budget_note".into(),
                        declared_type: "Option<BoundedText<512>>".into(),
                        style: None,
                    },
                ],
            },
            content_hash: SectionTemplate::hash_body(body.as_str()),
            body,
        }
    }

    #[test]
    fn substitution_and_line_omission_are_deterministic() {
        let template = template();
        let mut values = BTreeMap::new();
        values.insert(
            "tool_names".to_string(),
            VarValue::List(vec!["read_file".into(), "edit_file".into()]),
        );
        values.insert("budget_note".to_string(), VarValue::Absent);
        let rendered = template.render(&values).unwrap();
        assert_eq!(
            rendered.content,
            "Use only these tools:\n- read_file\n- edit_file\nEnd."
        );
        assert_eq!(rendered.content, template.render(&values).unwrap().content);
    }

    #[test]
    fn undeclared_and_missing_placeholders_fail_closed() {
        let mut template = template();
        template.body = "{{ghost}}".into();
        assert!(template.render(&BTreeMap::new()).is_err());
        let template = self::tests::template();
        assert!(template.render(&BTreeMap::new()).is_err());
    }

    #[test]
    fn oversize_rendering_fails_closed() {
        let mut template = template();
        template.schema.max_bytes = 8;
        let mut values = BTreeMap::new();
        values.insert(
            "tool_names".to_string(),
            VarValue::List(vec!["read_file".into()]),
        );
        values.insert("budget_note".to_string(), VarValue::Absent);
        assert!(template.render(&values).is_err());
    }
}
