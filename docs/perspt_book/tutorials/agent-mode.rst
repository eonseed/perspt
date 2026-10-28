.. _tutorial-agent-mode:

Agent Mode Tutorial
===================

Master autonomous multi-file code generation with the experimental SRBN engine.

Overview
--------

Agent mode lets Perspt plan, write, test, and commit multi-file projects
autonomously. The SRBN runtime (extended by the dependency-aware mutable work graph)
decomposes tasks into a graph of nodes — each revision acyclic — with each node
owning specific output files, verified by real LSP diagnostics and test runners.
Since v0.6.6 the engine underneath is the PSP-9 governed tool loop —
capability-scoped typed tool calls, measured energy descent, and a
Merkle-chained ledger — extended by the PSP-10 typed prompt and context
mechanisms.

.. admonition:: Experimental Feature
   :class: note

   Agent mode implements the SRBN theoretical framework. The engine is functional
   and usable, but has not yet been benchmarked. Results may vary depending on model
   capability and task complexity.

Prerequisites
-------------

- Perspt v0.6.6+
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

   # Use specific models per route
   perspt agent \
     --actuator-model gemini-3.1-pro \
     --explorer-model gemini-3.5-flash \
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

Step 2: Watch the Governed Loop
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

The agent proceeds through the governed loop phases:

**Detection** - Perspt inspects the workspace and selects a domain package
(here ``coding``); the selected domain is printed at startup. Pass
``--domain`` to pick one explicitly.

**Exploration** - Read-only repository mapping runs before any mutation is
proposed. A cheaper model can serve this route via ``--explorer-model``.

**Planning** - By default the session runs a single work-graph node. With
``--max-parallel-nodes`` above 1 (which requires ``--yes``), one governed
architect turn is forced to call the privileged ``update_graph`` tool. The
proposal declares nodes with output targets and edges and is validated
(acyclic, complete) before acceptance; an invalid or empty proposal falls
back to the single-node graph, and the fallback is recorded in the ledger.

Each node's declared output targets form its write footprint, so the
scheduler keeps concurrent nodes from touching the same files.

**Execution** - A closed-loop scheduler (utilizing the mutable work graph)
re-evaluates the graph each round and dispatches *ready* nodes whose
footprints do not conflict — not a precomputed topological walk. Reworked
nodes are re-picked and inserted nodes are executed on later rounds. For
each node:

1. The actuator proposes typed tool calls (file edits, commands, reads)
2. Effects are applied inside an isolated candidate workspace
3. Compiler, test, and lint sensors measure the checkpoint's energy
4. A checkpoint is accepted only when measured energy descends by at
   least ``--rho-gate``; non-descending attempts draw down the shared
   ``--rejection-budget``

Nodes run one at a time by default; ``--max-parallel-nodes`` raises the
concurrent dispatch bound (above 1 requires ``--yes``).

Step 3: Review Changes
~~~~~~~~~~~~~~~~~~~~~~

The agent runs inside a TUI with three tabs — **Dashboard**, **Tasks**,
and **Diff** — cycled with ``Tab``/``Shift-Tab`` or selected directly with
``1``, ``2``, ``3``. ``Up``/``k`` and ``Down``/``j`` navigate, ``p``
pauses and resumes, ``a`` opens the approval modal, and ``q`` quits.

In interactive mode, the review modal presents the pending change for
approval. Keys inside the modal:

