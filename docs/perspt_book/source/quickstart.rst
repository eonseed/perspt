.. _quickstart:

Quick Start
===========

This document outlines the minimal commands required to install, configure, and execute the Perspt terminal application and the autonomous agent mode.

Prerequisites
-------------

Verify that the target system satisfies the following conditions:

- **Rust Toolchain**: Version 1.97.1+ is required for compiling from source.
- **LLM API Key**: Access to OpenAI, Anthropic, Google Gemini, Groq, Cohere, XAI, or DeepSeek, OR a local Ollama service.

Installation
------------

.. tab-set::

   .. tab-item:: From Source (Recommended)

      To compile the release binary directly from the source repository:

      .. code-block:: bash

         git clone https://github.com/eonseed/perspt.git
         cd perspt
         cargo build --release

      The compiled binary is placed at ``target/release/perspt``.

   .. tab-item:: Cargo Install

      To compile and install the package from the local directory:

      .. code-block:: bash

         cargo install --path .

   .. tab-item:: With Ollama (No API Key)

      To run local models using Ollama:

      .. code-block:: bash

         # Start the local Ollama service
         ollama serve

         # Pull the target model
         ollama pull llama3.2

Set Environment API Keys
------------------------

Export the key for your selected provider. The system automatically detects these variables at startup:

.. code-block:: bash

   # Choose one
   export OPENAI_API_KEY="sk-..."        # For OpenAI
   export ANTHROPIC_API_KEY="sk-ant-..." # For Anthropic
   export GEMINI_API_KEY="..."           # For Google Gemini

Executing the Interactive Chat TUI
----------------------------------

To launch the default terminal user interface:

.. code-block:: bash

   # Auto-detects provider from env
   perspt

   # Or specify a model explicitly
   perspt chat --model gemini-3.1-pro

Type your dialogue prompt and press **Enter** to submit. Press **Esc** to exit the application.

TUI Key Bindings
~~~~~~~~~~~~~~~~

.. list-table::
   :header-rows: 1
   :widths: 20 80

   * - Key
     - Action
   * - **Enter**
     - Transmit dialogue input buffer.
   * - **Esc**
     - Terminate the TUI process.
   * - **Up / Down**
     - Navigate through dialogue command history.
   * - **Page Up / Down**
     - Scroll up/down in the chat conversation panel.
   * - **/save**
     - Save dialogue log to a local file.

Executing Agent Mode
--------------------

To execute autonomous multi-file code generation under the SRBN orchestrator:

.. code-block:: bash

   # Create a Python package inside a new directory
   perspt agent -w ./my-calculator "Create a Python calculator package with add, subtract, multiply, divide. Include pytest tests."

   # Auto-approve all modifications (headless mode)
   perspt agent -y -w ./my-api "Build a REST API in Rust with Axum"

   # Run with specific models for Actuator and Explorer roles
   perspt agent \
     --actuator-model gemini-3.5-flash \
     --explorer-model gemini-3.1-flash-lite \
     -w ./project "Create an ETL pipeline in Python"

A headless execution run narrates the governed tool loop: admitted effects, measured energies, and gate decisions. Below is a clinical trace of a typical autonomous run:

.. code-block:: text

   Domain: coding
   PSP-9 agent starting
   Task: Create a Python calculator package with add, subtract, multiply, divide. Include pytest tests.
   Workspace: ./my-calculator
     Exploration mapped 1 language groups and 1 package roots
     PSP-9 session 01997a2f using gemini::gemini-3.5-flash
     [implement-1] Coding
     Effect call-1 applied to candidate (mutated=true)
     Effect call-2 applied to candidate (mutated=true)
     Measured implement-1 generation 1: V=2.000, hard_pass=false, residuals=1
     Gate implement-1 generation 1: RejectedNonDescending { delta_v: 0.0 }
     Effect call-3 applied to candidate (mutated=true)
     Measured implement-1 generation 2: V=0.000, hard_pass=true, residuals=0
     Gate implement-1 generation 2: HardPass

   Outcome: HardPass
   Session: 01997a2f-9c1e-4c30-b7ac-2f5d8f3e6a41
   Turns: 5
   Ledger head: 9f8a7e...
   Promoted paths: pyproject.toml, src/calc/__init__.py, tests/test_calc.py

Every effect the model proposes passes the deterministic admissibility kernel before it touches the candidate workspace, and the acceptance gate reads the re-measured candidate — never the model's account of it.

Operational Modes
-----------------

Choose the appropriate command mode depending on your task requirement:

.. list-table::
   :header-rows: 1
   :widths: 15 35 50

   * - Mode
     - Command
     - Target Use Case
   * - **Chat TUI**
     - ``perspt`` or ``perspt chat``
     - Interactive conversation with formatted terminal rendering.
   * - **Agent**
     - ``perspt agent "<task>"``
     - Autonomous multi-file code generation (experimental).
   * - **Simple Chat**
     - ``perspt simple-chat``
     - CLI chat without terminal interface, ideal for shell piping.
   * - **Exploration**
     - ``perspt agent --exploration-only "<question>"``
     - Read-only repository survey; nothing is mutated or promoted.
   * - **Status**
     - ``perspt status``
     - Query metrics of the active agent session.
   * - **Providers**
     - ``perspt providers --probe``
     - Print the provider capability matrix with live behavioral probes.
   * - **Replay**
     - ``perspt replay <session-id>``
     - Deterministic, credential-free audit replay of a session.
   * - **Audit**
     - ``perspt audit <sample> --safe``
     - Ingest delayed audit labels for conformal calibration.
   * - **Prompts**
     - ``perspt prompts list``
     - Inspect the compiled prompt section libraries.
   * - **Context**
     - ``perspt context explain-turn --db-path <DB> <session-id>``
     - Explain a session's recorded resident-context events.

Essential System Commands
-------------------------

.. list-table::
   :header-rows: 1
   :widths: 30 70

   * - Command
     - Description
   * - ``perspt config --show``
     - Prints active configuration parameters.
   * - ``perspt config --edit``
     - Opens the TOML configuration file in your editor.
   * - ``perspt init --memory --rules``
     - Instantiates memory files and policy rules in the project workspace.
   * - ``perspt status``
     - Displays per-node states, energy components, and retries.
   * - ``perspt abort``
     - Signals the active agent process to terminate.
   * - ``perspt resume --last``
     - Resumes the most recently interrupted agent session.
   * - ``perspt ledger --recent``
     - Displays recent commits recorded in the Merkle ledger.
   * - ``perspt ledger --rollback <session>``
     - Undoes the named session's newest completed promotion (session id prefix).

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

      Customize providers and models.

   .. grid-item-card:: Agent Deep Dive
      :link: tutorials/agent-mode
      :link-type: doc

      Master autonomous coding.

   .. grid-item-card:: Architecture
      :link: developer-guide/architecture
      :link-type: doc

      Understand the fourteen-crate design.
