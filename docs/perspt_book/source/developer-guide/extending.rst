.. _developer-guide-extending:

Extending Perspt
================

Adding a Language Plugin
------------------------

Language plugins implement the ``LanguagePlugin`` trait from ``perspt-core``.

1. Create a new module in ``crates/perspt-core/src/plugin/``:

   .. code-block:: rust

      pub struct GoPlugin;

      impl LanguagePlugin for GoPlugin {
          fn name(&self) -> &str { "go" }
          fn extensions(&self) -> &[&str] { &["go"] }
          fn key_files(&self) -> &[&str] { &["go.mod", "go.sum"] }

          fn detect(&self, path: &Path) -> bool {
              path.join("go.mod").exists()
          }

          fn get_init_action(&self, opts: &InitOptions) -> ProjectAction {
              ProjectAction::ExecCommand {
                  command: format!("go mod init {}", opts.name),
                  description: "Initialize Go module".into(),
              }
          }

          fn test_command(&self) -> String {
              "go test ./...".into()
          }

          fn syntax_check_command(&self) -> Option<String> {
              Some("go vet ./...".into())
          }

          fn verifier_profile(&self) -> VerifierProfile {
              // Define capabilities for each verifier stage
              // with primary and fallback commands
          }
      }

2. Register in the ``PluginRegistry``
3. Add LSP config for ``gopls``

Designing Domain Adapters
--------------------------

The platform SDK establishes a modular separation of concerns: the core SRBN control plane (in ``perspt-sdk``) owns scheduling, residual scoring, capability checking, ledger tracking, and telemetry monitoring, while the domain adapter owns the domain-specific logic.

To write a custom domain extension (for example, to support research manuscript compilation, cloud deployment stability, or databases), developers must implement the ``AgentDomainPackage`` trait.

Implementing the AgentDomainPackage Trait
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

A domain package is a Rust struct that implements the ``AgentDomainPackage`` trait. Below is a complete implementation example of a custom research domain package:

.. code-block:: rust

   use perspt_sdk::{
       AgentDomainPackage, DomainDetection, DomainId, DomainScope, EnergyModel,
       ResidualClass, ResidualEvent, ResidualSchema, ResidualWeight, WorkspaceSnapshot,
       CorrectionDirection, EnergyComponent, StabilityClaim
   };

   pub struct ResearchDomain;

   impl AgentDomainPackage for ResearchDomain {
       /// Returns the unique identifier of the domain.
       fn domain_id(&self) -> DomainId {
           DomainId::new("research")
       }

       /// Detects if the current project workspace contains files corresponding to this domain.
       fn detect(&self, workspace: &WorkspaceSnapshot) -> DomainDetection {
           let mut evidence = Vec::new();
           for marker in ["paper.tex", "thesis.tex", "bibliography.bib"] {
               if workspace.has_file_named(marker) {
                   evidence.push(format!("found academic manuscript file: {}", marker));
               }
           }
           let activated = !evidence.is_empty();
           DomainDetection {
               domain: self.domain_id(),
               activated,
               confidence: if activated { 0.90 } else { 0.0 },
               evidence,
           }
       }

       /// Declares the list of residual classes this domain can emit.
       fn residual_schema(&self, _scope: &DomainScope) -> ResidualSchema {
           ResidualSchema::new(vec![
               ResidualClass::Syntax,             // LaTeX syntax check
               ResidualClass::SymbolMismatch,      // Missing citations
               ResidualClass::InterfaceMismatch,   // Broken cross-references
               ResidualClass::Build,              // pdflatex build failures
           ])
       }

       /// Configures the energy model, including tolerances, bounds, and weights.
       fn energy_model(&self, scope: &DomainScope) -> EnergyModel {
           use EnergyComponent::*;
           use ResidualClass::*;

           let weights = vec![
               ResidualWeight::new(Syntax, Syn, 2.0),
               ResidualWeight::new(Build, Syn, 4.0).with_hard_threshold(0.0),
               ResidualWeight::new(SymbolMismatch, Str, 1.5),
               ResidualWeight::new(InterfaceMismatch, Str, 1.0),
           ];

           let mut model = EnergyModel::new("research", 0.10)
               .with_correction_budget(5);
           model.residual_weights = weights;
           model.energy_tolerance = 0.0;
           model.stability_claim = Some(StabilityClaim::not_claimed(format!(
               "research scope: {}",
               scope.label
           )));
           model
       }

       /// Maps residual events to directed correction prompts for the actuator.
       fn correction_directions(&self, residuals: &[ResidualEvent]) -> Vec<CorrectionDirection> {
           let mut directions = Vec::new();
           for r in residuals {
               match r.class {
                   ResidualClass::Syntax => {
                       directions.push(CorrectionDirection {
                           instruction: format!(
                               "Repair LaTeX syntax error: {}. Check brackets and escape characters.",
                               r.message
                           ),
                           ..Default::default()
                       });
                   }
                   ResidualClass::SymbolMismatch => {
                       directions.push(CorrectionDirection {
                           instruction: format!(
                               "Insert missing citation entry in bibliography.bib for reference: {}.",
                               r.message
                           ),
                           ..Default::default()
                       });
                   }
                   _ => {}
               }
           }
           directions
       }
   }

Registering the Domain Adapter
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Once implemented, register your custom package inside the ``DomainRegistry`` at the composition root in ``crates/perspt-cli/src/commands/agent.rs``:

