.. Perspt documentation master file

.. title:: Perspt: The AI Workspace for Your Terminal

.. only:: html

   .. raw:: html

      <div style="text-align: center; margin: 2em 0 1.2em;">
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

.. only:: latex

   Perspt: The AI Workspace for Your Terminal
   ==========================================

.. epigraph::

   | *The keyboard hums, the screen aglow,*
   | *AI's wisdom, a steady flow.*
   |
   | *Will robots take over, it's quite the fright,*
   | *Or just provide insights, day and night?*
   |
   | *We ponder and chat, with code as our guide,*
   | *Is AI our helper or our human pride?*

**Build with AI in Your Terminal — Ship Code Verified by Your Compiler**

Perspt (*Personal Spectrum Pertaining Thoughts*) is a fast, single-binary Rust toolkit and embeddable SDK that brings 9+ cloud and local Large Language Models directly into your command-line workflow.

Instead of forcing you into a single interaction style, Perspt gives you **three ways to work in your terminal** plus a **modular Rust SDK**:

- **Simple CLI** (``perspt simple-chat``) — Stream plain text straight to ``stdout`` with zero UI chrome. Pipe files, compiler logs, or ``git diff`` from ``stdin`` into shell scripts and Unix pipelines.
- **Interactive Terminal UI** (``perspt`` / ``perspt chat``) — Chat in a full-screen ``ratatui`` workspace with streaming Markdown, syntax highlighting, tables, LaTeX math, conversation history, read-only MCP tools (``/mcp``), and live model switching.
- **SRBN Coding Agent** (``perspt agent``) — Delegate multi-file tasks to an autonomous coding agent that edits an isolated sandbox copy of your project and runs your real compiler, LSP, and test suite to fix its own mistakes before committing.
- **Embeddable Rust SDK** (``perspt-sdk``, ``perspt-coding``, ``perspt-research``) — Integrate Perspt's verification gate, capability sandbox, multi-provider router, and DuckDB audit ledger directly into your own Rust applications.

.. figure:: ../shared/_static/front-page-abm.svg
   :alt: Agent-based model trajectory toward consensus under SRBN verification
   :width: 100%

   Why ``perspt agent`` stays reliable across multi-step tasks: unguided LLM loops drift as small mistakes compound, whereas Perspt's internal SRBN verification loop checks every step against your compiler and test suite until errors reach zero (:math:`V(x) = 0`).

.. note:: What is SRBN?

   **SRBN (Stabilized Recursive Barrier Networks)** is the internal reliability engine inside ``perspt agent``, based on the *Stability is All You Need* research paper series. You never need to configure or know the math to use ``perspt agent``—SRBN works automatically behind the scenes by running your compiler, language server, and tests after each edit, scoring remaining errors as :math:`V(x)`, and discarding bad guesses so the code Perspt generates actually compiles and matches your task.

.. only:: html

   .. list-table::
      :header-rows: 1
      :widths: 28 72

      * - Start Here
        - What You Will Learn
      * - :doc:`quickstart`
        - Install Perspt and run the Simple CLI, Terminal UI, and Coding Agent in 5 minutes.
      * - :doc:`introduction`
        - Understand the three developer modes, the Rust SDK, and how SRBN keeps code reliable.
      * - :doc:`tutorials/agent-mode`
        - Step-by-step walkthrough of autonomous multi-file coding with ``perspt agent``.
      * - :doc:`api/index`
        - Embed ``perspt-sdk`` and ``perspt-coding`` into your own Rust applications.
      * - :doc:`user-guide/dashboard`
        - Monitor live agent runs, compiler/test checks, and diffs in your browser.

   What's Included
   ---------------

   .. list-table::
      :widths: 24 76
      :class: borderless

      * - **1. Simple CLI**
        - ``perspt simple-chat`` streams raw text to your terminal with no UI chrome—ideal for Unix pipes, shell scripts, and quick questions.
      * - **2. Terminal UI**
        - ``perspt chat`` launches a rich ``ratatui`` interface with Markdown, code highlighting, math rendering, clipboard paste, and ``/mcp`` tool inspection.
      * - **3. SRBN Coding Agent**
        - ``perspt agent`` edits code in a safe candidate overlay, verifies every step with compilers, LSPs (``rust-analyzer``, ``pyright``, ``tsc``, ``gopls``), and test runners (``cargo test``, ``pytest``), and promotes only working code.
      * - **4. Rust SDK**
        - ``perspt-sdk``, ``perspt-coding``, and ``perspt-research`` expose the verification loop, capability kernel, and domain traits for your own Rust programs.
      * - **9+ LLM Providers**
        - Works out of the box with OpenAI, Anthropic, Google Gemini, Vertex AI, Groq, Cohere, xAI, DeepSeek, local Ollama, and any OpenAI-compatible server.
      * - **Safe Sandbox & Undo**
        - Starlark security policies, OS process sandboxing (Bubblewrap / ``sandbox-exec``), and a local DuckDB ledger with one-command rollback (``perspt ledger --rollback``).

   ----

.. toctree::
   :maxdepth: 2
   :caption: Getting Started

   introduction
   quickstart
   installation
   getting-started

.. toctree::
   :maxdepth: 2
   :caption: Tutorials

   tutorials/index

.. toctree::
   :maxdepth: 2
   :caption: User Guide

   user-guide/index

.. toctree::
   :maxdepth: 2
   :caption: Concepts

   concepts/index

.. toctree::
   :maxdepth: 2
   :caption: How-To Guides

   howto/index
   configuration

.. toctree::
   :maxdepth: 2
   :caption: Reference

   reference/index

.. toctree::
   :maxdepth: 2
   :caption: API & SDK Reference

   api/index

.. toctree::
   :maxdepth: 2
   :caption: Developer Guide

   developer-guide/index

.. toctree::
   :maxdepth: 1
   :caption: Appendices

   changelog
   license
   acknowledgments

.. only:: html

   Quick Links
   -----------

   - `GitHub Repository <https://github.com/eonseed/perspt>`_
   - `Crates.io <https://crates.io/crates/perspt>`_
   - `PSP Catalog <../psps/index.html>`_
   - `Issue Tracker <https://github.com/eonseed/perspt/issues>`_
