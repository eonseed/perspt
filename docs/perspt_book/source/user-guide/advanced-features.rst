.. _user-guide-advanced-features:

Advanced Features
=================

Per-Tier Model Selection
------------------------

Assign different models to each SRBN tier:

.. code-block:: bash

   perspt agent \
     --architect-model gemini-pro-latest \
     --actuator-model gemini-3.1-flash-lite-preview \
     --verifier-model gemini-pro-latest \
     --speculator-model gemini-3.1-flash-lite-preview \
     -w ./project "Task description"

Each tier also supports a ``--<tier>-fallback-model`` flag for automatic failover:

.. code-block:: bash

   perspt agent \
     --architect-model gemini-pro-latest \
     --architect-fallback-model gemini-3.1-flash-lite-preview \
     -w ./project "Task"


Energy Weights
--------------

The single energy that gates acceptance is the **quadratic residual
energy**

.. math::

   V(x) = \sum_{e \in E} w_e \, \lVert r_e(x) \rVert^2, \qquad w_e > 0,

a weighted sum of *squared* edge residuals. Each sensor (compiler, type checker,
test oracle, lint, runtime probe, goal-presence) emits a residual with a
non-negative magnitude :math:`r_e`; the energy model squares and weights it. The
familiar component readouts :math:`V_{\text{syn}}`, :math:`V_{\text{str}}`,
:math:`V_{\text{log}}`, :math:`V_{\text{boot}}`, and :math:`V_{\text{sheaf}}` are
**derived rollups** of this same energy, grouped by component, with
:math:`V(x) = \sum_{\text{comp}} V_{\text{comp}}` — there is no second weighting
pass.

The ``--energy-weights "a,b,g"`` flag scales the per-component weights
(``a`` → syntactic, ``b`` → structural, ``g`` → logic) *proportionally*: the
default ``1.0,0.5,2.0`` is the identity reference that leaves the model's
built-in per-class weights untouched, and overrides re-weight relative to it.

.. code-block:: bash

   # Default (identity): use the model's built-in per-class weights
   perspt agent --energy-weights "1.0,0.5,2.0" -w . "Task"

   # Prioritize tests (raise the logic-component weight)
   perspt agent --energy-weights "1.0,0.5,3.0" -w . "Add tests"

   # Prioritize type safety (raise the syntactic-component weight)
   perspt agent --energy-weights "2.0,0.5,2.0" -w . "Add types"

.. note::

   Because the energy is now quadratic, its scale differs from the old linear
   form: a clean candidate still has :math:`V = 0`, but a defect with magnitude
   :math:`r` contributes :math:`w_e r^2` rather than :math:`w_e r`. The stability
   threshold :math:`\varepsilon` is interpreted against this quadratic scale.


Stability Threshold
-------------------

.. code-block:: bash

   # Default: epsilon = 0.10
   perspt agent --stability-threshold 0.10 -w . "Precise task"

   # Relaxed for prototyping
   perspt agent --stability-threshold 0.5 -w . "Quick prototype"


Cost and Step Limits
--------------------

.. code-block:: bash

   # Cap total LLM spend at $5
   perspt agent --max-cost 5.0 -w . "Large refactor"

   # Cap total iterations across all nodes
   perspt agent --max-steps 20 -w . "Iterative task"


Complexity Control
------------------

.. code-block:: bash

   # Set complexity threshold
   perspt agent -k 3 -w . "Simple task"

   # Explicit complexity estimation
   perspt agent --complexity medium -w . "Medium task"


Deferred Testing
----------------

Skip unit tests during per-node verification; only run them at sheaf validation:

.. code-block:: bash

   perspt agent --defer-tests -w . "Speed-optimized generation"

This sets V_log = 0.0 during node coding. Tests run only at the final sheaf stage.


Verifier Strictness
-------------------

.. code-block:: bash

   # Default strictness
   perspt agent --verifier-strictness default -w . "Task"

   # Strict: fail on any warning
   perspt agent --verifier-strictness strict -w . "Production code"

   # Minimal: only fail on errors
   perspt agent --verifier-strictness minimal -w . "Prototype"


LLM Logging and Analytics
--------------------------

.. code-block:: bash

   # Enable LLM call logging to DuckDB
   perspt agent --log-llm -w . "Task"

   # Interactive log browser
   perspt logs --tui

   # Show most recent session
   perspt logs --last

   # Usage statistics (tokens, cost, timing)
   perspt logs --stats


Merkle Ledger
--------------

Every stable node is committed to a content-addressed Merkle ledger stored in
DuckDB. This provides:

- **Auditability** - Full trace of what each node produced
- **Rollback** - Restore to any point in the session
- **Resume** - Continue interrupted sessions with verified context

.. code-block:: bash

   perspt ledger --recent
   perspt ledger --stats


Single-File Mode
----------------

Force the agent to produce a single file without DAG planning:

.. code-block:: bash

   perspt agent --single-file -w . "Create a Python utility script"


Plan Export
-----------

Save the task plan as JSON before execution:

.. code-block:: bash

   perspt agent --output-plan plan.json -w . "Create a web app"
   cat plan.json


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
     - Prompt when complexity exceeds threshold K (default)
   * - ``yolo``
     - Auto-approve everything without review


Planning Policy
---------------

The orchestrator automatically selects a ``PlanningPolicy`` based on workspace
state. The policy controls which agent tiers are activated:

.. list-table::
   :header-rows: 1
   :widths: 25 15 15 45

   * - Policy
     - Architect
     - Speculator
     - When Selected
   * - **LocalEdit**
     - Skipped
     - Skipped
     - Small, localized changes (single-node graph)
   * - **FeatureIncrement**
     - Active
     - Skipped
     - Existing projects (default)
   * - **LargeFeature**
     - Active
     - Active
     - Complex multi-module tasks
   * - **GreenfieldBuild**
     - Active
     - Active
     - New project (no existing files)
   * - **ArchitecturalRevision**
     - Active
     - Active
     - Cross-cutting redesign

When the Speculator is active, it runs a fast lookahead before each node's code
generation, producing risk hints about downstream impacts. This adds latency
but improves first-pass correctness for complex DAGs.

A **FeatureCharter** is auto-created with policy-derived limits before planning:

- **LocalEdit**: max 1 module, 5 files, 3 revisions
- **FeatureIncrement**: max 10 modules, 30 files, 5 revisions
- **LargeFeature / GreenfieldBuild / ArchitecturalRevision**: max 25 modules, 80 files, 10 revisions
