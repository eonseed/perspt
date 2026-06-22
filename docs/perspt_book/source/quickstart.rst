.. _quickstart:

Quick Start
===========

This document outlines the minimal commands required to install, configure, and execute the Perspt terminal application and the autonomous agent mode.

Prerequisites
-------------

Verify that the target system satisfies the following conditions:

- **Rust Toolchain**: Version 1.82+ is required for compiling from source.
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

   # Run with specific models for Architect and Actuator roles
   perspt agent \
     --architect-model gemini-3.1-pro \
     --actuator-model gemini-3.5-flash \
     -w ./project "Create an ETL pipeline in Python"

An execution run displays the active scheduling, verifier execution, and SDK gate telemetry. Below is a clinical trace of a typical autonomous run:

.. code-block:: text

   🚀 Starting SRBN agent session (ID: 01H2X...)
   [Step 1/3] Planning Task...
     Running Architect (model: gemini-3.1-pro)
     Decomposing task: "Create Python calculator package..."
     Generated 3 graph nodes (Interface -> Implementation -> Integration)
     Ownership closure verified.
   
   [Step 2/3] Closed-Loop Scheduling...
     Picking ready node: node_1_interface
       Generating outputs: [src/calc/__init__.py]
       Executing verification stage: SyntaxCheck... ✅ passed (V_syn = 0.0)
       Executing verification stage: StructuralCheck...
         goal-presence check: expected [add, sub, mul, div]
         missing required symbols: [add, sub, mul, div]
         goal-presence FAIL: SymbolMismatch residual raised.
         SDK V=Σwₑrₑ²=8.0 [syn 0.00|str 8.00|log 0.00|boot 0.00|sheaf 0.00] gate=rejected (Δ=8.0) ρ=0.5 bound≤3 (1 residuals)
       Generating correction prompt for node_1_interface (generation 1)...
       Generating outputs: [src/calc/__init__.py] (generation 2)
       Executing verification stage: StructuralCheck... ✅ passed (V_str = 0.0)
       SDK V=Σwₑrₑ²=0.0 [syn 0.00|str 0.00|log 0.00|boot 0.00|sheaf 0.00] gate=hard-pass ρ=0.5 bound≤3 (0 residuals)
       Committing node_1_interface to Merkle ledger.
   
     Picking ready node: node_2_implementation
       Generating outputs: [src/calc/core.py]
       Executing verification stage: TestCheck... pytest failed 1 test case.
       SDK V=Σwₑrₑ²=2.0 [syn 0.00|str 0.00|log 2.00|boot 0.00|sheaf 0.00] gate=rejected (Δ=2.0) ρ=0.5 bound≤3 (1 residuals)
       Generating correction prompt for node_2_implementation (generation 1)...
       Generating outputs: [src/calc/core.py] (generation 2)
       Executing verification stage: TestCheck... ✅ all tests passed.
       SDK V=Σwₑrₑ²=0.0 [syn 0.00|str 0.00|log 0.00|boot 0.00|sheaf 0.00] gate=hard-pass ρ=0.5 bound≤3 (0 residuals)
       Committing node_2_implementation to Merkle ledger.
   
     Picking ready node: node_3_integration
       Executing verification stage: SheafCheck... ✅ passed (V_sheaf = 0.0)
       SDK V=Σwₑrₑ²=0.0 [syn 0.00|str 0.00|log 0.00|boot 0.00|sheaf 0.00] gate=hard-pass ρ=0.5 bound≤3 (0 residuals)
       Committing node_3_integration to Merkle ledger.
   
   [Step 3/3] Settle Session...
     Session converged successfully. Output Merkle root: 9f8a7e...
     Completed nodes: 3, Escalated nodes: 0.
     Session outcome: Success. Exiting.

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
   * - **Status**
     - ``perspt status``
     - Query metrics of the active agent session.

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
   * - ``perspt ledger --rollback <hash>``
     - Rolls back the workspace state to a specific Merkle commit hash.
   * - ``perspt logs --tui``
     - Launches the interactive LLM request log viewer.

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

      Understand the twelve-crate design.
