.. _quickstart:

Quick Start
===========

**Start with a conversation, then try a small code change.** We need a model provider for chat and the project's own verification tools for agent mode.

1. Install the CLI
------------------

From the repository root, Rust 1.97.1 or later can build and install the CLI:

.. code-block:: bash

   git clone https://github.com/eonseed/perspt.git
   cd perspt
   cargo install --locked --path crates/perspt-cli --features bundled
   perspt --version

The ``bundled`` feature builds DuckDB into the executable, so we do not need a separate DuckDB library.
Compilation can take several minutes.
See :doc:`installation` for platform and agent prerequisites.

2. Connect a model
------------------

Set one provider key in the terminal where Perspt will run:

.. code-block:: bash

   export GEMINI_API_KEY="<api-key>"
   perspt chat

OpenAI and Anthropic use ``OPENAI_API_KEY`` and ``ANTHROPIC_API_KEY`` respectively.
With several credentials set, an explicit ``provider::model`` route selects the intended provider.
Model availability depends on the provider account.

For a local conversation, start Ollama in another terminal, then pull and select a model:

.. code-block:: bash

   ollama pull llama3.2
   perspt chat --model ollama::llama3.2

See :doc:`tutorials/local-models` for local setup and :doc:`user-guide/providers` for other providers.

3. Ask a question
-----------------

In the full-screen chat, type a question such as ``Explain the difference between a Rust String and &str`` and press **Enter**.
Use **Ctrl+J** for a newline, **Page Up / Page Down** to scroll, and **Esc** to exit.
Configured read-only MCP operations are available through ``/mcp``.

For a line-oriented interface:

.. code-block:: bash

   perspt simple-chat --log-file chat.txt

Type a message and press **Enter**; **Ctrl+D** exits.
This command takes no positional prompt argument.
It reads piped input one non-empty line at a time, and its output includes the banner, input echo, and session messages as well as model responses.
For a single piped question:

.. code-block:: bash

   printf '%s\n' 'Explain Rust ownership in three sentences.' | perspt simple-chat

4. Try a code change
--------------------

**Start in an existing project with working local checks.** For a Rust project, run ``cargo test`` first, then give Perspt a specific task:

.. code-block:: bash

   perspt agent -w ./my-project \
     "Add tests for empty input in the parser. Preserve the public API."

The agent edits a candidate copy and runs the detected checks.
It uses measured errors to guide repairs and asks for review before copying the final result into the project.
Promotion does not create a Git commit.
Agent mode is experimental; required tools and platform isolation are described in :doc:`installation`.

**A hard pass means the required checks passed.** An improving intermediate checkpoint is not a completed task.
If the run reaches a limit with errors remaining, it stops with a failure outcome instead of reporting success.
We still review the diff and the adequacy of the tests.

5. Watch or automate a run
--------------------------

.. code-block:: bash

   # Live local browser view at http://127.0.0.1:3000
   perspt agent --dashboard -w ./my-project "Fix the failing parser test"

   # Non-interactive run; verification still applies
   perspt agent --yes --output-summary summary.json \
     -w ./my-project "Add tests for empty parser input"

``--yes`` approves promotion automatically and is required without a terminal.
A non-hard-pass outcome exits with a nonzero status.
The embedded dashboard stops when the agent exits.

Next steps
----------

- :doc:`tutorials/agent-mode`: Follow a complete coding task and inspect the result.
- :doc:`configuration`: Choose providers, test policies, and verification settings.
- :doc:`user-guide/agent-mode`: Understand review, limits, resume, and replay.
- :doc:`api/index`: Reuse the Rust SDK.
