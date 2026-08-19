//! Rust source emission for compiled sections. The generated file lands in
//! `OUT_DIR` and is included by the owning crate; it is never committed and
//! never hand-edited.

use crate::frontmatter::ParsedVar;
use crate::CompiledSection;

/// PascalCase struct name for a section name like `tool_protocol`.
pub fn struct_name(section_name: &str) -> String {
    section_name
        .split('_')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect()
}

fn style_expr(var: &ParsedVar) -> String {
    match var.style {
        None => "None".into(),
        Some(style) => format!("Some(perspt_sdk::prompt::ListStyle::{style:?})"),
    }
}

fn role_expr(role: &str) -> &'static str {
    match role {
        "user" => "perspt_sdk::prompt::PromptMessageRole::User",
        _ => "perspt_sdk::prompt::PromptMessageRole::System",
    }
}

/// Emit the `SectionTemplate` constructor expression for one section.
fn template_expr(section: &CompiledSection) -> String {
    let vars = section
        .vars
        .iter()
        .map(|var| {
            format!(
                "perspt_sdk::prompt::VarSpec {{ name: {:?}.to_string(), \
                 declared_type: {:?}.to_string(), style: {} }}",
                var.name,
                var.declared_type,
                style_expr(var)
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "perspt_sdk::prompt::SectionTemplate {{\n\
         \x20           schema: perspt_sdk::prompt::SectionSchema {{\n\
         \x20               id: perspt_sdk::prompt::PromptSectionId({id:?}.to_string()),\n\
         \x20               version: perspt_sdk::prompt::PromptSectionVersion({version}),\n\
         \x20               role: {role},\n\
         \x20               required: {required},\n\
         \x20               priority: {priority},\n\
         \x20               max_bytes: {max_bytes},\n\
         \x20               vars: vec![{vars}],\n\
         \x20           }},\n\
         \x20           body: {body:?}.to_string(),\n\
         \x20           content_hash: {hash:?}.to_string(),\n\
         \x20       }}",
        id = section.schema.id.0,
        version = section.schema.version.0,
        role = role_expr(&section.role_name),
        required = section.schema.required,
        priority = section.schema.priority,
        max_bytes = section.schema.max_bytes,
        body = section.body,
        hash = section.content_hash,
    )
}

/// Emit one base section's struct, trait impl, and template constructor.
fn emit_base(section: &CompiledSection) -> String {
    let name = struct_name(&section.section_name);
    let fields = section
        .vars
        .iter()
        .map(|var| format!("        pub {}: {},", var.name, var.var_type.rust_type()))
        .collect::<Vec<_>>()
        .join("\n");
    let inserts = section
        .vars
        .iter()
        .map(|var| {
            format!(
                "            values.insert({:?}.to_string(), {});",
                var.name,
                var.var_type.value_expr(&var.name)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let template = template_expr(section);
    format!(
        "    /// Generated from `{file}`; do not edit.\n\
         \x20   #[derive(Debug, Clone)]\n\
         \x20   pub struct {name} {{\n{fields}\n    }}\n\n\
         \x20   impl {name} {{\n\
         \x20       pub fn template() -> perspt_sdk::prompt::SectionTemplate {{\n\
         \x20           {template}\n\
         \x20       }}\n\n\
         \x20       /// The typed variable values this instance renders with —\n\
         \x20       /// the substitution input for a validated override body.\n\
         \x20       pub fn values(\n\
         \x20           &self,\n\
         \x20       ) -> std::collections::BTreeMap<String, perspt_sdk::prompt::VarValue> {{\n\
         \x20           #[allow(unused_mut)]\n\
         \x20           let mut values = std::collections::BTreeMap::new();\n{inserts}\n\
         \x20           values\n\
         \x20       }}\n\
         \x20   }}\n\n\
         \x20   impl perspt_sdk::prompt::PromptSection for {name} {{\n\
         \x20       const ID: &'static str = {id:?};\n\
         \x20       const VERSION: u32 = {version};\n\
         \x20       const CONTENT_HASH: &'static str = {hash:?};\n\
         \x20       const REQUIRED: bool = {required};\n\
         \x20       const PRIORITY: u16 = {priority};\n\n\
         \x20       fn render(&self) -> perspt_sdk::error::Result<perspt_sdk::prompt::RenderedSection> {{\n\
         \x20           Self::template().render(&self.values())\n\
         \x20       }}\n\
         \x20   }}\n",
        file = section.file_name,
        id = section.schema.id.0,
        version = section.schema.version.0,
        hash = section.content_hash,
        required = section.schema.required,
        priority = section.schema.priority,
    )
}

/// Emit one stage module.
pub fn emit_stage(
    dir_name: &str,
    separator: &str,
    sections: &[&CompiledSection],
    overrides: &[&CompiledSection],
) -> String {
    let bases = sections
        .iter()
        .map(|section| emit_base(section))
        .collect::<Vec<_>>()
        .join("\n");
    let template_list = sections
        .iter()
        .map(|section| {
            format!(
                "            {}::template(),",
                struct_name(&section.section_name)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let override_list = overrides
        .iter()
        .map(|section| {
            format!(
                "            ({:?}, {:?}, {}),",
                section.section_name,
                section
                    .override_label
                    .as_deref()
                    .expect("override sections carry a label"),
                template_expr(section)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "pub mod {dir_name} {{\n\
         \x20   //! Generated by perspt-prompt-macros; do not edit.\n\
         \x20   #![allow(clippy::all)]\n\n\
         \x20   pub const SEPARATOR: &str = {separator:?};\n\n\
         {bases}\n\
         \x20   /// Every base section template, in declared order.\n\
         \x20   pub fn templates() -> Vec<perspt_sdk::prompt::SectionTemplate> {{\n\
         \x20       vec![\n{template_list}\n        ]\n\
         \x20   }}\n\n\
         \x20   /// Override templates as `(section, route_label, template)`.\n\
         \x20   /// Activation is governed by the committed manifest (Gate AE).\n\
         \x20   pub fn overrides()\n\
         \x20   -> Vec<(&'static str, &'static str, perspt_sdk::prompt::SectionTemplate)> {{\n\
         \x20       vec![\n{override_list}\n        ]\n\
         \x20   }}\n\
         }}\n"
    )
}
