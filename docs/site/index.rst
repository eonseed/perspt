.. meta::
   :description: Perspt is a fast Rust AI toolkit for developers featuring a pipe-friendly Simple CLI, an interactive Terminal UI, a compiler-verified SRBN Coding Agent, and an embeddable Rust SDK.

Perspt
======

.. container:: hero

   .. container:: hero-copy

      .. rst-class:: eyebrow

      $_ PERSPT // CLI · TUI · SRBN AGENT · RUST SDK

      .. raw:: html

         <h1>Simple CLI. Terminal UI.<br><em>Code that actually runs.</em></h1>

      .. rst-class:: lead

      Perspt brings Large Language Models directly into your terminal workflow in one fast Rust binary: **1. a pipe-friendly Simple CLI**, **2. an interactive Terminal UI**, **3. an autonomous SRBN Coding Agent** that verifies its own code with your compiler and tests, and **4. a Rust SDK** to integrate into your own apps.

      .. container:: actions

         `Read the Book <mount://book/index.html>`_
         `5-Min Quick Start <mount://book/quickstart.html>`_
         `Rust SDK Guide <mount://book/api/index.html>`_
         `Download PDF <mount://pdf/perspt-book-en.pdf>`_

      .. rst-class:: hero-note

      100% Rust · Zero Python dependencies · OpenAI, Anthropic, Gemini, Vertex AI, Ollama, Groq, DeepSeek, xAI & Cohere.

      Linux, macOS, and Windows · `GitHub <https://github.com/eonseed/perspt>`_ · `Rustdoc API <perspt/index.html>`_ · `PSP Catalog <mount://psps/index.html>`_.

   .. container:: hero-art

      .. image:: ../shared/_static/cover-art.svg
         :alt: Terminal Spectrum and SRBN Verification Prism showing Simple CLI, Terminal UI, and SRBN Agent converging to V(x) = 0.

      .. rst-class:: art-caption

      HÓZHÓ TERMINAL PRECISION // CLI · TUI · SRBN · SDK

.. container:: pillars

   .. container:: pillar

      .. rubric:: 01 // SIMPLE CLI (``perspt simple-chat``)

      **Plain terminal output with zero UI chrome.**
      Ask a quick question in the terminal, pipe compiler errors or ``git diff`` straight from ``stdin``, and redirect clean text output into shell scripts and Unix pipelines.

      .. code-block:: sh

         git diff | perspt simple-chat "Write a commit message"

   .. container:: pillar

      .. rubric:: 02 // TERMINAL UI (``perspt`` / ``perspt chat``)

      **Interactive full-screen chat right in your terminal.**
      Stream Markdown, syntax-highlighted code blocks, tables, and LaTeX math; switch models on the fly across 9+ cloud or local Ollama providers; and inspect read-only MCP tools with ``/mcp``.

      .. code-block:: sh

         perspt chat --model gemini-3.5-flash

   .. container:: pillar

      .. rubric:: 03 // SRBN CODING AGENT (``perspt agent``)

      **Autonomous coding that compiles and tests its own work.**
      Give Perspt a coding task and a project folder. It plans the work, edits files in a safe sandbox, runs your real compiler, LSP, and test suite, and fixes errors automatically before committing.

      .. code-block:: sh

         perspt agent -w ./app "Add CSV export with unit tests"

   .. container:: pillar

      .. rubric:: 04 // EMBEDDABLE RUST SDK (``perspt-sdk``)

      **Integrate verified AI workflows into your own Rust apps.**
      Embed ``perspt-sdk``, ``perspt-coding``, and ``perspt-agent`` as library crates to use Perspt's verification gate, capability sandbox, and DuckDB audit ledger in custom software.

      .. code-block:: sh

         cargo add perspt-sdk perspt-coding

.. container:: section-heading

   .. rst-class:: eyebrow

   UNDER THE HOOD: WHY THE CODING AGENT WORKS

   .. rubric:: SRBN keeps generated code running and aligned with your task.

Most AI coding agents generate code in an open loop: if the LLM makes a small mistake in one file, subsequent edits build on top of that mistake until the project no longer compiles. Internally, Perspt's coding agent is powered by **SRBN (Stabilized Recursive Barrier Networks)**, an engineering architecture based on the *Stability is All You Need* research paper series.

Instead of trusting the LLM's first guess, SRBN runs every candidate edit through your actual language toolchain (``cargo check``, ``rust-analyzer``, ``pytest``, ``pyright``, ``tsc``) and scores remaining compiler, contract, and test issues as a single error energy :math:`V(x) = \sum_{e \in E} w_e \|r_e(x)\|^2`. Changes are only accepted when all checks pass or measured errors strictly decrease (:math:`V(y) \le V_{\mathrm{best}} - \rho_{\mathrm{gate}}`). Bad edits are rolled back automatically, so the agent stays locked on working code that matches the task you gave it.

