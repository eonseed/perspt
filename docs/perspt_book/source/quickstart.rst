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
   export GEMINI_API_KEY="..."           # Google

Run Your First Chat
-------------------

.. code-block:: bash

   # Start the TUI
   ./target/release/perspt

   # Or with a specific model
   perspt chat --model gpt-5.2

Type your message and press **Enter**. Press **Esc** to exit.

Try Agent Mode
--------------

.. versionadded:: 0.5.0

Let Perspt autonomously write code:

.. code-block:: bash

   # Basic task
   perspt agent "Create a Python calculator with add, subtract, multiply, divide"

   # With workspace directory
   perspt agent -w ./my-project "Add unit tests for the API"

   # Auto-approve all changes
   perspt agent -y "Refactor the parser for better error handling"

The SRBN engine will:

1. **Sheafify** — Decompose task into sub-tasks
2. **Speculate** — Generate code for each sub-task
3. **Verify** — Check with LSP and tests
4. **Converge** — Retry until V(x) < ε
5. **Commit** — Record in Merkle ledger

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
     - Autonomous code generation and modification
   * - **Status**
     - ``perspt status``
     - Check current agent session status

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
   perspt resume                  # Resume interrupted session

   # Change tracking
   perspt ledger --recent         # View recent changes
   perspt ledger --rollback abc   # Rollback to commit

Key Bindings (Chat TUI)
-----------------------

.. list-table::
   :header-rows: 1
   :widths: 20 80

   * - Key
     - Action
   * - **Enter**
     - Send message
   * - **Esc**
     - Exit application
   * - **↑/↓**
     - Scroll chat history
   * - **Page Up/Down**
     - Fast scroll
   * - **/save**
     - Save conversation (command)

Next Steps
----------

.. grid:: 2
   :gutter: 3

   .. grid-item-card:: 📚 Tutorials
      :link: tutorials/index
      :link-type: doc

      Step-by-step learning guides.

   .. grid-item-card:: ⚙️ Configuration
      :link: howto/configuration
      :link-type: doc

      Customize providers and models.

   .. grid-item-card:: 🤖 Agent Deep Dive
      :link: tutorials/agent-mode
      :link-type: doc

      Master autonomous coding.

   .. grid-item-card:: 🏗️ Architecture
      :link: developer-guide/architecture
      :link-type: doc

      Understand the 6-crate design.
