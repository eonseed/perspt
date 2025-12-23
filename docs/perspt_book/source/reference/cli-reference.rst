.. _cli-reference:

CLI Reference
=============

Complete command-line interface reference for Perspt.

Global Options
--------------

.. code-block:: text

   perspt [OPTIONS] <COMMAND>

.. list-table::
   :header-rows: 1
   :widths: 25 75

   * - Option
     - Description
   * - ``-v, --verbose``
     - Enable verbose logging
   * - ``-c, --config <FILE>``
     - Configuration file path
   * - ``-h, --help``
     - Print help information
   * - ``-V, --version``
     - Print version

Commands Overview
-----------------

.. list-table::
   :header-rows: 1
   :widths: 15 85

   * - Command
     - Description
   * - ``chat``
     - Start interactive TUI chat session
   * - ``agent``
     - Run SRBN agent for autonomous coding
   * - ``init``
     - Initialize project configuration
   * - ``config``
     - Manage configuration settings
   * - ``ledger``
     - Query and manage Merkle ledger
   * - ``status``
     - Show current agent status
   * - ``abort``
     - Abort current agent session
   * - ``resume``
     - Resume paused or crashed session
   * - ``simple-chat``
     - Simple CLI chat mode (no TUI)

perspt chat
-----------

**Usage**: ``perspt chat [OPTIONS]``

Start an interactive TUI chat session.

.. list-table::
   :widths: 25 75

   * - ``-m, --model <MODEL>``
     - Model to use (e.g., ``gpt-5.2``)

**Example**:

.. code-block:: bash

   perspt chat
   perspt chat --model claude-opus-4.5

perspt agent
------------

**Usage**: ``perspt agent [OPTIONS] <TASK>``

Run the SRBN agent for autonomous coding.

**Arguments**:

.. list-table::
   :widths: 25 75

   * - ``<TASK>``
     - Task description or path to task file

**Model Options**:

.. list-table::
   :widths: 30 70

   * - ``--model <MODEL>``
     - Override all model tiers
   * - ``--architect-model <M>``
     - Model for planning (deep reasoning)
   * - ``--actuator-model <M>``
     - Model for code generation
   * - ``--verifier-model <M>``
     - Model for stability checking
   * - ``--speculator-model <M>``
     - Model for fast lookahead

**Execution Options**:

.. list-table::
   :widths: 30 70

   * - ``-w, --workdir <DIR>``
     - Working directory (default: current)
   * - ``-y, --yes``
     - Auto-approve all actions
   * - ``--auto-approve-safe``
     - Auto-approve read-only operations
   * - ``-k, --complexity <K>``
     - Max complexity before approval (default: 5)
   * - ``--mode <MODE>``
     - ``cautious``, ``balanced``, or ``yolo``

**SRBN Options**:

.. list-table::
   :widths: 40 60

   * - ``--energy-weights <α,β,γ>``
     - Lyapunov weights (default: ``1.0,0.5,2.0``)
   * - ``--stability-threshold <ε>``
     - Convergence threshold (default: ``0.1``)

**Limit Options**:

.. list-table::
   :widths: 25 75

   * - ``--max-cost <USD>``
     - Maximum cost in dollars (0 = unlimited)
   * - ``--max-steps <N>``
     - Maximum iterations (0 = unlimited)

**Examples**:

.. code-block:: bash

   perspt agent "Create a calculator"
   perspt agent -y -w ./project "Add tests"
   perspt agent --architect-model gpt-5.2 --actuator-model claude-opus-4.5 "Build API"

perspt init
-----------

**Usage**: ``perspt init [OPTIONS]``

Initialize project configuration.

.. list-table::
   :widths: 25 75

   * - ``--memory``
     - Create ``PERSPT.md`` project memory file
   * - ``--rules``
     - Create ``.perspt/rules.star`` policy file

**Example**:

.. code-block:: bash

   perspt init --memory --rules

perspt config
-------------

**Usage**: ``perspt config [OPTIONS]``

Manage configuration settings.

.. list-table::
   :widths: 25 75

   * - ``--show``
     - Display current configuration
   * - ``--set <KEY=VALUE>``
     - Set a configuration value
   * - ``--edit``
     - Open in ``$EDITOR``

**Examples**:

.. code-block:: bash

   perspt config --show
   perspt config --set default.model=gpt-5.2
   perspt config --edit

perspt ledger
-------------

**Usage**: ``perspt ledger [OPTIONS]``

Query and manage the Merkle change ledger.

.. list-table::
   :widths: 25 75

   * - ``--recent``
     - Show recent commits
   * - ``--rollback <HASH>``
     - Rollback to specific commit
   * - ``--stats``
     - Show ledger statistics

**Examples**:

.. code-block:: bash

   perspt ledger --recent
   perspt ledger --rollback abc123
   perspt ledger --stats

perspt status
-------------

**Usage**: ``perspt status``

Show current agent session status. Displays:

- Session ID
- Current task
- Energy levels (V_syn, V_str, V_log)
- Token usage

perspt abort
------------

**Usage**: ``perspt abort [OPTIONS]``

Abort the current agent session.

.. list-table::
   :widths: 25 75

   * - ``-f, --force``
     - Force abort without confirmation

perspt resume
-------------

**Usage**: ``perspt resume [SESSION_ID]``

Resume a paused or crashed session.

.. list-table::
   :widths: 25 75

   * - ``[SESSION_ID]``
     - Session ID to resume (optional, uses latest if omitted)

perspt simple-chat
-----------------

**Usage**: ``perspt simple-chat [OPTIONS]``

Simple command-line chat mode for scripting and automation.
No TUI - just a prompt with streaming responses.

.. list-table::
   :widths: 25 75

   * - ``-m, --model <MODEL>``
     - Model to use for chat
   * - ``--log-file <FILE>``
     - Log session to file

**Examples**:

.. code-block:: bash

   perspt simple-chat
   perspt simple-chat --log-file session.txt
   echo "Explain Rust" | perspt simple-chat

Exit Codes
----------

.. list-table::
   :header-rows: 1
   :widths: 15 85

   * - Code
     - Meaning
   * - 0
     - Success
   * - 1
     - General error
   * - 2
     - Configuration error
   * - 3
     - Provider/API error
   * - 4
     - Agent aborted by user

See Also
--------

- :doc:`../howto/configuration` - Configuration guide
- :doc:`../howto/agent-options` - Agent options detail
- :doc:`../api/perspt-cli` - API documentation