.. figure:: ../shared/_static/front-page-abm.svg
   :alt: Agent-based simulation comparing unguided LLM drift against SRBN Lyapunov-guided trajectory convergence.
   :width: 100%

   Why closed-loop verification matters over multi-step tasks: unverified LLM loops drift as errors compound across steps, while Perspt's SRBN verification loop tests and corrects each step until the code compiles and passes tests (:math:`V(x) = 0`).

.. container:: split-section

   .. container:: split-copy

      .. rst-class:: eyebrow

      SAFE SANDBOX & FULL REPLAY

      .. rubric:: Isolated workspace edits, real compiler feedback, and instant rollback.

      When you run ``perspt agent``, the LLM never writes directly to your working tree. It proposes changes inside an isolated candidate workspace while Perspt's Starlark policy engine and OS sandbox (Bubblewrap on Linux, ``sandbox-exec`` on macOS) block unsafe commands.

      Every verified step is recorded in a local DuckDB ledger, so you can watch progress in the live web dashboard (``--dashboard``), replay any session offline (``perspt replay``), or roll back promoted changes with a single command (``perspt ledger --rollback``).

      `Explore How Agent Mode Works <mount://book/concepts/srbn-architecture.html>`_

   .. image:: ../shared/_static/srbn-control-sequence.svg
      :alt: Sequence diagram of the SRBN governed control loop across User, Orchestrator, Architect, Actuator, Admissibility Kernel, Verifier Suite, and Merkle Ledger.

.. container:: section-heading

   .. rst-class:: eyebrow

   MODULAR 14-CRATE WORKSPACE & SDK

   .. rubric:: Use Perspt as a CLI tool or embed the Rust SDK in your own stack.

Perspt is engineered as a modular 14-crate Rust workspace around **``perspt-sdk``**. You can embed the verification loop, capability sandbox, provider portfolio, and DuckDB audit ledger directly into your own Rust applications, or build custom domain agents alongside ``perspt-coding`` and ``perspt-research``.

.. code-block:: rust

   use perspt_coding::CodingDomain;
   use perspt_sdk::{
       domain::AgentDomainPackage,
       energy::{score_candidate, EnergyModel},
       gate::evaluate_gate,
   };

   // Score compiler/LSP/test diagnostics and check the acceptance gate:
   let domain = CodingDomain;
   let model: EnergyModel = domain.energy_model(&scope);
   let score = score_candidate(&residuals, &model)?;
   let decision = evaluate_gate(score.hard_violations.is_empty(), score.total, best_v, model.rho_gate)?;

.. figure:: ../shared/_static/workspace-architecture.svg
   :alt: Layered architecture of the fourteen crates in the Perspt Rust workspace.
   :width: 100%

   The 14-crate Perspt workspace: presentation surfaces (Simple CLI, TUI, Web Dashboard) drive the SRBN agent runtime over ``perspt-sdk``, security & DuckDB persistence engines, and 9+ LLM providers.

.. container:: book-section

   .. image:: ../shared/_static/book-cover.png
      :alt: Cover of The Perspt Book: Simple CLI. Terminal UI. Code that actually runs.

   .. container:: book-copy

      .. rst-class:: eyebrow

      DEVELOPER HANDBOOK & PSP ARCHIVE

      .. rubric:: Clear developer guides, SDK references, and engineering specs.

      *The Perspt Book* walks you through installing Perspt, configuring cloud or local Ollama models, piping commands with ``simple-chat``, chatting in the TUI, running ``perspt agent`` on real projects, and embedding ``perspt-sdk`` in Rust.

      Alongside the book, the **Perspt Specification Proposals (PSPs)** archive documents the complete engineering design and mathematical foundations behind the SRBN runtime.

      .. container:: actions

         `Start Reading the Book <mount://book/index.html>`_
         `Open the PDF Book <mount://pdf/perspt-book-en.pdf>`_
         `Browse PSP Catalog <mount://psps/index.html>`_

.. container:: text-links

   `Quick Start <mount://book/quickstart.html>`_
   `Simple CLI & TUI Guide <mount://book/getting-started.html>`_
   `Coding Agent Guide <mount://book/user-guide/agent-mode.html>`_
   `Rust SDK Reference <mount://book/api/index.html>`_
   `Real-Time Dashboard <mount://book/user-guide/dashboard.html>`_
   `MCP Integration <mount://book/user-guide/mcp.html>`_
   `Rustdoc API <perspt/index.html>`_

Get Started in 60 Seconds
-------------------------

Build Perspt from source with Cargo and try all three modes right away:

.. code-block:: sh

   git clone https://github.com/eonseed/perspt.git
   cd perspt
   cargo build --release

   export GEMINI_API_KEY="your-api-key"   # Or OPENAI_API_KEY, ANTHROPIC_API_KEY, or local Ollama

   # 1. Simple CLI: plain terminal output for quick questions and Unix pipes
   cat Cargo.toml | ./target/release/perspt simple-chat "Summarize the workspace crates"

   # 2. Interactive Terminal UI: full-screen chat with Markdown, code, and model switching
   ./target/release/perspt chat

   # 3. SRBN Coding Agent: plan, write, compile, and test code in a project directory
   ./target/release/perspt agent -w ./my-app "Build a CLI calculator in Rust with unit tests"
