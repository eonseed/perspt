//! The codegen validation list, falsified case by case (PSP-10 Gate Y):
//! a malformed section fails the build with an error naming the offending
//! file — never a session.

use std::path::Path;

use perspt_prompt_macros::{compile_prompt_dir, StageDecl};

fn write_stage(root: &Path, stage: &str, files: &[(&str, &str)]) {
    let dir = root.join(stage);
    std::fs::create_dir_all(&dir).unwrap();
    for (name, content) in files {
        std::fs::write(dir.join(name), content).unwrap();
    }
}

fn stages() -> Vec<StageDecl> {
    vec![StageDecl::new("branch_correct", " ")]
}

const VALID: &str = "---\n\
id: branch_correct/tool_protocol\n\
version: 3\n\
role: system\n\
required: true\n\
max_bytes: 4096\n\
vars:\n\
\x20 tool_names: { type: \"BoundedList<64,128>\", style: bullet_list }\n\
\x20 budget_note: { type: \"Option<BoundedText<512>>\" }\n\
---\n\
Use only these tools:\n\
{{tool_names}}\n\
{{budget_note}}\n";

#[test]
fn a_valid_section_compiles_to_a_typed_struct() {
    let dir = tempfile::tempdir().unwrap();
    write_stage(
        dir.path(),
        "branch_correct",
        &[("20_tool_protocol.md", VALID)],
    );
    let generated = compile_prompt_dir(dir.path(), &stages()).unwrap();
    assert_eq!(generated.sections.len(), 1);
    let section = &generated.sections[0];
    assert_eq!(section.schema.id.0, "branch_correct/tool_protocol");
    assert!(section.content_hash.starts_with("sha256:"));
    assert!(generated.rust_source.contains("pub struct ToolProtocol"));
    assert!(generated
        .rust_source
        .contains("const ID: &'static str = \"branch_correct/tool_protocol\""));
    assert!(generated
        .rust_source
        .contains("pub const SEPARATOR: &str = \" \""));
    // The compiled template renders through the SDK's single renderer.
    let template = perspt_sdk::prompt::SectionTemplate {
        schema: section.schema.clone(),
        body: section.body.clone(),
        content_hash: section.content_hash.clone(),
    };
    let mut values = std::collections::BTreeMap::new();
    values.insert(
        "tool_names".to_string(),
        perspt_sdk::prompt::VarValue::List(vec!["read_file".into()]),
    );
    values.insert(
        "budget_note".to_string(),
        perspt_sdk::prompt::VarValue::Absent,
    );
    let rendered = template.render(&values).unwrap();
    assert_eq!(rendered.content, "Use only these tools:\n- read_file");
}

#[test]
fn undeclared_variable_fails_the_build() {
    let dir = tempfile::tempdir().unwrap();
    let body = VALID.replace("{{tool_names}}", "{{ghost_tools}}");
    write_stage(
        dir.path(),
        "branch_correct",
        &[("20_tool_protocol.md", &body)],
    );
    let error = compile_prompt_dir(dir.path(), &stages()).unwrap_err();
    assert!(error.message.contains("ghost_tools"), "{error}");
    assert!(error.file.contains("20_tool_protocol.md"), "{error}");
}

#[test]
fn orphan_variable_fails_the_build() {
    let dir = tempfile::tempdir().unwrap();
    let body = VALID.replace("{{budget_note}}\n", "");
    write_stage(
        dir.path(),
        "branch_correct",
        &[("20_tool_protocol.md", &body)],
    );
    let error = compile_prompt_dir(dir.path(), &stages()).unwrap_err();
    assert!(error.message.contains("orphan"), "{error}");
}

#[test]
fn unbounded_variable_type_fails_the_build() {
    let dir = tempfile::tempdir().unwrap();
    let body = VALID.replace("Option<BoundedText<512>>", "String");
    write_stage(
        dir.path(),
        "branch_correct",
        &[("20_tool_protocol.md", &body)],
    );
    let error = compile_prompt_dir(dir.path(), &stages()).unwrap_err();
    assert!(error.message.contains("supported list"), "{error}");
}

#[test]
fn a_declared_stage_without_base_sections_fails_the_build() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("branch_correct")).unwrap();
    let error = compile_prompt_dir(dir.path(), &stages()).unwrap_err();
    assert!(error.message.contains("no base sections"), "{error}");
}

