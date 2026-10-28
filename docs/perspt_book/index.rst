.. title:: The Perspt Book

The Perspt Book
===============

.. rst-class:: lead

**Learn to use Perspt for AI chat and coding in the terminal.** We start with installation and a conversation, then use the coding agent to work on a project and review the result.

Where to start
--------------

.. list-table::
   :header-rows: 1
   :widths: 34 66

   * - Task
     - Guide
   * - **Try Perspt**
     - :doc:`quickstart`: Install Perspt, connect a provider, and ask a question.
   * - **Use a local model**
     - :doc:`tutorials/local-models`: Connect Ollama and select an installed model.
   * - **Run a coding task**
     - :doc:`tutorials/agent-mode`: Ask the agent to edit code, run tests, and show the result for review.
   * - **Run without prompts**
     - :doc:`tutorials/headless-mode`: Automate a task and save its result as JSON.
   * - **Use Perspt from Rust**
     - :doc:`api/index`: Choose the libraries needed for a Rust application.

What Perspt does
----------------

- **AI chat** with ``perspt chat``: a full-screen conversation with formatted answers, code, and multiline input.
- **Simple terminal chat** with ``perspt simple-chat``: type a message, read the answer, and optionally save the conversation.
- **Coding tasks** with ``perspt agent``: ask the agent to fix a bug, add a feature, or write tests. It edits a separate copy and runs the project's tools before asking for review.
- **Rust libraries** for adding AI workflows, build and test checks, and session history to another application.

.. figure:: ../shared/_static/agent-workflow-overview.svg
   :target: _static/agent-workflow-overview.svg
   :alt: Describe a task, edit a project copy, run builds and tests, then review and apply the changes.
   :width: 100%

   The coding agent asks for approval before copying a successful result into the project. If it cannot finish within the run's limits, it reports the remaining problems.

Agent mode is experimental.
We review the code and tests before committing, because passing checks can miss bugs.
:doc:`introduction` explains the main commands and :doc:`concepts/srbn-architecture` covers the implementation in detail.

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
