Introduction to Perspt
======================

.. only:: html

   .. raw:: html

      <div style="text-align: center; margin: 2em 0;">
      <pre style="font-family: monospace; font-size: 0.8em; line-height: 1.2; margin: 0 auto; display: inline-block; text-align: left;">
      ██████╗ ███████╗██████╗ ███████╗██████╗ ████████╗
      ██╔══██╗██╔════╝██╔══██╗██╔════╝██╔══██╗╚══██╔══╝
      ██████╔╝█████╗  ██████╔╝███████╗██████╔╝   ██║
      ██╔═══╝ ██╔══╝  ██╔══██╗╚════██║██╔═══╝    ██║
      ██║     ███████╗██║  ██║███████║██║        ██║
      ╚═╝     ╚══════╝╚═╝  ╚═╝╚══════╝╚═╝        ╚═╝
      </pre>
      <p><em>Your Terminal's Window to the AI World</em></p>
      </div>

.. centered:: **Perspt: Your Terminal's Window to the AI World**

What is Perspt?
---------------

**Perspt** (pronounced "perspect," short for **Per**\ sonal **S**\ pectrum
**P**\ ertaining **T**\ houghts) is a high-performance, terminal-based interface
to Large Language Models (LLMs). It serves two complementary purposes:

1. **A simple command-line interface (CLI) for testing LLM services** - Connect
   to OpenAI, Anthropic, Google Gemini, Groq, Cohere, xAI, DeepSeek, or Ollama
   with a single command. Auto-detect your application programming interface
   (API) key, chat interactively in a beautiful terminal user interface (TUI),
   or pipe responses through the simple-chat mode. Perspt makes it effortless to
   evaluate and compare different LLM providers from your terminal.

2. **An experimental implementation of the SRBN engine** - Perspt's agent mode
    is a practical implementation of the **Stabilized Recursive Barrier Network**
    (SRBN) framework described by the three-paper *Stability is All You Need*
    series. The SRBN engine decomposes coding tasks into directed acyclic graphs
    (DAGs), uses Lyapunov energy as a stability measure through multi-stage
    verification barriers, and commits only when energy converges - applying
    control-theoretic ideas to autonomous code generation. The theoretical
    framework is mature; the implementation is under active development and has
    not yet been benchmarked.