#[test]
fn oversize_static_body_fails_the_build() {
    let dir = tempfile::tempdir().unwrap();
    let body = VALID.replace("max_bytes: 4096", "max_bytes: 8");
    write_stage(
        dir.path(),
        "branch_correct",
        &[("20_tool_protocol.md", &body)],
    );
    let error = compile_prompt_dir(dir.path(), &stages()).unwrap_err();
    assert!(error.message.contains("exceeds max_bytes"), "{error}");
}

#[test]
fn bad_front_matter_fails_with_the_offending_line() {
    let dir = tempfile::tempdir().unwrap();
    let body = VALID.replace("version: 3", "version: three");
    write_stage(
        dir.path(),
        "branch_correct",
        &[("20_tool_protocol.md", &body)],
    );
    let error = compile_prompt_dir(dir.path(), &stages()).unwrap_err();
    assert!(error.message.contains("not a u32"), "{error}");
    assert!(error.line.is_some());
    // Missing keys fail too.
    let body = VALID.replace("role: system\n", "");
    write_stage(
        dir.path(),
        "branch_correct",
        &[("20_tool_protocol.md", &body)],
    );
    let error = compile_prompt_dir(dir.path(), &stages()).unwrap_err();
    assert!(error.message.contains("missing `role`"), "{error}");
}

#[test]
fn inline_list_placeholder_fails_the_line_omission_rule() {
    let dir = tempfile::tempdir().unwrap();
    let body = VALID.replace("{{tool_names}}\n", "tools: {{tool_names}} end\n");
    write_stage(
        dir.path(),
        "branch_correct",
        &[("20_tool_protocol.md", &body)],
    );
    let error = compile_prompt_dir(dir.path(), &stages()).unwrap_err();
    assert!(error.message.contains("only"), "{error}");
}

#[test]
fn an_override_with_a_different_schema_fails_the_build() {
    let dir = tempfile::tempdir().unwrap();
    let override_body = VALID
        .replace(
            "\x20 budget_note: { type: \"Option<BoundedText<512>>\" }\n",
            "",
        )
        .replace("{{budget_note}}\n", "");
    write_stage(
        dir.path(),
        "branch_correct",
        &[
            ("20_tool_protocol.md", VALID),
            ("20_tool_protocol.family_a.md", &override_body),
        ],
    );
    let error = compile_prompt_dir(dir.path(), &stages()).unwrap_err();
    assert!(error.message.contains("variable schema"), "{error}");
}

#[test]
fn a_compatible_override_compiles_as_an_override() {
    let dir = tempfile::tempdir().unwrap();
    let override_body = VALID.replace("Use only these tools:", "Tools available to you:");
    write_stage(
        dir.path(),
        "branch_correct",
        &[
            ("20_tool_protocol.md", VALID),
            ("20_tool_protocol.family_a.md", &override_body),
        ],
    );
    let generated = compile_prompt_dir(dir.path(), &stages()).unwrap();
    assert_eq!(generated.sections.len(), 2);
    assert!(generated
        .sections
        .iter()
        .any(|section| section.override_label.as_deref() == Some("family_a")));
    assert!(generated.rust_source.contains("pub fn overrides()"));
}

#[test]
fn manifest_drift_is_refused_with_a_regenerate_hint() {
    let dir = tempfile::tempdir().unwrap();
    write_stage(
        dir.path(),
        "branch_correct",
        &[("20_tool_protocol.md", VALID)],
    );
    let generated = compile_prompt_dir(dir.path(), &stages()).unwrap();
    let manifest = dir.path().join("manifest.toml");
    std::fs::write(
        &manifest,
        "[[section]]\nid = \"branch_correct/tool_protocol\"\nversion = 3\n\
         content_hash = \"sha256:stale\"\n",
    )
    .unwrap();
    let error =
        perspt_prompt_macros::validate_manifest(&manifest, &generated.sections).unwrap_err();
    assert!(error.message.contains("stale"), "{error}");
    assert!(error.message.contains("perspt prompts manifest"), "{error}");
    // A faithful manifest passes.
    let section = &generated.sections[0];
    std::fs::write(
        &manifest,
        format!(
            "[[section]]\nid = \"branch_correct/tool_protocol\"\nversion = 3\ncontent_hash = \"{}\"\n",
            section.content_hash
        ),
    )
    .unwrap();
    perspt_prompt_macros::validate_manifest(&manifest, &generated.sections).unwrap();
}
