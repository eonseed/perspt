Getting Started
===============

This guide walks through installation, first chat, and first agent run.

Prerequisites
-------------

System Requirements
~~~~~~~~~~~~~~~~~~~

.. list-table::
   :widths: 20 80
   :header-rows: 1

   * - Component
     - Requirement
   * - **Operating System**
     - Linux, macOS, or Windows
   * - **Rust Toolchain**
     - Rust 1.82.0 or later
   * - **Terminal**
     - Any modern terminal emulator with UTF-8 support
   * - **Network**
     - Required for cloud LLM API calls (not needed for Ollama)

API Keys
~~~~~~~~

You need an API key from at least one provider. Set it as an environment variable
and Perspt will auto-detect the provider:

.. code-block:: bash

   # Pick one — Perspt auto-detects from the environment
   export OPENAI_API_KEY="sk-..."
   export ANTHROPIC_API_KEY="sk-ant-..."
   export GEMINI_API_KEY="..."

   # Ollama needs no key — just run: ollama serve

.. note::
   **Auto-detection priority**: OpenAI > Anthropic > Gemini > Groq > Cohere >
   XAI > DeepSeek > Ollama.


Quick Installation
------------------

.. tab-set::

   .. tab-item:: From Source (Recommended)

      .. code-block:: bash

         git clone https://github.com/eonseed/perspt.git
         cd perspt
         cargo build --release
         ./target/release/perspt --version

   .. tab-item:: Cargo Install

      .. code-block:: bash

         cargo install perspt
         perspt --version

   .. tab-item:: Binary Download

      .. code-block:: bash

         curl -L https://github.com/eonseed/perspt/releases/latest/download/perspt-linux-x86_64.tar.gz | tar xz
         chmod +x perspt && sudo mv perspt /usr/local/bin/


Your First Chat
---------------

**TUI mode** (default) provides a rich terminal interface with markdown rendering:

.. code-block:: bash

   export GEMINI_API_KEY="your-key"
   perspt

You will see a chat interface. Type a message and press **Enter**. Perspt streams
the response in real time with markdown formatting. Press **Esc** to exit.

**Simple CLI mode** is a minimal text interface suitable for scripting:

.. code-block:: bash

   perspt simple-chat
   # Or with logging
   perspt simple-chat --log-file session.txt

Type your question, get a streamed answer. Type ``exit`` or press ``Ctrl+D`` to quit.


Your First Agent Run
--------------------

Agent mode lets Perspt plan and write multi-file projects autonomously:

.. code-block:: bash

   # Create a new Python package
   perspt agent -w ./demo-calc \
     "Create a Python calculator package with add, subtract, multiply, divide.
      Include type hints, a pyproject.toml, and pytest tests."

What happens:

1. **Detection** — Perspt identifies Python from the task description, selects the
   ``python`` plugin (``ty`` LSP, ``pytest`` runner, ``uv init --lib``).
2. **Planning** — The Architect model decomposes the task into a DAG of nodes, each
   owning specific output files (ownership closure rule).
3. **Execution** — Nodes execute in topological order. For each node, the Actuator
   generates a multi-file artifact bundle (writes, diffs, commands).
4. **Verification** — LSP diagnostics compute V_syn, ``pytest`` computes V_log,
   and bootstrap commands compute V_boot. Total energy V(x) is checked.
5. **Review** — In interactive mode, a diff viewer presents changes for approval.
   In headless mode (``--yes``), all changes are auto-approved.
6. **Commit** — Stable nodes are recorded in the Merkle ledger.

After completion, inspect the output:

.. code-block:: bash

   ls demo-calc/
   # pyproject.toml  src/  tests/  uv.lock

   cd demo-calc && uv run pytest -v


Headless Mode
~~~~~~~~~~~~~

For CI/CD or batch workflows, use ``--yes`` to skip all interactive prompts:

.. code-block:: bash

   perspt agent --yes -w ./output "Build a Rust CLI that converts CSV to JSON"

Combine with ``--defer-tests`` for faster iteration (skips V_log during coding,
only runs tests at the final sheaf validation stage):

.. code-block:: bash

   perspt agent --yes --defer-tests -w ./output "Build a data pipeline"


Switching Models
----------------

.. code-block:: bash

   # Chat with a specific model
   perspt chat --model gemini-pro-latest

   # Agent with per-tier models
   perspt agent \
     --architect-model gemini-pro-latest \
     --actuator-model gemini-3.1-flash-lite-preview \
     --verifier-model gemini-pro-latest \
     --speculator-model gemini-3.1-flash-lite-preview \
     "Build a web scraper"


Next Steps
----------

.. grid:: 2
   :gutter: 3

   .. grid-item-card:: Tutorials
      :link: tutorials/index
      :link-type: doc

      Step-by-step learning path.

   .. grid-item-card:: Configuration
      :link: howto/configuration
      :link-type: doc

      Providers, models, and preferences.

   .. grid-item-card:: Agent Mode
      :link: tutorials/agent-mode
      :link-type: doc

      Full SRBN agent walkthrough.

   .. grid-item-card:: Concepts
      :link: concepts/index
      :link-type: doc

      SRBN architecture and energy model.
