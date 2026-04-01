Introduction to Perspt
======================

.. raw:: html

   <div style="text-align: center; margin: 2em 0;">
   <pre style="font-family: monospace; font-size: 0.8em; line-height: 1.2; margin: 0 auto; display: inline-block;">
   ██████╗ ███████╗██████╗ ███████╗██████╗ ████████╗
   ██╔══██╗██╔════╝██╔══██╗██╔════╝██╔══██╗╚══██╔══╝
   ██████╔╝█████╗  ██████╔╝███████╗██████╔╝   ██║
   ██╔═══╝ ██╔══╝  ██╔══██╗╚════██║██╔═══╝    ██║
   ██║     ███████╗██║  ██║███████║██║        ██║
   ╚═╝     ╚══════╝╚═╝  ╚═╝╚══════╝╚═╝        ╚═╝
   </pre>
   <p><em>Your Terminal's Window to the AI World</em></p>
   </div>

What is Perspt?
---------------

**Perspt** (pronounced "perspect," short for **Per**\ sonal **S**\ pectrum
**P**\ ertaining **T**\ houghts) is a high-performance, terminal-based interface
to Large Language Models. It serves two complementary purposes:

1. **A simple CLI for testing LLM services** — Connect to OpenAI, Anthropic,
   Google Gemini, Groq, Cohere, xAI, DeepSeek, or Ollama with a single command.
   Auto-detect your API key, chat interactively in a beautiful TUI, or pipe
   responses through the simple-chat mode. Perspt makes it effortless to
   evaluate and compare different LLM providers from your terminal.

2. **An experimental implementation of the SRBN engine** — Perspt's agent mode
   is a practical implementation of the **Stabilized Recursive Barrier Network
   (SRBN)** framework described in the paper *"Stability is All You Need:
   Lyapunov-Guided Hierarchies for Long-Horizon LLM Reliability"* by
   **Vikrant R. and Ronak R.** (pre-publication). The SRBN engine decomposes
   coding tasks into DAGs, uses Lyapunov energy as a stability measure through
   multi-stage verification barriers, and commits only when energy converges —
   applying control-theoretic ideas to autonomous code generation. The theoretical
   framework is mature; the implementation is under active development and has not
   yet been benchmarked.

.. admonition:: Version 0.5.6 "ikigai 生き甲斐" Highlights
   :class: tip

   **LLM CLI:**

   - **Multi-Provider Chat** — 8 providers with zero-config auto-detection
   - **Beautiful TUI** — Markdown rendering, streaming responses, scroll navigation
   - **Simple CLI Mode** — Pipe-friendly ``simple-chat`` for scripting and logging

   **SRBN Agent (Experimental):**

   - **PSP-5 Runtime** — Project-first multi-file execution with ownership closure
   - **Five-Component Energy** — V(x) = alpha * V_syn + beta * V_str + gamma * V_log + V_boot + V_sheaf
   - **Plugin-Driven Verification** — Language plugins select LSP server, test runner, and init commands
   - **Sheaf Validation** — Cross-node consistency checks after all nodes converge
   - **Provisional Branches** — Speculative child-node execution isolated until parent commits
   - **Headless Mode** — ``--yes`` flag for fully autonomous CI/CD operation
   - **Session Resume** — ``perspt resume`` rehydrates energy, retries, and escalation state

Architecture
------------

Perspt is built as a **7-crate Rust workspace**:

.. graphviz::
   :align: center
   :caption: Perspt Architecture Overview

   digraph arch {
       rankdir=TB;
       node [shape=box, style="rounded,filled", fontname="Helvetica", fontsize=10];

       subgraph cluster_cli {
           label="User Interface";
           style=dashed;
           cli [label="perspt-cli\n10 commands", fillcolor="#4ECDC4"];
           tui [label="perspt-tui\nTerminal UI", fillcolor="#96CEB4"];
       }

       subgraph cluster_core {
           label="Core Engine";
           style=dashed;
           core [label="perspt-core\nLLM Provider + Types", fillcolor="#45B7D1"];
           agent [label="perspt-agent\nSRBN Engine", fillcolor="#FFEAA7"];
           store [label="perspt-store\nDuckDB Sessions", fillcolor="#B8D4E3"];
       }

       subgraph cluster_security {
           label="Security";
           style=dashed;
           policy [label="perspt-policy\nStarlark Rules", fillcolor="#DDA0DD"];
           sandbox [label="perspt-sandbox\nIsolation", fillcolor="#F8B739"];
       }

       cli -> tui;
       cli -> agent;
       agent -> core;
       agent -> store;
       agent -> policy;
       agent -> sandbox;
   }

