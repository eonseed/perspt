.. _tutorial-agent-mode:

Agent Mode Tutorial
===================

Master autonomous multi-file code generation with the experimental SRBN engine.

Overview
--------

Agent mode lets Perspt plan, write, test, and commit multi-file projects
autonomously. The PSP-5 runtime decomposes tasks into a directed acyclic graph
(DAG) of nodes, each owning specific output files, verified by real LSP
diagnostics and test runners.

.. admonition:: Experimental Feature
   :class: note

   Agent mode implements the SRBN theoretical framework. The engine is functional
   and usable, but has not yet been benchmarked. Results may vary depending on model
   capability and task complexity.

Prerequisites
-------------

- Perspt v0.5.6+
- An API key for a capable model
- For Python projects: ``uv`` and ``python3`` installed
- For Rust projects: ``cargo`` and ``rustc`` installed

Basic Usage
-----------

.. code-block:: bash

   # Plan and build a project in a new directory
   perspt agent -w ./my-project "Create a Python calculator package"

   # Auto-approve all changes (headless)
   perspt agent -y -w ./my-project "Create a REST API in Rust"

   # Use specific models per tier
   perspt agent \
     --architect-model gemini-pro-latest \
     --actuator-model gemini-3.1-flash-lite-preview \
     -w ./project "Build an ETL pipeline"


Step-by-Step: Python Calculator
-------------------------------

Step 1: Start the Agent
~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: bash

   mkdir calc-demo && cd calc-demo
   perspt agent -w . \
     "Create a Python calculator package with add, subtract, multiply,
      divide operations. Include type hints, a pyproject.toml with
      build-system, and comprehensive pytest tests."

Step 2: Watch the SRBN Loop
~~~~~~~~~~~~~~~~~~~~~~~~~~~~

The agent proceeds through the PSP-5 phases:

**Detection** — Perspt identifies Python from the task description and selects
the ``python`` plugin:

.. code-block:: text

   [detect] Workspace: greenfield
   [detect] Plugin: python (LSP: ty, tests: pytest, init: uv init --lib)

**Planning** — The Architect decomposes the task into a DAG:

.. code-block:: text

   [plan] TaskPlan: 4 nodes, 1 plugin
   [plan]   node-1: Initialize Python package (Command)
   [plan]   node-2: Create calculator module (Code) -> [node-1]
   [plan]   node-3: Create main entry point (Code) -> [node-2]
   [plan]   node-4: Write pytest tests (UnitTest) -> [node-2]

Each node owns specific output files (ownership closure):

- node-1: ``pyproject.toml``
- node-2: ``src/calculator/__init__.py``, ``src/calculator/core.py``
- node-3: ``src/main.py``
- node-4: ``tests/test_calculator.py``

**Execution** — Nodes execute in topological order. For each node:

1. Actuator generates a multi-artifact bundle (writes, diffs, commands)
2. Files are applied transactionally
3. LSP diagnostics run (V_syn), contracts check (V_str), tests run (V_log)
4. Bootstrap commands check (V_boot)

.. code-block:: text

   [node-2] Generating artifact bundle...
   [node-2] Bundle: 2 files created (write), 0 modified (diff)
   [node-2] Verification:
             V_syn  = 0.00 (ty: 0 errors, 0 warnings)
             V_str  = 0.00 (contracts satisfied)
             V_log  = 0.00 (deferred or passed)
             V_boot = 0.00 (all commands succeeded)
             V_sheaf = 0.00 (pending final check)
             V(x)  = 0.00 < epsilon=0.10 -> STABLE

Step 3: Review Changes
~~~~~~~~~~~~~~~~~~~~~~

In interactive mode, the review modal presents grouped diffs:

.. code-block:: text

   Review Node 2: Create calculator module
   ────────────────────────────────────────
   Bundle: 2 created, 0 modified
   + src/calculator/__init__.py  [create] (3 lines)
   + src/calculator/core.py      [create] (45 lines)

   Verification: V_syn OK | V_str OK | V_log OK | V_boot OK
   Energy: V(x) = 0.00

   [y] Approve  [n] Reject  [c] Correct  [e] Edit  [d] Diff

Actions:

- **y** — Approve and commit to ledger
- **n** — Reject and re-generate from scratch
- **c** — Send correction feedback to the agent
- **e** — Open files in your editor, then return
- **d** — Toggle full unified diff view

Step 4: Inspect Results
~~~~~~~~~~~~~~~~~~~~~~~

