.. _quickstart:

Quick Start
===========

Get Perspt running in 5 minutes.

Prerequisites
-------------

- **Rust 1.82+** (for building from source)
- **API key** from any provider, or **Ollama** for local models

Install
-------

.. code-block:: bash

   git clone https://github.com/eonseed/perspt.git
   cd perspt
   cargo build --release

Run Your First Chat
-------------------

.. code-block:: bash

   # Set your API key
   export OPENAI_API_KEY="sk-..."

   # Start chatting
   ./target/release/perspt

That's it! Type a message and press Enter.

Try Agent Mode
--------------

.. versionadded:: 0.5.0

Let Perspt autonomously write and verify code:

.. code-block:: bash

   perspt agent "Create a Python calculator with add, subtract, multiply, divide"

The SRBN engine will:

1. Decompose the task into sub-tasks
2. Generate code for each sub-task
3. Verify with LSP diagnostics
4. Retry until stable

See :doc:`tutorials/agent-mode` for a full walkthrough.

Choose Your Mode
----------------

.. list-table::
   :widths: 20 40 40
   :header-rows: 1

   * - Mode
     - Command
     - Best For
   * - **TUI**
     - ``perspt``
     - Interactive chat with markdown
   * - **Simple CLI**
     - ``perspt --simple-cli``
     - Scripting, accessibility
   * - **Agent**
     - ``perspt agent "task"``
     - Autonomous code generation

Next Steps
----------

.. grid:: 2
   :gutter: 3

   .. grid-item-card:: 📚 Tutorials
      :link: tutorials/index
      :link-type: doc

      Learn through hands-on examples.

   .. grid-item-card:: ⚙️ Configuration
      :link: howto/configuration
      :link-type: doc

      Set up providers and preferences.
