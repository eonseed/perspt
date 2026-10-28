.. _api-perspt-prompt-macros:

``perspt-prompt-macros``
========================

Build-time compiler for Perspt prompt section files (PSP-10). Called from an
owning crate's ``build.rs``, it parses every ``prompts/<stage>/NN_name.md``,
runs the codegen validation list, and emits typed section structs into
``OUT_DIR``. A malformed section fails ``cargo build`` with an error naming
the offending file — never a session.

Core Types
----------

.. code-block:: rust

   pub struct StageDecl {
       pub dir_name: String,
       pub separator: String,
   }

   pub struct PromptBuildError {
       pub file: String,
       pub line: Option<usize>,
       pub message: String,
   }

   pub struct Generated {
       pub rust_source: String,
       pub sections: Vec<CompiledSection>,
   }

   pub fn compile_prompt_dir(
       root: &Path,
       stages: &[StageDecl],
   ) -> Result<Generated, PromptBuildError>;

Usage
-----

Consumers (``perspt-core``, ``perspt-coding``) call ``compile_prompt_dir``
from ``build.rs`` and include the generated source:

.. code-block:: rust

   include!(concat!(env!("OUT_DIR"), "/prompt_sections.rs"));

The same validation functions (``validate_section_body``,
``parse_section_file``) serve the runtime bundle scanner and
``perspt prompts lint``, so external replacement bodies obey exactly the
rules the build does.
