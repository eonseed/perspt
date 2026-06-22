.. _user-guide-agent-mode:

Agent Mode
==========

The agent command activates the experimental SRBN (Stabilized Recursive Barrier
Network) engine for autonomous multi-file code generation. Agent mode is the
current coding-domain implementation of the stability contracts described in
:doc:`../concepts/stability-agent-mode`.

Launching Agent Mode
--------------------

.. code-block:: bash

   perspt agent -w <DIR> "<task>"

   # Examples
   perspt agent -w ./my-project "Create a Python REST API"
   perspt agent -y -w /tmp/demo "Build a Rust CLI tool"

Core Workflow
-------------

The SRBN agent follows a structured closed-loop lifecycle utilizing a quadratic energy model and a mutable work graph:

1. **Detection** - Identify workspace state (greenfield/brownfield) and select
   language plugins
2. **Planning** - Based on the auto-selected ``PlanningPolicy``, either:

   - **LocalEdit** - Skip Architect, create a single-node graph
   - **FeatureIncrement / LargeFeature / GreenfieldBuild / ArchitecturalRevision** -
     Architect decomposes the task into a DAG of nodes with assigned classes.

   A ``FeatureCharter`` is created with policy-derived limits (max modules, files,
   and revisions) to constrain plan scope.
3. **Execution** - The scheduler does *not* walk a precomputed topological
   order. Utilizing a mutable work graph, it runs a closed
   ("fly-by-wire") loop: each round it re-evaluates the graph and selects the
   next *ready* node — one whose dependencies are all complete and whose
   interface seals are satisfied — from a dependency-aware ready queue. For the
   selected node:

   a. Actuator generates a multi-artifact bundle (writes, diffs, commands)
   b. Bundle is applied transactionally
   c. Verification computes Lyapunov energy from syntax, structure, tests,
      bootstrap, and sheaf checks
   d. If :math:`V(x) \leq \varepsilon` (default 0.10), the node is stable;
      otherwise the agent retries, repairs the graph (requeue, split, insert
      interface, or replan a subgraph), or escalates with residual evidence

   Because the graph is mutable, a reworked node is re-picked on a later round
   and newly inserted nodes are executed. When the ready queue empties, a
   goal-completion gate decides whether the task is met, the plan should be
   amended, or the loop should stop. Each graph *revision* remains acyclic.

   .. note::

      The scheduler currently executes **one ready node per round
      (sequential)**. Bounded parallelism — a worker pool with
      file/interface/toolchain leases running non-conflicting nodes
      concurrently (``max_parallel*`` controls) — is planned for a future
      release.

The verification formula is the quadratic residual energy (each sensor
emits a residual of magnitude :math:`r_e \ge 0`):

.. math::

   V(x) = \sum_{e \in E} w_e \, \lVert r_e(x) \rVert^2, \qquad w_e > 0.

The component readouts :math:`V_{syn}, V_{str}, V_{log}, V_{boot}, V_{sheaf}` are
derived rollups of this single energy, so :math:`V(x) = \sum_{\text{comp}}
V_{\text{comp}}` (no separate :math:`\alpha/\beta/\gamma` pass).

4. **Sheaf Validation** - Cross-node contract verification
5. **Review** - In interactive mode: grouped-diff modal with approve/reject/correct
6. **Commit** - Stable nodes are committed to the Merkle ledger

Node Classes
------------

Each DAG node belongs to a class that governs scheduling and verification:

.. list-table::
   :header-rows: 1
   :widths: 25 75

   * - Class
     - Description
   * - **Interface**
     - Public API definitions, type signatures, traits. Scheduled first.
   * - **Implementation**
     - Internal logic. May depend on Interface nodes.
   * - **Integration**
     - Wiring, main entry, config assembly. Scheduled last.

Artifact Bundle Protocol
------------------------

Each node produces a bundle with three artifact types:

.. list-table::
   :header-rows: 1
   :widths: 20 80

   * - Type
     - Description
   * - ``write``
     - Create a new file with full content
   * - ``diff``
     - Modify an existing file (unified diff format)
   * - ``command``
     - Execute a shell command (e.g., ``uv add pandas``)

Ownership closure ensures no two nodes own the same file.

Review Modal (Interactive)
--------------------------

When running without ``--yes``, the review modal presents:

.. code-block:: text

   Review Node 3: Implement data transformer
   ------------------------------------------
   Bundle: 1 created, 1 modified
   + src/transformer.py    [create] (45 lines)
   ~ src/pipeline.py       [diff]   (+3, -1)

   Verification: V_syn OK | V_str OK | V_log OK | V_boot OK
   Energy: V(x) = 0.00

   [y] Approve  [n] Reject  [c] Correct  [e] Edit  [d] Diff

- **y** - Approve and commit to ledger
- **n** - Reject and regenerate
- **c** - Send feedback for correction
- **e** - Open files in your editor
- **d** - Toggle full diff view

Session Management
------------------

.. code-block:: bash

   # Check session state (per-node counts, energy, escalations, correction attempts)
   perspt status

   # Abort the current session
   perspt abort

   # Resume with trust context (shows escalation count, energy, retries)
   perspt resume --last

When resuming, the ``BudgetEnvelope`` (step, cost, and revision caps) is restored
from the database so limits continue from where the session left off.

Dashboard Monitoring
--------------------

While an agent session runs, you can observe it in a browser:

.. code-block:: bash

   # In a separate terminal
   perspt dashboard

Open ``http://localhost:3000`` to see the Overview, DAG, Energy, LLM, Sandbox,
and Decisions pages. The dashboard reads the session store in read-only mode so
it never interferes with the running agent.

See :doc:`dashboard` for full details.

Speculator Lookahead
--------------------

For complex policies (``LargeFeature``, ``GreenfieldBuild``, ``ArchitecturalRevision``),
the Speculator tier runs a fast lookahead before each node's Actuator generation.
It examines pending child nodes and produces risk hints that are injected into the
Actuator's prompt, helping avoid downstream breakage. Simpler policies
(``LocalEdit``, ``FeatureIncrement``) skip the speculator to reduce latency and cost.

See :doc:`advanced-features` for model tiers, energy tuning, and cost controls.

Correction Observability (PSP-7)
---------------------------------

PSP-7 adds structured telemetry to the correction loop. Every correction attempt is
persisted with the parse result state, retry classification, and energy snapshot.

The ``perspt status`` command now shows:

- **Step Timeline** - per-step-type counts and total execution time
- **Correction Attempts** - per-node accepted/rejected attempt counts

The dashboard **Decisions** page includes a dedicated Correction Attempts table with
node, attempt number, parse state, and rejection reason.

In headless mode, the agent summary emits ``[STEPS]`` and ``[CORRECTIONS]`` lines
for CI integration:

.. code-block:: text

   [STEPS] 15 records, 42.3s total
   [CORRECTIONS] 2 node(s) needed correction, 5 total attempts

Live Dashboard Monitoring
--------------------------

Use ``--dashboard`` to start the web monitoring dashboard alongside the agent:

.. code-block:: bash

   perspt agent --dashboard "Build a REST server"
   # Open http://127.0.0.1:3000 in a browser

The embedded dashboard opens a read-only DuckDB connection to the same
database the agent writes to, providing real-time views of DAG topology,
energy convergence, LLM telemetry, and correction-attempt history. Use
``--dashboard-port`` to change the port. The server stops when the agent exits.
