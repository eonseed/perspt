.. title:: Perspt: AI tools for developers

The Perspt Book
===============

.. rst-class:: lead

**Chat, explore, and make checked code changes from the terminal.** This book starts with everyday workflows and introduces the runtime and SDK as we need them.

Choose a starting point
-----------------------

.. list-table::
   :header-rows: 1
   :widths: 34 66

   * - Goal
     - Start here
   * - **Try Perspt**
     - :doc:`quickstart`: Install the CLI, connect a model, and ask a question.
   * - **Work locally**
     - :doc:`tutorials/local-models`: Connect an installed Ollama model.
   * - **Change a project**
     - :doc:`tutorials/agent-mode`: Run a small coding task, inspect checks, and review the diff.
   * - **Automate a task**
     - :doc:`tutorials/headless-mode`: Use explicit approval, limits, and a JSON summary.
   * - **Embed the SDK**
     - :doc:`api/index`: Score diagnostics and evaluate a gate in Rust.

What Perspt provides
--------------------

- **Full-screen chat** with ``perspt chat``: streamed Markdown, code, multiline input, and configured read-only MCP tools.
- **Plain terminal chat** with ``perspt simple-chat``: a line-oriented conversation with optional logging. Output includes session messages.
- **Checked coding tasks** with ``perspt agent``: experimental candidate edits, detected project checks, bounded repairs, and promotion review.
- **Reusable Rust libraries** for scoring, gates, capabilities, domain adapters, and ledger records.

.. figure:: ../shared/_static/front-page-abm.svg
   :target: _static/front-page-abm.svg
   :alt: Illustrative measured candidate energy and accepted checkpoints, with smooth curves for visual guidance and discrete gate decisions.
   :width: 100%

   A candidate can improve without being ready to deliver. Dots show verification checkpoints; smooth curves are illustrative. A run can reach a hard pass or stop with errors remaining.

**A hard pass means the required checks passed.** It does not establish every requirement beyond the coverage of those checks.
Promotion copies an approved result into the project; it does not create a Git commit.
:doc:`introduction` explains the workflow and :doc:`concepts/srbn-architecture` explains the underlying SRBN runtime.

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