Key Features
------------

.. list-table::
   :widths: 5 25 70
   :class: borderless

   * - **SRBN**
     - **Agent Mode**
     - Experimental autonomous multi-file coding. Plans a task DAG, generates artifact bundles per node,
       verifies with LSP + tests, retries until energy converges, and commits to the ledger.
   * - **LLM**
     - **Multi-Provider**
     - OpenAI, Anthropic, Google Gemini, Groq, Cohere, XAI, DeepSeek, and Ollama (local).
       Zero-config auto-detection from environment variables.
   * - **LSP**
     - **Sensor Architecture**
     - Plugin-driven LSP selection: ``rust-analyzer`` for Rust, ``ty`` or ``pyright`` for Python,
       ``typescript-language-server`` for JS/TS, ``gopls`` for Go. Computes V_syn.
   * - **Test**
     - **Test Runner**
     - pytest integration with weighted failure scoring. Critical tests carry weight 10,
       high-priority 3, low-priority 1. Produces V_log.
   * - **Ledger**
     - **Merkle Ledger**
     - DuckDB-backed cryptographic change tracking. Supports rollback, session resume,
       energy history, and escalation reports.
   * - **Policy**
     - **Security**
     - Starlark policy engine validates commands before execution. Workspace-bound
       enforcement prevents escaping the project directory.
   * - **Budget**
     - **Token Budget**
     - Per-session cost tracking with ``--max-cost`` USD limit and ``--max-steps`` iteration cap.
   * - **TUI**
     - **Terminal UI**
     - Ratatui-based with markdown rendering, diff viewer, task tree, dashboard,
       review modal, and logs viewer.

SRBN: Stabilized Recursive Barrier Network
------------------------------------------

The SRBN engine in Perspt is based on the paper *"Stability is All You Need:
Lyapunov-Guided Hierarchies for Long-Horizon LLM Reliability"* by **Vikrant R.
and Ronak R.** (pre-publication). The paper introduces a topological framework
that reformulates LLM agency as a sheaf-theoretic control problem, replacing
probabilistic search with Lyapunov stability analysis. Key theoretical
contributions include:

- **Input-to-State Stability (ISS)** proof showing bounded reasoning errors
  result in bounded system deviation (paper result)
- **Flow Matching Barriers** that project diverging agent trajectories back onto
  the safe manifold (paper result)
- **Adaptive Flow Speculation** for latency reduction via branch prediction
- Theoretical reliability scaling from exponential decay to logarithmic: :math:`O(\log N)` (paper prediction)

Perspt implements this theory as an experimental coding agent, governed by PSP-5.
The mathematical framework is mature; empirical benchmarks on this implementation
have not yet been published.

.. graphviz::
   :align: center
   :caption: SRBN Control Flow (PSP-5)

   digraph srbn {
       rankdir=LR;
       node [shape=box, style="rounded,filled", fontname="Helvetica", fontsize=10];

       detect [label="Detect\nPlugins", fillcolor="#E0F7FA"];
       plan [label="Plan\n(Architect)", fillcolor="#E8F5E9"];
       gen [label="Generate\n(Actuator)", fillcolor="#FFF3E0"];
       verify [label="Verify\n(LSP+Tests)", fillcolor="#F3E5F5"];
       check [label="V(x) < e?", shape=diamond, fillcolor="#FFECB3"];
       sheaf [label="Sheaf\nValidation", fillcolor="#E8EAF6"];
       commit [label="Commit\n(Ledger)", fillcolor="#C8E6C9"];

       detect -> plan;
       plan -> gen;
       gen -> verify;
       verify -> check;
       check -> gen [label="retry", style=dashed, color="#E53935"];
       check -> sheaf [label="stable"];
       sheaf -> commit;
   }