.. code-block:: rust

   let mut domains = perspt_sdk::DomainRegistry::new();
   domains.register(std::sync::Arc::new(ResearchDomain));

An explicit ``--domain`` id wins; otherwise the registry selects the best detection, falling back to the coding domain. Every domain must implement the same interface to maintain compatibility with the ledger and dashboard.

Adding an Agent Tool
--------------------

Tools live under ``crates/perspt-agent/src/tools/`` (catalog and executor in
``mod.rs`` and ``executor.rs``, sandboxing in ``sandbox.rs``, builtin
handlers in ``handlers/``, first-party families in ``families/``). The
extension point is the open execution plane: a *tool family* contributes
catalog entries (``perspt_sdk::ToolEntry``) and registers one
``CandidateToolHandler`` per tool name in the ``CandidateHandlerRegistry``.
The governance wrapper (catalog -> validate -> budget -> certify -> execute
-> re-certify) is uniform and lives outside the handlers.

.. code-block:: rust

   use perspt_agent::{
       CandidateHandlerRegistry, CandidateToolHandler, CandidateWorkspace,
   };
   use perspt_agent::toolloop::EffectOutcome;

   struct MyProbe;

   #[async_trait::async_trait]
   impl CandidateToolHandler for MyProbe {
       async fn apply(
           &self,
           workspace: &CandidateWorkspace,
           call: &perspt_sdk::ProviderToolCall,
           entry: &perspt_sdk::ToolEntry,
       ) -> anyhow::Result<EffectOutcome> {
           // Realize the effect against the reversible candidate.
       }
   }

Register the family at the composition root
(``crates/perspt-cli/src/commands/agent.rs``), through the same public path
the shipped system and DB explorer families use:

.. code-block:: rust

   let mut handlers = CandidateHandlerRegistry::with_builtins();
   handlers.register("my_probe", Arc::new(MyProbe))?;
   runtime = runtime
       .with_tool_family(vec![my_probe_entry()])  // catalog entries
       .with_tool_handlers(handlers);             // execution surface

Duplicate registration fails closed, so a family can never silently shadow
a builtin. Handlers never decide admission, budgets, or certification — the
admissibility kernel already did; they only realize the effect against the
reversible candidate.


Adding a Prompt Section Library
-------------------------------

Actor prompts are typed section libraries under
``crates/perspt-core/prompts/`` (``session_bootstrap``, ``graph_plan``,
``repository_explore``, ``adjudicate``, ``evidence_summarize``). A library
is a directory of ordered Markdown sections with YAML frontmatter:

.. code-block:: markdown

   ---
   id: session_bootstrap/role
   version: 1
   role: system
   required: true
   max_bytes: 256
   vars:
     domain_id: { type: "BoundedText<64>" }
   ---
   You are a governed {{domain_id}} agent.

``perspt-prompt-macros`` compiles the sections at build time
(``frontmatter.rs`` parses, ``validate.rs`` runs the codegen validation
list, ``emit.rs`` generates the section constructors). After any section
change:

1. ``perspt prompts lint`` - run the validation list (``--bundle`` points it
   at an external bundle directory)
2. ``perspt prompts manifest crates/perspt-core/prompts`` - regenerate the
   committed ``manifest.toml`` with fresh content hashes


Adding a Benchmark Corpus Task
------------------------------

The optional evaluation corpus lives in ``crates/perspt-benchmark/corpus/``
(30 hidden-oracle tasks; see its ``README.md``). Each task directory holds:

- ``task.json`` - user goal, shell-free hidden-check argv, expectation, one
  language tag (``rust``, ``python``, ``mixed``), one scale tag, and
  capability tags
- ``fixture/`` - the only tree copied into the agent's workspace
- ``hidden/`` - the oracle overlay copied only after the agent finishes
- ``solution/`` - a withheld reference overlay used only by validation

Validate with the credential-free authoring gate:

.. code-block:: bash

   cargo run -p perspt-cli --features benchmark -- benchmark validate

The gate refuses fewer than 30 tasks, an imbalanced language/scale mix,
insufficient capability coverage, a fixture that already passes its oracle,
or a reference solution that does not pass.


Adding an LLM Provider
-----------------------

The ``genai`` adapter in ``perspt-core/src/llm_provider.rs`` handles providers.
To add a new provider:

1. Add the adapter kind to ``str_to_adapter_kind()``
2. Map the env var in ``new_with_config()``
3. Add the provider's default model to ``detect_provider_from_env()`` in
   ``perspt-core/src/llm_provider.rs``
4. Update the auto-detection priority in config

.. code-block:: rust

   fn str_to_adapter_kind(provider: &str) -> AdapterKind {
       match provider {
           "openai" => AdapterKind::OpenAI,
           "anthropic" => AdapterKind::Anthropic,
           // ... existing providers ...
           "newprovider" => AdapterKind::NewProvider,
           _ => AdapterKind::OpenAI,
       }
   }


Adding a Starlark Policy
--------------------------

Create a ``.star`` file in the policy directory (``~/.config/perspt/policies/``):

.. code-block:: python

   # custom_policy.star

   def check_file_write(path, content):
       # Called before any file write.
       if path.endswith(".env"):
           return deny("Cannot write .env files")
       return allow()

   def check_command(cmd):
       # Called before any command execution.
       if "curl" in cmd and "http://" in cmd:
           return prompt("Insecure HTTP request: " + cmd)
       return allow()

The ``PolicyEngine`` loads all ``.star`` files from the policy directory automatically.