.. admonition:: Version 0.6.1 "AKU" Highlights
   :class: tip

    **Config Coherency, Rich TUI Rendering, and Slash Commands:**

    - **Provider Coherency** - Bound the configured provider cohesively across all modes: TUI, CLI (simple-chat), and SRBN agent. Added robust schema validation for TOML (Tom's Obvious, Minimal Language) configuration files and refined config commands.
    - **TUI and CLI Slash Commands** - Integrated ``rustyline`` command history and slash commands in CLI simple-chat. Added persistent history and UTF-8 safe input navigation in TUI inputs.
    - **Mathematical and Structural Rendering** - Added real-time LaTeX math transpilation and self-wrapping ASCII tables inside the terminal chat UI.
    - **Workspace Upgrades** - Bumped version of all workspace crates to ``0.6.1`` with codename **AKU - sharp fixes from sharp ears**.

.. admonition:: Version 0.6.0 "kukuza" Highlights
   :class: note

   **Ecosystem & Dependency Modernization:**

    - **Major Upgrades** - Bumped core dependencies to their latest major versions including ``duckdb (=1.10503.1)``, ``starlark (0.14)``, ``genai (0.6.1)``, ``askama (0.16)``, and ``diffy (0.5)``.
    - **Enterprise AI Providers** - Integrated native support for **AWS Bedrock** and **Google Agent Platform** (formerly Vertex AI) with advanced Identity and Access Management (IAM) credentials and Open Authorization 2.0 (OAuth2) token authorization.
    - **State-of-the-Art Models** - Added full, out-of-the-box support for the newest generation of models, including ``gpt-5.5`` and ``claude-sonnet-4.7``.

.. admonition:: Version 0.5.9 Highlights
   :class: note

   **Robust Correction Loop Contracts (Perspt Specification Proposal 7, PSP-7):**

    - **Structured Artifact Bundle Format** - Switched correction prompt from free-form output to a strict JavaScript Object Notation (JSON) schema, ensuring the LLM explicitly declares target paths and artifacts.
    - **Typed Parse Pipeline** - Replaced Option-based extraction with a 5-layer fail-closed parse pipeline that classifies retries (Retarget, MalformedRetry, SupportFileViolation, Replan) for intelligent convergence.
    - **Manifest Policy Enforcement** - Added semantic validation to prevent implicit mutation of root manifests unless explicitly requested.
    - **Strict Budget Exhaustion** - Upgraded budget checks to respect step and revision caps alongside cost, preventing runaway loops.

Architecture
------------

Perspt is one program assembled from twelve small Rust libraries. Rust calls
such a library a *crate*. Each crate has one job, and a crate may use another
crate to get its work done.

The crates fall into four groups. The first group faces the user: one crate
reads the commands you type, and another draws the terminal interface. The
second group does the work: one crate talks to the language model, one crate
runs the agent, and one crate stores the history of a session. The third group
keeps that work safe and visible: one crate checks each command against a set of
rules, one crate isolates commands so they cannot escape the project, and one
crate serves a web dashboard that lets you watch the agent run. The fourth group
is a single umbrella crate that ties the others together.

Three of the work crates - ``perspt-sdk``, ``perspt-coding``, and
``perspt-research`` - form a reusable platform. The agent already follows a set
of stability rules every time it changes a file. These three crates package
those rules so that a new field of work, such as research instead of coding, can
reuse the same guarantees without rewriting the engine. The design of this
platform is recorded in Perspt Specification Proposal 8 (PSP-8); the developer
guide describes it in detail.

.. graphviz::
   :align: center
   :caption: Perspt Architecture Overview

   digraph arch {
       rankdir=TB;
       node [shape=box, style="rounded,filled", fontname="Helvetica", fontsize=10];

       subgraph cluster_cli {
           label="User Interface";
           style=dashed;
           cli [label="perspt-cli\n11 commands", fillcolor="#4ECDC4"];
           tui [label="perspt-tui\nTerminal UI", fillcolor="#96CEB4"];
       }

       subgraph cluster_core {
           label="Core Engine";
           style=dashed;
           core [label="perspt-core\nLLM Provider + Types", fillcolor="#45B7D1"];
           agent [label="perspt-agent\nSRBN Engine", fillcolor="#FFEAA7"];
           store [label="perspt-store\nDuckDB Sessions", fillcolor="#B8D4E3"];
       }

       subgraph cluster_sdk {
           label="Reusable SDK Platform";
           style=dashed;
           sdk [label="perspt-sdk\nStability Contract", fillcolor="#AED9E0"];
           coding [label="perspt-coding\nCoding Domain", fillcolor="#C7CEEA"];
           research [label="perspt-research\nResearch Domain", fillcolor="#E2C2C6"];
       }

       subgraph cluster_security {
           label="Security";
           style=dashed;
           policy [label="perspt-policy\nStarlark Rules", fillcolor="#DDA0DD"];
           sandbox [label="perspt-sandbox\nIsolation", fillcolor="#F8B739"];
       }

       subgraph cluster_monitoring {
           label="Monitoring";
           style=dashed;
           dashboard [label="perspt-dashboard\nWeb Dashboard", fillcolor="#FFB6C1"];
       }

       meta [label="perspt\nUmbrella Crate", fillcolor="#D5E8D4"];

       cli -> tui;
       cli -> agent;
       cli -> dashboard;
       agent -> core;
       agent -> store;
       agent -> policy;
       agent -> sandbox;
       agent -> sdk [style=dotted, label="PSP-8"];
       coding -> sdk;
       research -> sdk;
       dashboard -> store;
       dashboard -> core;
       meta -> core [style=dotted];
   }

.. note::

   The 12 crates are: ``perspt-core``, ``perspt-agent``, ``perspt-cli``,
   ``perspt-tui``, ``perspt-store``, ``perspt-policy``, ``perspt-sandbox``,
   ``perspt-dashboard``, ``perspt-sdk``, ``perspt-coding``, ``perspt-research``,
   and the ``perspt`` umbrella crate. See :doc:`concepts/workspace-crates` for a
   crate-by-crate tour.

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
     - Per-session cost tracking with a ``--max-cost`` United States dollar (USD) limit and ``--max-steps`` iteration cap.
   * - **TUI**
     - **Terminal UI**
     - Ratatui-based with markdown rendering, diff viewer, task tree, dashboard,
       review modal, and logs viewer.
   * - **Web**
     - **Dashboard**
     - Browser-based real-time monitoring of agent execution, energy, LLM
       telemetry, sandbox branches, and decision traces.

SRBN: Stabilized Recursive Barrier Network
------------------------------------------

The SRBN engine in Perspt is based on the *Stability is All You Need* paper
series. Paper I introduces the stability certificate for accepted trajectories;
Paper II turns that certificate into an observed harness with descent-gated
acceptance; Paper III lifts the harness into a capability-constrained platform
contract. Together they reformulate LLM agency as a control problem in which
proposed states must be measured, corrected, and recorded before they are
trusted. Key theoretical contributions include:

- **Input-to-State Stability (ISS)** proof showing bounded reasoning errors
  result in bounded system deviation (paper result)
- **Flow Matching Barriers** that project diverging agent trajectories back onto
  the safe manifold (paper result)
- **Adaptive Flow Speculation** for latency reduction via branch prediction
- Theoretical reliability scaling from exponential decay to logarithmic: :math:`O(\log N)` (paper prediction)

Perspt implements this theory as an experimental coding agent and extends it
through PSP-8 toward an SDK-first agent platform. The mathematical framework is
mature; empirical benchmarks on this implementation have not yet been published.

The central paper prediction is that *measured, barrier-gated* acceptance turns
the error growth of a long autonomous run from a quantity that accumulates with
the number of steps :math:`N` into one that grows only logarithmically. The
following chart illustrates the qualitative difference (conceptual, not measured):

.. plot::
   :caption: Conceptual residual-error growth: an unbarriered agent versus the
             SRBN logarithmic prediction (illustrative, not a benchmark).

   import numpy as np
   import matplotlib.pyplot as plt

   N = np.arange(1, 101)
   naive = 0.05 * N                 # O(N): errors accumulate every step
   srbn = 0.05 * np.log(N + 1)      # O(log N): barrier-gated acceptance

   fig, ax = plt.subplots()
   ax.plot(N, naive, label=r'Unbarriered agent  $O(N)$', color='#E53935', lw=2)
   ax.plot(N, srbn, label=r'SRBN prediction  $O(\log N)$', color='#1E88E5', lw=2)
   ax.set_xlabel('Autonomous steps $N$')
   ax.set_ylabel('Expected uncorrected errors')
   ax.set_title('Reliability scaling: accumulation vs. logarithmic bound')
   ax.legend()

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

- :math:`V_{syn}` - LSP diagnostic count (errors plus warnings)
- :math:`V_{str}` - Structural contract violations
- :math:`V_{log}` - Weighted test failures (pytest)
- :math:`V_{boot}` - Bootstrap command exit codes (build, init)
- :math:`V_{sheaf}` - Cross-node consistency failures

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
   * - ``dashboard``
     - Real-time web monitoring
     - ``perspt dashboard``
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
     - Local models - no API key required

Philosophy
----------

.. epigraph::

   | *The keyboard hums, the screen aglow,*
   | *AI's wisdom, a steady flow.*
   | *Through SRBN's loop, stability we find,*
   | *Code that works, refined and aligned.*

  -- The Perspt Manifesto

Perspt embodies the belief that AI tools should be:

- **Accessible** - A simple ``perspt`` command connects you to any LLM provider
- **Fast** - Rust-native performance with async streaming
- **Stable** - Lyapunov energy guides convergence before commit (SRBN agent, based on paper theory)
- **Secure** - Policy-controlled execution with workspace bounds
- **Extensible** - Modular twelve-crate architecture
- **Experimental** - A testbed for control-theoretic approaches to LLM reliability

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

      Understand the twelve-crate design.
