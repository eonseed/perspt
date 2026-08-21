.. _getting-started:

Getting Started
===============

This guide outlines the protocol for setting up the environment, compiling the binaries, executing an interactive chat session, and running an autonomous agent task.

System Requirements
-------------------

Before installation, verify that the host environment conforms to the following operational parameters:

.. list-table::
   :widths: 25 75
   :header-rows: 1

   * - Component
     - Specification Requirement
   * - **Operating System**
     - Linux, macOS, or Windows (via Windows Subsystem for Linux)
   * - **Rust Compiler**
     - Version 1.97.1 or later (required for building from source)
   * - **Terminal Emulator**
     - Modern console supporting UTF-8 encoding and 256-color escape sequences
   * - **Network Link**
     - Required for cloud LLM API communication (unnecessary for local Ollama deployments)

Provider Access Configuration
-----------------------------

Perspt requires access to an external model oracle. You must define and export the appropriate API key as an environment variable. The system inspects the environment and maps the configuration according to a deterministic detection priority:

.. code-block:: text

   Vertex AI > Gemini > OpenAI > Anthropic > Groq > Cohere > XAI > DeepSeek > Ollama

Set the key for your selected provider:

.. code-block:: bash

   # Example configurations
   export OPENAI_API_KEY="sk-..."
   export ANTHROPIC_API_KEY="sk-ant-..."
   export GEMINI_API_KEY="..."

For offline execution using local models, start the Ollama service:

.. code-block:: bash

   ollama serve
   ollama pull llama3.2

Quick Installation
------------------

.. tab-set::

   .. tab-item:: From Source (Recommended)

      To compile the release binary directly from the source repository:

      .. code-block:: bash

         git clone https://github.com/eonseed/perspt.git
         cd perspt
         cargo build --release
         ./target/release/perspt --version

   .. tab-item:: Cargo Install

      To install the package into your Cargo binary path:

      .. code-block:: bash

         cargo install perspt
         perspt --version

   .. tab-item:: Binary Archive

      To download and deploy the precompiled release archive:

      .. code-block:: bash

         curl -L https://github.com/eonseed/perspt/releases/latest/download/perspt-linux-x86_64.tar.gz | tar xz
         chmod +x perspt && sudo mv perspt /usr/local/bin/

Interactive Dialogue Session
----------------------------

The terminal user interface (TUI) is the default interactive environment. To launch the TUI:

.. code-block:: bash

   perspt

Upon initiation, the system establishes a session using the detected API key.

- **Input Entry**: Enter your dialogue prompt and press **Enter** to stream the response.
- **Scrollback**: Navigate the conversation scrollback window using **Up/Down** or **Page Up/Page Down**.
- **Exit**: Press **Esc** to terminate the TUI session.

For non-interactive pipelines or shell-script piping, use the simple chat command:

.. code-block:: bash

   perspt simple-chat
   # Optionally record the session output
   perspt simple-chat --log-file session.txt

Type ``exit`` or enter ``Ctrl+D`` to terminate the simple chat process.

Autonomous Agent Execution
--------------------------

Agent mode compiles a task charter into a state graph of modules and executes them under a closed-loop stabilizer.

To execute an autonomous coding task:

.. code-block:: bash

   perspt agent -w ./demo-calculator \
     "Create a Python calculator package with add, subtract, multiply, divide. Include type hints and pytest tests."

Operational Execution Steps
~~~~~~~~~~~~~~~~~~~~~~~~~~~

During execution, the SRBN engine performs the following operations:

