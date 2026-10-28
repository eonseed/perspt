.. meta::
   :description: Perspt is an AI chat and coding assistant for the terminal. Chat with cloud or local models, ask an agent to edit code, and review build and test results.

Perspt
======

.. container:: hero

   .. container:: hero-copy

      .. rst-class:: eyebrow

      PERSPT / AI TOOLS FOR DEVELOPERS

      .. raw:: html

         <h1>AI chat and coding<br>in your terminal.</h1>

      .. rst-class:: lead

      Ask questions, understand a codebase, or ask a coding agent to fix a bug. Perspt runs in the terminal, connects to cloud or local AI models, and uses the project's compiler and tests to check the agent's work.

      .. container:: actions

         `Get started <mount://book/quickstart.html>`_
         `Try the coding agent <mount://book/tutorials/agent-mode.html>`_

      .. rst-class:: hero-note

      Built in Rust. Works with OpenAI, Anthropic, Gemini, Ollama, and more.

      `GitHub <https://github.com/eonseed/perspt>`_ | `Read the book <mount://book/index.html>`_ | `Download PDF <mount://pdf/perspt-book-en.pdf>`_

   .. container:: hero-art

      .. image:: ../shared/_static/cover-art.svg
         :alt: Example commands for AI chat, a coding task, and using Perspt as a Rust library.

      .. rst-class:: art-caption

      CHAT, WRITE CODE, AND RUN TESTS FROM THE TERMINAL.

.. container:: section-heading

   .. rst-class:: eyebrow

   WHAT CAN PERSPT DO?

   .. rubric:: Chat with AI or put it to work on code.

.. container:: pillars

   .. container:: pillar

      .. rubric:: Chat with AI

      **Ask a question and read the answer as it arrives.** The full-screen chat displays Markdown, code, and tables. Paste a multiline question, switch models, or connect read-only MCP tools.

      .. code-block:: sh

         perspt chat

      `Start a conversation <mount://book/tutorials/first-chat.html>`_

   .. container:: pillar

      .. rubric:: Use a simpler chat interface

      **Type a message, get an answer, and keep the conversation in the terminal.** Use ``simple-chat`` for a line-by-line chat without a full-screen interface. Save the conversation with ``--log-file``.

      .. code-block:: sh

         perspt simple-chat --log-file chat.txt

      `Learn the chat commands <mount://book/user-guide/basic-usage.html>`_

   .. container:: pillar

      .. rubric:: Ask the agent to work on code

      **Fix a bug, add a feature, or write tests.** Give Perspt a task and a project folder. It edits a separate copy, runs the build and tests, and asks for review before applying the result.

      .. code-block:: sh

         perspt agent -w ./app "Fix the parser tests"

      `Follow the coding tutorial <mount://book/tutorials/agent-mode.html>`_

   .. container:: pillar

      .. rubric:: Use Perspt in a Rust application

      **Build an AI workflow with Perspt's libraries.** Reuse the code for running agents, checking their output, and recording session history. The SDK guide explains which crates to use.

      .. code-block:: sh

         cargo add perspt-sdk perspt-coding \
           --git https://github.com/eonseed/perspt

      `Read the SDK guide <mount://book/api/index.html>`_

.. container:: section-heading

   .. rst-class:: eyebrow

   HOW THE CODING AGENT WORKS

   .. rubric:: Describe the task. Review the code.

**Perspt works on a separate copy of the project.** The agent can read files, make edits, and run the project's tools. When a build or test fails, it uses the error messages to try a fix. The original files are updated after the required checks pass and the result is approved.

.. figure:: ../shared/_static/agent-workflow-overview.svg
   :target: _static/agent-workflow-overview.svg
   :alt: Give the agent a task, let it edit a project copy, run the build and tests, then review and apply the changes.
   :width: 100%

   The agent tries to fix build and test failures before asking for review. If it runs out of attempts, it stops and reports the remaining problems.

Agent mode is experimental. Passing tests can miss bugs, so review the diff and the tests before committing. Perspt does not create a Git commit when it applies edits.

.. container:: section-heading

   .. rst-class:: eyebrow

   LEARN FROM BUILD AND TEST FAILURES

   .. rubric:: Keep improvements. Reject regressions.

While required checks are failing, Perspt keeps an attempt only if its error score improves enough.
Otherwise, it restores the best version it has saved and uses the reported problems to try another fix.
An improvement can help the next attempt; the required checks still need to pass before the result can be applied.

.. figure:: ../shared/_static/front-page-abm.svg
   :target: _static/front-page-abm.svg
   :alt: An illustrative error score falls across kept attempts. Two attempts that make it worse are rejected. The final attempt passes the required checks.
   :width: 100%

   Dots show individual build and test results. The curves help us follow the attempts; this is an illustration, not benchmark data or a promise that every task will succeed.

.. container:: split-section

   .. container:: split-copy

      .. rst-class:: eyebrow

      FOLLOW THE WORK

      .. rubric:: See what the agent is doing.

      Add ``--dashboard`` to watch a run in the browser. See the task, build and test results, and the history of the agent's actions.

      ``perspt replay <session-id>`` reads the saved history without calling an AI model or rerunning tests. ``perspt resume --last`` continues an interrupted session.

      `Explore the dashboard <mount://book/user-guide/dashboard.html>`_

   .. container:: split-copy

      .. rst-class:: eyebrow

      STAY IN CONTROL

      .. rubric:: Review edits before they reach the project.

      Interactive runs ask for approval before applying code changes. Use ``--yes`` when an unattended run should apply a successful result automatically.

      ``perspt ledger --rollback <session-prefix>`` undoes a session's latest set of applied edits. It refuses to overwrite files changed afterward.

      Linux and macOS support OS sandboxing. Native Windows agent runs need an explicit option to run without that isolation, or WSL.

      `Read the agent guide <mount://book/user-guide/agent-mode.html>`_ | `Check setup requirements <mount://book/installation.html>`_

Install Perspt and start chatting
---------------------------------

With Rust 1.97.1 or later installed:

.. code-block:: sh

   git clone https://github.com/eonseed/perspt.git
   cd perspt
   cargo install --locked --path crates/perspt-cli --features bundled
   export GEMINI_API_KEY="<api-key>"
   perspt chat

For local AI models, connect Ollama and choose an installed model such as ``ollama::llama3.2``. To try the coding agent, start with a small task in a project whose build and tests already run locally.

.. container:: text-links

   `Quick start <mount://book/quickstart.html>`_
   `Local models <mount://book/tutorials/local-models.html>`_
   `Coding tutorial <mount://book/tutorials/agent-mode.html>`_
   `Rust SDK <mount://book/api/index.html>`_
   `How verification works <mount://book/concepts/srbn-architecture.html>`_
   `Specification proposals <mount://psps/index.html>`_
   `Rustdoc <perspt/index.html>`_
