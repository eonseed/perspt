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
   <p><em>Your Terminal's Window to the AI World 🤖</em></p>
   </div>

What is Perspt?
---------------

**Perspt** (pronounced "perspect," short for **Per**\ sonal **S**\ pectrum **P**\ ertaining **T**\ houghts) is a 
high-performance, terminal-based interface to Large Language Models with **autonomous coding capabilities**.

.. admonition:: Version 0.5.0 Highlights
   :class: tip

   - **SRBN Agent Mode** — Autonomous coding with Lyapunov stability guarantees
   - **7-Crate Architecture** -- Modular, extensible workspace design
   - **LSP Integration** — Real-time type checking with ``ty`` server
   - **Latest Models** — GPT-5.2, Claude Opus 4.5, Gemini 3

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
           core [label="perspt-core\nLLM Provider", fillcolor="#45B7D1"];
           agent [label="perspt-agent\nSRBN Engine", fillcolor="#FFEAA7"];
           store [label="perspt-store\nSession DB", fillcolor="#B8D4E3"];
       }
       
       subgraph cluster_security {
           label="Security";
           style=dashed;
           policy [label="perspt-policy\nPolicy Engine", fillcolor="#DDA0DD"];
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

   * - 🤖
     - **SRBN Agent Mode**
     - Autonomous coding with stability guarantees. Decomposes tasks, generates code, verifies with LSP.
   * - 🔌
     - **Multi-Provider**
     - OpenAI GPT-5.2, Anthropic Claude Opus 4.5, Google Gemini 3, Groq, Cohere, XAI, DeepSeek, Ollama.
   * - 🔬
     - **LSP Integration**
     - Real-time Python type checking using ``ty`` server. Computes syntax energy V_syn.
   * - 🧪
     - **Test Runner**
     - pytest integration with weighted failure scoring for logic energy V_log.
   * - 📝
     - **Merkle Ledger**
     - Git-style change tracking with rollback support.
   * - 🔒
     - **Security**
     - Starlark policy rules and command sanitization.
   * - 💰
     - **Token Budget**
     - Cost tracking with per-request limits.
   * - 🎨
     - **Beautiful TUI**
     - Ratatui-based with markdown rendering, diff viewer, task tree.

SRBN: Stabilized Recursive Barrier Network
------------------------------------------

The core innovation in Perspt v0.5.0 is the SRBN control loop:

.. graphviz::
   :align: center
   :caption: SRBN Control Flow

   digraph srbn {
       rankdir=LR;
       node [shape=box, style="rounded,filled", fontname="Helvetica", fontsize=10];
       
       task [label="Task", shape=ellipse, fillcolor="#E3F2FD"];
       sheaf [label="Sheafify\n(Architect)", fillcolor="#E8F5E9"];
       spec [label="Speculate\n(Actuator)", fillcolor="#FFF3E0"];
       verify [label="Verify\n(LSP + Tests)", fillcolor="#F3E5F5"];
       check [label="V(x) > ε?", shape=diamond, fillcolor="#FFECB3"];
       commit [label="Commit\n(Ledger)", fillcolor="#C8E6C9"];
       
       task -> sheaf;
       sheaf -> spec;
       spec -> verify;
       verify -> check;
       check -> spec [label="retry", style=dashed, color="#E53935"];
       check -> commit [label="stable"];
   }

**Lyapunov Energy**: V(x) = α·V_syn + β·V_str + γ·V_log

- **V_syn**: LSP diagnostics (errors, warnings)
- **V_str**: Structural analysis
- **V_log**: Test failures (weighted by criticality)

CLI Commands
------------

.. list-table::
   :header-rows: 1
   :widths: 15 45 40

   * - Command
     - Description
     - Example
   * - ``chat``
     - Interactive TUI
     - ``perspt chat``
   * - ``agent``
     - Autonomous coding
     - ``perspt agent "create calculator"``
   * - ``init``
     - Project setup
     - ``perspt init --memory``
   * - ``config``
     - Configuration
     - ``perspt config --show``
   * - ``ledger``
     - Change history
     - ``perspt ledger --recent``
   * - ``status``
     - Agent status
     - ``perspt status``
   * - ``abort``
     - Cancel session
     - ``perspt abort``
   * - ``resume``
     - Resume session
     - ``perspt resume``
   * - ``logs``
     - View LLM logs
     - ``perspt logs --tui``
   * - ``simple-chat``
     - Simple CLI mode
     - ``perspt simple-chat``

Supported Providers
-------------------

.. list-table::
   :header-rows: 1
   :widths: 20 30 50

   * - Provider
     - Environment Variable
     - Models (2025)
   * - OpenAI
     - ``OPENAI_API_KEY``
     - GPT-5.2, o3-mini, o1-preview
   * - Anthropic
     - ``ANTHROPIC_API_KEY``
     - Claude Opus 4.5
   * - Google
     - ``GEMINI_API_KEY``
     - Gemini 3 Flash, Gemini 3 Pro
   * - Groq
     - ``GROQ_API_KEY``
     - Llama 3.x (ultra-fast)
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
     - Local models

Philosophy
----------

.. epigraph::

   | *The keyboard hums, the screen aglow,*
   | *AI's wisdom, a steady flow.*
   | *Through SRBN's loop, stability we find,*
   | *Code that works, refined and aligned.*

   — The Perspt Manifesto

Perspt embodies the belief that AI tools should be:

- **Fast** — Rust-native performance
- **Stable** — Lyapunov-guaranteed convergence  
- **Secure** — Policy-controlled execution
- **Extensible** — Modular crate architecture

Next Steps
----------

.. grid:: 3
   :gutter: 3

   .. grid-item-card:: 🚀 Quick Start
      :link: quickstart
      :link-type: doc

      Get running in 5 minutes.

   .. grid-item-card:: 🤖 Agent Mode
      :link: tutorials/agent-mode
      :link-type: doc

      Autonomous code generation.

   .. grid-item-card:: 📖 Architecture
      :link: developer-guide/architecture
      :link-type: doc

      Understand the 7-crate design.
