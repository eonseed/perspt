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
     - Version 1.82.0 or later (required for building from source)
   * - **Terminal Emulator**
     - Modern console supporting UTF-8 encoding and 256-color escape sequences
   * - **Network Link**
     - Required for cloud LLM API communication (unnecessary for local Ollama deployments)

Provider Access Configuration
-----------------------------

Perspt requires access to an external model oracle. You must define and export the appropriate API key as an environment variable. The system inspects the environment and maps the configuration according to a deterministic detection priority:

.. code-block:: text

   OpenAI > Anthropic > Gemini > Groq > Cohere > XAI > DeepSeek > Ollama

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
2. **Task Sheafification**: The Architect model decomposes the instruction into a directed acyclic graph (DAG) of task nodes. Each node represents a single module and lists its expected output files. The system enforces the *ownership closure* rule (no file can be modified by more than one node).
3. **Stabilization Loop**: The scheduler processes nodes in ready order. For each node, the Actuator proposes an artifact bundle containing file writes, diffs, and commands. The system applies the changes and computes the Lyapunov energy:
   
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

To speed up iteration, you can defer execution-tier tests until the final validation pass:

.. code-block:: bash

   perspt agent --yes --defer-tests -w ./rust-csv-converter "Build a Rust CLI tool that converts CSV to JSON"

Parameterizing Models per Tier
------------------------------

The system divides the agent runtime into four operational tiers. You can allocate different models to these tiers depending on the complexity of the role:

- **Architect**: Responsible for graph planning and structural revisions.
- **Actuator**: Responsible for generating code edits and artifact bundles.
- **Verifier**: Responsible for measuring remaining residuals.
- **Speculator**: Responsible for fast validation and lookahead.

To run the agent with customized model selections:

.. code-block:: bash

   perspt agent \
     --architect-model gemini-2.5-pro \
     --actuator-model gemini-2.5-flash \
     --verifier-model gemini-2.5-pro \
     --speculator-model gemini-2.5-flash \
     -w ./project "Task description"

Each tier also supports a fallback model option in case the primary oracle returns a rate limit or API error:

.. code-block:: bash

   perspt agent \
     --architect-model gemini-2.5-pro \
     --architect-fallback-model gemini-2.5-flash \
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

      Understand the twelve-crate design.