**Lyapunov Energy**:

.. math::

   V(x) = \alpha \cdot V_{syn} + \beta \cdot V_{str} + \gamma \cdot V_{log} + V_{boot} + V_{sheaf}

- **V_syn** — LSP diagnostic count (errors + warnings)
- **V_str** — Structural contract violations
- **V_log** — Weighted test failures (pytest)
- **V_boot** — Bootstrap command exit codes (build, init)
- **V_sheaf** — Cross-node consistency failures

Default weights: alpha = 1.0, beta = 0.5, gamma = 2.0. Convergence threshold: epsilon = 0.10.

CLI Commands
------------

.. list-table::
   :header-rows: 1
   :widths: 15 45 40

   * - Command
     - Description
     - Example
   * - ``chat``
     - Interactive TUI chat (default)
     - ``perspt chat --model gemini-pro-latest``
   * - ``agent``
     - Autonomous multi-file coding (experimental)
     - ``perspt agent "Create a REST API in Rust"``
   * - ``init``
     - Initialize project config
     - ``perspt init --memory --rules``
   * - ``config``
     - View or edit configuration
     - ``perspt config --show``
   * - ``ledger``
     - Query Merkle change history
     - ``perspt ledger --recent``
   * - ``status``
     - Session lifecycle and energy
     - ``perspt status``
   * - ``abort``
     - Cancel running session
     - ``perspt abort``
   * - ``resume``
     - Resume interrupted session
     - ``perspt resume --last``
   * - ``logs``
     - View LLM request logs
     - ``perspt logs --tui``
   * - ``simple-chat``
     - CLI chat without TUI
     - ``perspt simple-chat``

Supported Providers
-------------------

.. list-table::
   :header-rows: 1
   :widths: 20 30 50

   * - Provider
     - Environment Variable
     - Notes
   * - OpenAI
     - ``OPENAI_API_KEY``
     - GPT-4o, o3-mini, o1-preview, GPT-4.1
   * - Anthropic
     - ``ANTHROPIC_API_KEY``
     - Claude Sonnet 4, Claude Opus 4
   * - Google
     - ``GEMINI_API_KEY``
     - Gemini 2.5 Pro/Flash, Gemini 3.1 Pro/Flash
   * - Groq
     - ``GROQ_API_KEY``
     - Llama-based models (ultra-fast inference)
   * - Cohere
     - ``COHERE_API_KEY``
     - Command R+
   * - XAI
     - ``XAI_API_KEY``
     - Grok
   * - DeepSeek
     - ``DEEPSEEK_API_KEY``
     - DeepSeek Coder
   * - Ollama
     - *(none)*
     - Local models — no API key required

Philosophy
----------

.. epigraph::

   | *The keyboard hums, the screen aglow,*
   | *AI's wisdom, a steady flow.*
   | *Through SRBN's loop, stability we find,*
   | *Code that works, refined and aligned.*

   — The Perspt Manifesto

Perspt embodies the belief that AI tools should be:

- **Accessible** — A simple ``perspt`` command connects you to any LLM provider
- **Fast** — Rust-native performance with async streaming
- **Stable** — Lyapunov energy guides convergence before commit (SRBN agent, based on paper theory)
- **Secure** — Policy-controlled execution with workspace bounds
- **Extensible** — Modular 7-crate architecture
- **Experimental** — A testbed for control-theoretic approaches to LLM reliability

Next Steps
----------

.. grid:: 3
   :gutter: 3

   .. grid-item-card:: Quick Start
      :link: quickstart
      :link-type: doc

      Get running in 5 minutes.

   .. grid-item-card:: Agent Mode
      :link: tutorials/agent-mode
      :link-type: doc

      Autonomous multi-file coding.

   .. grid-item-card:: Architecture
      :link: developer-guide/architecture
      :link-type: doc

      Understand the 7-crate design.
