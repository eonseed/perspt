.. _developer-guide-extending:

Extending Perspt
================

Adding a Language Plugin
------------------------

Language plugins implement the ``LanguagePlugin`` trait from ``perspt-core``.

**The built-in registry contains Rust, Python, and JavaScript/TypeScript.** Go is an example of an extension we could add, not a shipped plugin.

1. Start from the trait in ``crates/perspt-core/src/plugin/contract.rs`` and an existing implementation such as ``plugin/js.rs``.
2. Implement detection, LSP configuration, initialization and tooling actions, test and run commands, and the verifier profile.
3. Declare real availability checks and explicit no-op stages. A missing required stage must block a hard pass.
4. Register the plugin in ``PluginRegistry`` at the composition root and add the corresponding structured adapter to ``CodingAdapterRegistry``.
5. Test detection, diagnostics, missing tools, and fail-before/pass-after verification with a local fixture.

Designing Domain Adapters
--------------------------

The platform SDK establishes a modular separation of concerns: the core SRBN control plane (in ``perspt-sdk``) owns scheduling, residual scoring, capability checking, ledger tracking, and telemetry monitoring, while the domain adapter owns the domain-specific logic.

To write a custom domain extension (for example, to support research manuscript compilation, cloud deployment stability, or databases), developers must implement the ``AgentDomainPackage`` trait.

Implementing the AgentDomainPackage Trait
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

A package implements ``AgentDomainPackage`` from ``crates/perspt-sdk/src/domain.rs``.
We can inspect a complete, small implementation in ``crates/perspt-research/src/lib.rs``:

.. code-block:: rust

   use perspt_research::ResearchDomain;
   use perspt_sdk::{AgentDomainPackage, DomainScope, WorkspaceSnapshot};

   let domain = ResearchDomain::new();
   let workspace = WorkspaceSnapshot::new(
       "/work/paper",
       vec!["references.bib".into()],
   );
   assert!(domain.detect(&workspace).activated);
   let model = domain.energy_model(&DomainScope::default());
   model.validate()?;

The required methods declare the domain identity, detection evidence, residual schema, energy model, correction directions, tool entries, hard-gate policy, verifier suite, and safety-barrier position.
Every emitted consistency residual needs an explicit positive weight.
Use ``CorrectionDirection::new(class, instruction)`` to construct a correction from structured residual evidence.

**A contract declaration does not implement a verifier.** The research package is a skeleton that declares a ``citation-check`` stage; source discovery and citation verification are not complete research workflows.
A new domain also needs executable measurement and effect handlers wired into the runtime.
We leave analytic stability and safety claims undeclared unless we can supply their required evidence.

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

The ``genai`` adapter in ``crates/perspt-core/src/llm_provider.rs`` handles providers.
To add a new provider:

1. Add the adapter kind to ``str_to_adapter_kind()``
2. Map the env var in ``new_with_config()``
3. Add the provider's default model to ``detect_provider_from_env()`` in
   ``perspt-core/src/llm_provider.rs``
4. Update the auto-detection priority in config

Unknown provider identifiers must remain errors; a new adapter must not introduce an implicit fallback to another provider.
Update explicit route resolution, environment detection, and the portfolio tests together.

Adding a Starlark Policy
--------------------------

Create a ``.star`` file under ``rules/`` in the platform configuration directory.
Each file defines ``evaluate(command)`` and returns ``"allow"``, ``"prompt"``, or ``"deny"``:

.. code-block:: python

   # Starlark, not a Python runtime hook.
   def evaluate(command):
       if matches_pattern(command, "sudo"):
           return "deny"
       return "allow"

The engine evaluates command policies; it does not call invented file-write hooks.
Capability and workspace checks govern file effects separately.
See :doc:`../howto/security-rules` for policy loading and precedence.
