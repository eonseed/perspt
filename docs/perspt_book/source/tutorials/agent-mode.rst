.. _agent-mode:

Agent Mode: Autonomous Coding Assistant
=======================================

.. versionadded:: 0.5.0

**Agent Mode** uses the **Stabilized Recursive Barrier Network (SRBN)** to autonomously
decompose coding tasks, generate code, and verify correctness via LSP diagnostics.

This implements the FBC (Flow Barrier Control) theory from PSP-000004, where
code generation is treated as a dynamical system that must be stabilized.

Quick Start
-----------

.. code-block:: bash

   # Basic agent mode - create a Python project
   perspt agent "Create a Python calculator with add, subtract, multiply, divide"

   # With explicit workspace directory
   perspt agent -w /path/to/project "Add unit tests for the existing API"

   # Auto-approve all actions (no prompts)
   perspt agent -y "Refactor the parser for better error handling"

How SRBN Works
--------------

The SRBN control loop implements a **Lyapunov stability framework** where code
generation is treated as a dynamical system. The goal is to drive the system
to a stable state where `V(x) < ε` (energy below threshold).

Control Loop Steps
~~~~~~~~~~~~~~~~~~

1. **Sheafification** - The Architect LLM decomposes the task into a JSON TaskPlan
2. **Speculation** - The Actuator LLM generates code for each sub-task
3. **Verification** - LSP diagnostics compute Lyapunov Energy V(x)
4. **Convergence** - If V(x) > ε, feed errors back to LLM and retry
5. **Commit** - When stable, record changes in Merkle Ledger

Lyapunov Energy
~~~~~~~~~~~~~~~

The total energy is computed as:

.. math::

   V(x) = \alpha \cdot V_{syn} + \beta \cdot V_{str} + \gamma \cdot V_{log}

.. list-table:: Energy Components
   :widths: 15 50 15
   :header-rows: 1

   * - Component
     - Source
     - Default Weight
   * - V_syn
     - LSP diagnostics (errors × 1.0, warnings × 0.1)
     - α = 1.0
   * - V_str
     - Structural analysis (placeholder for future)
     - β = 0.5
   * - V_log
     - Test failures weighted by criticality
     - γ = 2.0

Agent Mode Options
------------------

.. code-block:: bash

   perspt agent [OPTIONS] <TASK>

.. list-table:: Command Options
   :widths: 30 70
   :header-rows: 1

   * - Option
     - Description
   * - ``-w, --workspace <DIR>``
     - Working directory (default: current directory)
   * - ``-y, --yes``
     - Auto-approve all actions without prompting
   * - ``-k, --complexity <K>``
     - Max tasks before requiring approval (default: 5)
   * - ``--architect-model <M>``
     - Model for task planning/decomposition
   * - ``--actuator-model <M>``
     - Model for code generation
   * - ``--max-tokens <N>``
     - Token budget limit (default: 100000)
   * - ``--max-cost <USD>``
     - Maximum cost in dollars

Examples
--------

Python Project with Tests
~~~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: bash

   # Create a workspace
   mkdir myproject && cd myproject

   # Let the agent create a Flask API with tests
   perspt agent -y "Create a REST API with Flask including:
   - GET /health endpoint
   - POST /users endpoint  
   - Unit tests for all endpoints"

Model Selection
~~~~~~~~~~~~~~~

Use different models for planning vs code generation:

.. code-block:: bash

   # Use GPT-4o for planning, GPT-4o-mini for code
   perspt agent --architect-model gpt-4o --actuator-model gpt-4o-mini \
     "Implement a binary search tree with insert, search, delete"

Complexity Gating
~~~~~~~~~~~~~~~~~

Pause for approval when plans exceed a task threshold:

.. code-block:: bash

   # Pause if plan has more than 3 tasks
   perspt agent -k 3 "Build authentication module with JWT and refresh tokens"

LSP Integration
---------------

Agent mode uses the ``ty`` type checker for Python to detect errors in real-time.
The LSP client is automatically started and monitors generated files.

Supported LSP Features:

- **textDocument/diagnostic** - Error/warning detection
- **textDocument/definition** - Go to definition
- **textDocument/references** - Find all references
- **textDocument/hover** - Type information

Automatic Correction
~~~~~~~~~~~~~~~~~~~~

When LSP detects errors, the agent:

1. Extracts diagnostic messages (file, line, error text)
2. Constructs a correction prompt with the error context
3. Calls the LLM to regenerate the code
4. Writes the corrected file
5. Re-verifies until stable or max retries reached

Retry Policy
------------

Per PSP-000004, different error types have different retry limits:

.. list-table:: Retry Limits
   :widths: 30 20 50
   :header-rows: 1

   * - Error Type
     - Max Retries
     - Action on Exhaustion
   * - Compilation/type errors
     - 3
     - Escalate to user
   * - Tool failures
     - 5
     - Escalate to user
   * - Review rejections
     - 3
     - Escalate to user

Token Budget & Cost Control
---------------------------

Control spending with token and cost limits:

.. code-block:: bash

   # Limit total tokens
   perspt agent --max-tokens 50000 "Write a simple script"

   # Limit by dollar cost
   perspt agent --max-cost 1.00 "Create a complex module"

The token budget tracks:

- Input tokens (prompt)
- Output tokens (response)
- Estimated cost (based on model pricing)

When the budget is exhausted, the agent will stop and report status.

Test Runner Integration
-----------------------

Agent mode includes a Python test runner using ``uv`` and ``pytest``:

.. code-block:: bash

   # The agent can auto-generate and run tests
   perspt agent -y "Create a calculator module with unit tests"

The test runner:

1. Auto-creates ``pyproject.toml`` if missing
2. Installs pytest via ``uv sync --dev``
3. Runs ``uv run pytest`` and parses results
4. Calculates V_log from test failures

V_log Calculation
~~~~~~~~~~~~~~~~~

Test failures are weighted by criticality (from behavioral contracts):

- **Critical**: weight = 10.0
- **High**: weight = 3.0
- **Low**: weight = 1.0

``V_log = γ × Σ(criticality_weight)``

Architecture
------------

Agent mode uses these components:

.. list-table:: Components
   :widths: 25 75
   :header-rows: 1

   * - Component
     - Purpose
   * - ``SRBNOrchestrator``
     - Main control loop implementing the 7-step SRBN workflow
   * - ``LspClient``
     - LSP client for Python (ty) with diagnostic extraction
   * - ``PythonTestRunner``
     - pytest execution via uv with V_log calculation
   * - ``ContextRetriever``
     - Code search via grep crate for LLM context
   * - ``MerkleLedger``
     - Immutable log of all file changes
   * - ``AgentTools``
     - File read/write, search, command execution

See Also
--------

- :doc:`/concepts/srbn-architecture` - Technical details of SRBN
- :doc:`/howto/configuration` - Configuration options for agent mode
- `PSP-000004 <https://github.com/eonseed/perspt/blob/master/docs/psps/source/psp-000004.rst>`_ - Full specification