- **y** - Approve
- **n** - Reject
- **c** - Send correction feedback to the agent
- **e** - Edit
- **d** - View diff
- **s** - Skip
- **Left**/**Right** - Move between actions; **Enter** confirms the
  selected action
- **Esc** - Close the modal

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


Model Route Configuration
-------------------------

Assign specialized models to each route:

.. code-block:: bash

   perspt agent \
     --actuator-model gemini-3.1-pro \
     --explorer-model gemini-3.5-flash \
     --adjudicator-model gemini-3.1-pro \
     --fallback-model gemini-3.5-flash \
     -w ./project "Build a web server"

.. list-table::
   :header-rows: 1
   :widths: 25 45 30

   * - Flag
     - Purpose
     - Recommendation
   * - ``--actuator-model``
     - Proposes governed coding tool calls (alias: ``--model``)
     - Strong coding model (e.g., Gemini Pro, Claude Sonnet)
   * - ``--explorer-model``
     - Cheaper read-only repository exploration
     - Fast model (e.g., Gemini Flash)
   * - ``--adjudicator-model``
     - No-tool conjunctive diff adjudication
     - Analytical model (e.g., Gemini Pro)
   * - ``--fallback-model``
     - Ordered sticky actuator fallback; repeat the flag to add routes
     - Reliable alternate provider

The ``[models]`` table in the configuration file can pin the same routes
per role (``actuator``, ``speculator`` for the explorer route,
``adjudicator``); its ``architect`` role supplies the higher-capability
handoff route used by the recovery ladder.


Descent Gating
--------------

Tune how much measured progress each accepted checkpoint must show:

.. code-block:: bash

   # Require steeper measured descent per accepted checkpoint
   perspt agent --rho-gate 1.0 -w . "Add tests"

   # Accept shallower descent for exploratory work
   perspt agent --rho-gate 0.1 -w . "Quick prototype"

Each accepted checkpoint must lower the measured energy (compiler, test,
and lint sensor readings) by at least ``--rho-gate`` (default 0.5).
Checkpoints that fail the gate are refused and draw down the shared
``--rejection-budget``.

Approval
--------

Approval is interactive by default: the agent pauses at promotion and asks
in the review modal. Pass ``-y``/``--yes`` to approve final promotion
automatically. A run without a terminal (CI, piped output) requires
``--yes``; otherwise it fails fast rather than silently escalating.

.. code-block:: bash

   perspt agent --yes -w . "Modify database schema"


Loop Bounds
-----------

.. code-block:: bash

   # Allow more model turns per node (default 12)
   perspt agent --max-turns 20 -w . "Large refactor"

   # Tighten the per-turn tool-call bound (default 8)
   perspt agent --max-calls-per-turn 4 -w . "Small fix"

   # Shrink the shared non-descending and recovery budget (default 4)
   perspt agent --rejection-budget 2 -w . "Iterative improvement"


Exploration and Experimental Prompts
------------------------------------

.. code-block:: bash

   # Read-only exploration: deterministic map plus an explorer tool
   # loop; nothing is mutated or promoted
   perspt agent --exploration-only -w . "Map the module structure"

   # Substitute validated [prompts] bundle sections live (Gate AE:
   # experimental until a change record passes paired evaluation)
   perspt agent --allow-experimental-prompts -w . "Refactor the parser"


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


Auditing Sessions
-----------------

Every model turn and tool effect is recorded in the Merkle-chained ledger;
no logging flag is needed:

.. code-block:: bash

   # Deterministic, credential-free replay of a session
   perspt replay <SESSION_ID>

   # The prompt programs a session actually compiled, with digests
   perspt prompts explain-session --db-path <PATH> <SESSION_ID>

   # A session's recorded context events (compactions, refusals)
   perspt context explain-turn --db-path <PATH> <SESSION_ID>

   # Ledger statistics
   perspt ledger --stats


Best Practices
--------------

1. **Start with a clear task description** - Include language, package structure,
   and testing requirements in the prompt
2. **Use workspace directories** - Always specify ``-w <dir>`` for clarity
3. **Set loop bounds** - Use ``--max-turns`` and ``--rejection-budget`` to
   bound runaway sessions
4. **Review before committing** - In interactive mode, inspect diffs carefully
5. **Use per-route models** - Match model capabilities to each route with
   ``--actuator-model``, ``--explorer-model``, and ``--adjudicator-model``
6. **Track changes** - Use ``perspt ledger`` to review and rollback


Troubleshooting
---------------

**Agent stuck in retry loop:**

- Check LSP is working: ``ty check file.py`` or ``cargo check``
- Relax the descent gate: ``--rho-gate 0.1``
- Raise the shared recovery budget: ``--rejection-budget 8``

**High energy despite clean code:**

- Check test failures: ``uv run pytest -v``
- Review LSP diagnostics
- Replay the session to see what the sensors measured:
  ``perspt replay <SESSION_ID>``

**Plugin not detected:**

- Ensure required binaries are installed (``uv``, ``cargo``, ``node``, etc.)
- Check ``perspt status`` for active plugins

See Also
--------

- :doc:`headless-mode` - Fully autonomous operation
- :doc:`../concepts/srbn-architecture` - SRBN technical details
- :doc:`../howto/agent-options` - Full CLI reference
