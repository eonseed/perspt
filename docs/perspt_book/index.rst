.. Perspt documentation master file

.. title:: Perspt: Simple CLI, Terminal UI, and Verified Coding Agent

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
      <p><em>Simple CLI · Interactive Terminal UI · Reliable Coding Agent · Rust SDK</em></p>
      </div>

.. only:: latex

   Perspt: Simple CLI, Terminal UI, and Verified Coding Agent
   ==========================================================

.. figure:: ../shared/_static/front-page-abm.svg
   :alt: Agent-based model trajectory toward consensus under SRBN verification
   :width: 100%

   How Perspt's coding agent stays reliable: unguided LLM loops drift as errors compound across steps, while Perspt's internal SRBN verification loop checks every change against your compiler and test suite until errors reach zero (:math:`V(x) = 0`).

.. epigraph::

   The keyboard hums, the screen aglow,
   AI's wisdom, a steady flow.
   Will robots take over, it's quite the fright,
   Or just provide insights, day and night?
   We ponder and chat, with code as our guide,
   Is AI our helper or our human pride?

**AI in Your Terminal — Built for Everyday Developer Workflows**

Perspt is a fast, single-binary Rust tool (and embeddable **Rust SDK**) designed to make working with Large Language Models effortless and reliable from your terminal. It gives developers **three practical ways to work with AI**, plus an SDK for custom integrations:

1. **Simple CLI (``perspt simple-chat``)** — Plain terminal output with zero UI chrome. Pipe files, compiler logs, or ``git diff`` into ``stdin`` and get clean text back on ``stdout`` for shell scripts and one-liners.
2. **Interactive Terminal UI (``perspt`` / ``perspt chat``)** — A full-screen terminal chat app with streaming Markdown, syntax highlighting, tables, LaTeX math, conversation saving, read-only MCP tool support, and instant switching across 9+ LLM providers.
3. **SRBN Coding Agent (``perspt agent``)** — An autonomous coding agent that plans tasks, edits files in an isolated sandbox, and runs your real compiler, LSP, and test suite to fix its own mistakes before committing changes to your project.
4. **Embeddable Rust SDK (``perspt-sdk``, ``perspt-coding``, ``perspt-research``)** — Library crates that let you integrate Perspt's verified agent loop, policy engine, and multi-provider runtime directly into your own Rust applications.

.. note:: What is SRBN?

   **SRBN (Stabilized Recursive Barrier Networks)** is the internal reliability engine inside ``perspt agent``, based on the *Stability is All You Need* research paper series. As a developer, you don't need to configure or know the math to use ``perspt agent``—SRBN works automatically behind the scenes by running your compiler, language server, and tests after each edit, scoring remaining errors as :math:`V(x)`, and rolling back bad guesses so the code Perspt generates actually runs and stays close to the task you gave it.

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