1. **System Detection**: The program identifies Python as the target workspace language, and registers the corresponding LSP verifier and pytest environments.
2. **Graph Planning**: Planning is a governed architect turn that revises the work graph through the ``update_graph`` tool. Each node lists its declared file footprint, and the dispatcher schedules ready nodes by footprint conflict (no two concurrent nodes may touch the same files).
3. **Stabilization Loop**: For each dispatched node, the Actuator issues typed tool calls against a reversible candidate overlay; the deterministic kernel admits each call before it is applied. The system then computes the Lyapunov energy on the realized candidate:

   - Syntactic energy (:math:`V_{\text{syn}}`): Diagnostics from the LSP.
   - Logical energy (:math:`V_{\text{log}}`): Test failures from the test runner.
   - Build energy (:math:`V_{\text{boot}}`): Exit codes of environment setups.

   If :math:`V(x) > \varepsilon`, the engine compiles the error diagnostics into a correction prompt and retries. This loops until the node converges (:math:`V(x) \leq \varepsilon`) or the retry cap is reached.
4. **Interactive Review**: In interactive mode, the TUI displays the proposed file changes (unified diffs) and verifier states for approval before commit.
5. **Merkle Commit**: Stable nodes are written to the Merkle ledger and committed to the active workspace.

Verifying Output Structures
~~~~~~~~~~~~~~~~~~~~~~~~~~~

Upon task completion, inspect the workspace directory to verify the generated files:

.. code-block:: bash

   ls demo-calculator/
   # Expected structure:
   # pyproject.toml  src/  tests/  uv.lock

To run the verification suite locally:

.. code-block:: bash

   cd demo-calculator && uv run pytest -v

Headless Mode
~~~~~~~~~~~~~

For non-interactive environments, such as automated build pipelines, use the ``--yes`` flag to bypass the interactive review gate:

.. code-block:: bash

   perspt agent --yes -w ./rust-csv-converter "Build a Rust CLI tool that converts CSV to JSON"

Exploration-Only Mode
~~~~~~~~~~~~~~~~~~~~~

To survey a repository under a strictly read-only capability, run only the exploration phase. Every call passes the kernel, mutation attempts are recorded denials, and nothing is mutated or promoted:

.. code-block:: bash

   perspt agent --exploration-only -w ./rust-csv-converter "Summarize how CSV parsing is structured"

Perspt also accepts ``--allow-experimental-prompts`` to substitute validated ``[prompts]`` bundle sections live; such overrides remain experimental until a change record passes paired evaluation.

Parameterizing Models per Role
------------------------------

The agent runtime routes model calls by role. You can allocate different models to these roles depending on the complexity of the work:

- **Actuator** (``--model`` / ``--actuator-model``): Proposes the governed coding tool calls.
- **Explorer** (``--explorer-model``): Optional cheaper read-only repository exploration.
- **Adjudicator** (``--adjudicator-model``): Optional no-tool conjunctive diff veto.

To run the agent with customized model selections:

.. code-block:: bash

   perspt agent \
     --actuator-model gemini-2.5-flash \
     --explorer-model gemini-2.5-flash \
     --adjudicator-model gemini-2.5-pro \
     -w ./project "Task description"

The ``[models]`` table in ``config.toml`` additionally routes architect, actuator, verifier, speculator, and adjudicator turns as fully qualified ``provider::model`` values; when present it takes precedence over the flat ``*_model`` configuration fields.

The actuator route also supports ordered fallback models in case the primary oracle returns a rate limit or API error. The flag is repeatable, and failover is sticky:

.. code-block:: bash

   perspt agent \
     --actuator-model gemini-2.5-pro \
     --fallback-model gemini-2.5-flash \
     --fallback-model gemini-2.5-flash-lite \
     -w ./project "Task description"

Next Steps
----------

.. grid:: 2
   :gutter: 3

   .. grid-item-card:: Tutorials
      :link: tutorials/index
      :link-type: doc

      Step-by-step learning guides.

   .. grid-item-card:: Configuration
      :link: configuration
      :link-type: doc

      Providers, models, and preferences.

   .. grid-item-card:: Agent Deep Dive
      :link: tutorials/agent-mode
      :link-type: doc

      Master autonomous coding.

   .. grid-item-card:: Architecture
      :link: developer-guide/architecture
      :link-type: doc

      Understand the fourteen-crate design.