After all nodes converge and pass sheaf validation:

.. code-block:: bash

   ls -la
   # pyproject.toml  src/  tests/  uv.lock

   # Run the tests
   cd calc-demo && uv run pytest -v

   # Check the ledger
   perspt ledger --recent


Model Tier Configuration
------------------------

Assign specialized models to each SRBN phase:

.. code-block:: bash

   perspt agent \
     --architect-model gemini-pro-latest \
     --actuator-model gemini-3.1-flash-lite-preview \
     --verifier-model gemini-pro-latest \
     --speculator-model gemini-3.1-flash-lite-preview \
     -w ./project "Build a web server"

.. list-table::
   :header-rows: 1
   :widths: 20 40 40

   * - Tier
     - Purpose
     - Recommendation
   * - Architect
     - Task decomposition, DAG planning
     - Deep reasoning model (e.g., Gemini Pro, Claude Sonnet)
   * - Actuator
     - Code generation, artifact bundles
     - Fast coding model (e.g., Gemini Flash)
   * - Verifier
     - Stability analysis, contract checking
     - Analytical model (e.g., Gemini Pro)
   * - Speculator
     - Branch prediction, lookahead
     - Ultra-fast model (e.g., Gemini Flash Lite)


Energy Tuning
-------------

Customize the Lyapunov energy weights:

.. code-block:: bash

   # Prioritize test passing (higher gamma)
   perspt agent --energy-weights "1.0,0.5,3.0" -w . "Add tests"

   # Prioritize type safety (higher alpha)
   perspt agent --energy-weights "2.0,0.5,1.0" -w . "Add type hints"

   # Custom convergence threshold
   perspt agent --stability-threshold 0.5 -w . "Quick prototype"

Execution Modes
---------------

.. list-table::
   :header-rows: 1
   :widths: 20 80

   * - Mode
     - Behavior
   * - ``cautious``
     - Prompt for approval on every node
   * - ``balanced``
     - Prompt when complexity > K (default)
   * - ``yolo``
     - Auto-approve everything (use with caution)

.. code-block:: bash

   perspt agent --mode cautious -w . "Modify database schema"


Cost and Step Limits
--------------------

.. code-block:: bash

   # Maximum $5 cost
   perspt agent --max-cost 5.0 -w . "Large refactor"

   # Maximum 20 iterations across all nodes
   perspt agent --max-steps 20 -w . "Iterative improvement"


Managing Sessions
-----------------

.. code-block:: bash

   # Show session status: lifecycle counts, energy breakdown, escalations
   perspt status

   # Abort the current session
   perspt abort

   # Resume the last interrupted session with trust context
   perspt resume --last

The ``status`` command shows per-node lifecycle counts (queued, running, verifying,
retrying, completed, failed, escalated), the latest energy breakdown, total retry
count, and recent escalation reports.

The ``resume`` command displays trust context before resuming: escalation count,
last energy state, and total retries across all nodes.


LLM Logging
-----------

.. code-block:: bash

   # Log all LLM calls to the DuckDB store
   perspt agent --log-llm -w . "Debug task"

   # View logs interactively
   perspt logs --tui

   # View most recent session
   perspt logs --last

   # Usage statistics
   perspt logs --stats


Best Practices
--------------

1. **Start with a clear task description** — Include language, package structure,
   and testing requirements in the prompt
2. **Use workspace directories** — Always specify ``-w <dir>`` for clarity
3. **Set cost limits** — Use ``--max-cost`` to prevent runaway spending
4. **Review before committing** — In interactive mode, inspect diffs carefully
5. **Use per-tier models** — Match model capabilities to task complexity
6. **Track changes** — Use ``perspt ledger`` to review and rollback


Troubleshooting
---------------

**Agent stuck in retry loop:**

- Check LSP is working: ``ty check file.py`` or ``cargo check``
- Lower stability threshold: ``--stability-threshold 0.5``
- Use ``--defer-tests`` to skip V_log during coding

**High energy despite clean code:**

- Check test failures: ``uv run pytest -v``
- Review LSP diagnostics
- Adjust weights: ``--energy-weights "0.5,0.5,1.0"``

**Plugin not detected:**

- Ensure required binaries are installed (``uv``, ``cargo``, ``node``, etc.)
- Check ``perspt status`` for active plugins

See Also
--------

- :doc:`headless-mode` — Fully autonomous operation
- :doc:`../concepts/srbn-architecture` — SRBN technical details
- :doc:`../howto/agent-options` — Full CLI reference
