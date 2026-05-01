.. _quickstart:

Quick Start
===========

Get Perspt running in 5 minutes.

Prerequisites
-------------

.. list-table::
   :widths: 20 80

   * - **Rust 1.82+**
     - `rustup.rs <https://rustup.rs/>`_ for building from source
   * - **API Key**
     - From any provider (OpenAI, Anthropic, Google, etc.) OR Ollama for local models

Installation
------------

.. tab-set::

   .. tab-item:: From Source (Recommended)

      .. code-block:: bash

         git clone https://github.com/eonseed/perspt.git
         cd perspt
         cargo build --release

   .. tab-item:: Cargo Install

      .. code-block:: bash

         cargo install perspt

   .. tab-item:: With Ollama (No API Key)

      .. code-block:: bash

         # Install Ollama
         brew install ollama  # macOS

         # Start and pull a model
         ollama serve
         ollama pull llama3.2

Set Your API Key
----------------

.. code-block:: bash

   # Choose your provider
   export OPENAI_API_KEY="sk-..."        # OpenAI
   export ANTHROPIC_API_KEY="sk-ant-..." # Anthropic
   export GEMINI_API_KEY="..."           # Google Gemini

Run Your First Chat
-------------------

.. code-block:: bash

   # Start the TUI (auto-detects provider from env)
   ./target/release/perspt

   # Or with a specific model
   perspt chat --model gemini-pro-latest

Type your message and press **Enter**. Press **Esc** to exit.

Try Agent Mode
--------------

Let the experimental SRBN agent autonomously plan and build multi-file projects:

.. code-block:: bash

   # Create a project in a new directory
   perspt agent -w ./my-calculator "Create a Python calculator package with
   add, subtract, multiply, divide. Include type hints and pytest tests."

   # Auto-approve all changes (headless)
   perspt agent -y -w ./my-api "Build a REST API in Rust with Axum"

   # Use specific models per tier
   perspt agent \
     --architect-model gemini-pro-latest \
     --actuator-model gemini-3.1-flash-lite-preview \
     -w ./project "Create an ETL pipeline in Python"

The SRBN engine will:

1. **Detect** — Identify language plugins and workspace state
2. **Plan** — Architect decomposes task into a DAG with ownership closure
3. **Generate** — Actuator emits multi-file artifact bundles per node
4. **Verify** — LSP diagnostics + tests compute Lyapunov energy V(x)
5. **Converge** — Retry with grounded error feedback until V(x) < epsilon
6. **Sheaf Check** — Validate cross-node consistency
7. **Commit** — Record stable state in Merkle ledger

.. seealso:: :doc:`tutorials/agent-mode` for a full walkthrough.

Choose Your Mode
----------------

.. list-table::
   :header-rows: 1
   :widths: 15 35 50

   * - Mode
     - Command
     - Best For
   * - **Chat TUI**
     - ``perspt`` or ``perspt chat``
     - Interactive conversations with markdown rendering
   * - **Agent**
     - ``perspt agent "<task>"``
     - Autonomous multi-file code generation (experimental)
   * - **Simple Chat**
     - ``perspt simple-chat``
     - Scripting, pipelines, no TUI
   * - **Status**
     - ``perspt status``
     - Check current agent session

Essential Commands
------------------

.. code-block:: bash

   # Configuration
   perspt config --show           # View current config
   perspt config --edit           # Edit in $EDITOR
   perspt init --memory --rules   # Initialize project

   # Agent management
   perspt status                  # Current session status
   perspt abort                   # Cancel current session
   perspt resume --last           # Resume last interrupted session

   # Change tracking
   perspt ledger --recent         # View recent changes
   perspt ledger --rollback abc   # Rollback to commit
   perspt ledger --stats          # Session statistics

   # Debugging
   perspt logs --tui              # Interactive log viewer
   perspt logs --last             # Most recent session
   perspt logs --stats            # Usage statistics

Key Bindings (Chat TUI)
------------------------

.. list-table::
   :header-rows: 1
   :widths: 20 80

   * - Key
     - Action
   * - **Enter**
     - Send message
   * - **Esc**
     - Exit application
   * - **Up / Down**
     - Scroll chat history
   * - **Page Up / Down**
     - Fast scroll
   * - **/save**
     - Save conversation (command)

Next Steps
----------

.. grid:: 2
   :gutter: 3

   .. grid-item-card:: Tutorials
      :link: tutorials/index
      :link-type: doc

      Step-by-step learning guides.

   .. grid-item-card:: Configuration
      :link: howto/configuration
      :link-type: doc

      Customize providers and models.

   .. grid-item-card:: Agent Deep Dive
      :link: tutorials/agent-mode
      :link-type: doc

      Master autonomous coding.

   .. grid-item-card:: Architecture
      :link: developer-guide/architecture
      :link-type: doc

      Understand the 9-crate design.
