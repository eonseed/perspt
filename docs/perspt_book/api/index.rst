.. _api-index:

API & SDK Reference
===================

Perspt is built as a modular 14-crate Rust workspace (Edition 2024, Rust 1.100+). In addition to the ``perspt`` CLI binary, you can embed the **``perspt``** ``"Omni"`` facade crate (``AgentBuilder`` and ``perspt::prelude::*``) or compose **``perspt-sdk``**, **``perspt-coding``**, **``perspt-research``**, and **``perspt-agent``** directly for coding and non-coding agentic workflows.

Integrating the Perspt Omni SDK
-------------------------------

Add the root ``perspt`` facade crate (or individual crates such as ``perspt-sdk`` and ``perspt-coding``) to your ``Cargo.toml``:

.. code-block:: toml

   [dependencies]
   perspt = { git = "https://github.com/eonseed/perspt" }

Build and run a governed agent across coding, research (``ResearchDomain``), or custom domains with ``AgentBuilder``:

.. code-block:: rust

   use perspt::prelude::*;

   # async fn run() -> anyhow::Result<()> {
   let runtime = AgentBuilder::new(".")
       .research_domain()
       .approval_policy(ApprovalPolicy::Auto)
       .on_ask_user_async(|q| async move { Ok(format!("answered: {q}")) })
       .build()?;
   let _summary = runtime.run("verify paper citations".into()).await?;
   # Ok(())
   # }

You can then score verifier diagnostics with the canonical quadratic Lyapunov energy :math:`V(x) = \sum_{e \in E} w_e \lVert r_e(x) \rVert^2` and evaluate the acceptance gate in your own Rust code:

.. code-block:: rust

   use perspt_coding::CodingDomain;
   use perspt_sdk::{
       domain::{AgentDomainPackage, DomainScope},
       energy::score_candidate,
       gate::{evaluate_gate, GateDecision},
       residual::ResidualEvent,
   };

   fn check_candidate(
       scope: &DomainScope,
       residuals: &[ResidualEvent],
       all_required_checks_passed: bool,
       best_v: f64,
   ) -> perspt_sdk::error::Result<GateDecision> {
       let model = CodingDomain.energy_model(scope);
       let score = score_candidate(&model, residuals)?;
       let hard_pass = all_required_checks_passed
           && score.hard_violations.is_empty()
           && score.blocked.is_empty();
       evaluate_gate(hard_pass, score.total, best_v, model.rho_gate)
   }

The caller must run the required verifiers to determine ``all_required_checks_passed``.
An empty ``hard_violations`` list only means no declared residual threshold was exceeded.
``AcceptedByDescent`` permits intermediate progress; it is not delivery success.
This example scores evidence and evaluates a gate; it does not run tools, authorize effects, or promote files.

.. tip::

   For full Rustdoc-generated API documentation across all workspace crates, open the `Rustdoc API Reference <../../perspt/index.html>`_ or run locally:

   .. code-block:: bash

      cargo doc --open --workspace --no-deps

.. list-table::
   :header-rows: 1
   :widths: 30 70

   * - Crate
     - Responsibility
   * - :doc:`perspt-core`
     - Types, config, LLM provider, events, plugins.
   * - :doc:`perspt-sdk`
     - Domain-neutral SRBN control plane: energy, gates, ledger.
   * - :doc:`perspt-coding`
     - Coding domain package: residuals, adapters, barriers.
   * - :doc:`perspt-research`
     - Research domain package (executable ``citation-check`` & ``correction_packet``).
   * - :doc:`perspt-agent`
     - Governed candidate runtime, tool loop, verifier, work-graph dispatch.
   * - :doc:`perspt-tui`
     - Ratatui terminal UI (chat + agent).
   * - :doc:`perspt-cli`
     - Clap CLI entry point and subcommands.
   * - :doc:`perspt-store`
     - DuckDB session persistence.
   * - :doc:`perspt-policy`
     - Starlark policy engine.
   * - :doc:`perspt-sandbox`
     - Command sandboxing and isolation.
   * - :doc:`perspt-dashboard`
     - Axum + Askama + HTMX web dashboard.
   * - :doc:`perspt-prompt-macros`
     - Build-time compiler for typed prompt section libraries.
   * - :doc:`perspt-benchmark`
     - Optional, feature-gated manual evaluation tooling.
   * - ``perspt``
     - Root ``"Omni"`` facade crate (``AgentBuilder``, ``prelude``, subcrate re-exports).

.. toctree::
   :maxdepth: 2
   :hidden:

   perspt-core
   perspt-sdk
   perspt-coding
   perspt-research
   perspt-agent
   perspt-tui
   perspt-cli
   perspt-store
   perspt-policy
   perspt-sandbox
   perspt-dashboard
   perspt-prompt-macros
   perspt-benchmark
