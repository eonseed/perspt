Introduction to Perspt
======================

**Perspt is an AI chat and coding assistant that runs in the terminal.** We can ask questions in chat, get help understanding code, or give the coding agent a task to carry out in a project.
It connects to cloud providers and local models through Ollama.

Choose a way to work
--------------------

.. list-table::
   :header-rows: 1
   :widths: 25 30 45

   * - Task
     - Command or library
     - What it does
   * - **Chat with AI**
     - ``perspt chat`` or ``perspt``
     - Opens a full-screen conversation with formatted answers, code, and multiline input.
   * - **Use simple terminal chat**
     - ``perspt simple-chat``
     - Reads a message one line at a time and prints the answer. It can also save a conversation to a file.
   * - **Work on a codebase**
     - ``perspt agent -w ./app "Fix the parser tests"``
     - Reads the project, edits a separate copy, runs the build and tests, and asks for review before applying changes.
   * - **Add Perspt to a Rust application**
     - ``perspt-sdk`` and related crates
     - Provides libraries for building AI workflows, running checks, and recording what happened.

Follow :doc:`quickstart` to install Perspt and start a conversation.
:doc:`user-guide/basic-usage` covers chat controls and :doc:`user-guide/providers` explains provider setup.

Give the coding agent a task
----------------------------

A useful task describes the feature or bug, the project folder, and the expected behavior:

.. code-block:: bash

   perspt agent -w ./app "Add CSV export with tests for empty input"

The agent reads the repository and edits a separate copy of it.
Perspt runs the tools provided by the detected language plugin, such as ``cargo check`` and ``cargo test`` for Rust.
The built-in plugins support Rust, Python, and JavaScript/TypeScript projects.
If a build or test fails, the agent uses the reported errors to try a correction.
Limits on attempts and tool calls prevent a run from continuing indefinitely.

.. figure:: ../shared/_static/agent-workflow-overview.svg
   :target: _static/agent-workflow-overview.svg
   :alt: Describe a coding task, edit a project copy, run builds and tests, then review and apply the changes.
   :width: 100%

   The project files are updated after the required checks pass and the changes are approved. Perspt does not create a Git commit.

**Review the code and the tests.** A passing suite can miss bugs or leave requirements untested.
Tests can change with the implementation by default.
For tasks that must preserve existing behavior, the ``backward-compatible`` policy also runs the historical tests.
An ``external-oracle`` policy adds acceptance tests maintained separately from the agent's work.
See :doc:`configuration` for these settings.

Watch, review, and continue the work
------------------------------------

- **Review edits in the terminal.** The agent shows the diff and the check results before applying changes. ``--yes`` approves this step automatically for unattended runs.
- **Watch in the browser.** Add ``--dashboard`` and open ``http://127.0.0.1:3000`` while the agent runs.
- **Read the session history.** ``perspt replay <session-id>`` reads recorded actions without calling a model or rerunning the tools.
- **Continue an interrupted run.** ``perspt resume --last`` restores the saved progress and checks that the run is still authorized.
- **Undo applied edits.** ``perspt ledger --rollback <session-prefix>`` undoes that session's latest set of applied changes. It refuses to overwrite files changed afterward.

Linux uses Bubblewrap and macOS uses ``sandbox-exec`` to isolate the agent's child processes.
Native Windows requires an explicit option to run without this isolation, or WSL.
See :doc:`installation` for setup and :doc:`user-guide/agent-mode` for the full agent guide.

Use the Rust libraries
----------------------

Perspt's libraries can be used from another Rust application.
``perspt-agent`` runs coding sessions, ``perspt-core`` connects to model providers, and ``perspt-store`` saves session history.
``perspt-sdk`` and ``perspt-coding`` provide the types and checks used by the agent.
The research package is an early example of another domain, not a complete research assistant.

:doc:`api/index` contains a crate guide and a Rust example.
The architecture and mathematical details are in :doc:`concepts/srbn-architecture`.
