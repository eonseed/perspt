.. _user-guide-agent-mode:

Agent Mode
==========

The agent command runs the governed PSP-9 tool loop for autonomous
multi-file code generation. Agent mode is the current coding-domain
implementation of the stability contracts described in
:doc:`../concepts/stability-agent-mode`.

Launching Agent Mode
--------------------

.. code-block:: bash

   perspt agent -w <DIR> "<task>"

   # Examples
   perspt agent -w ./my-project "Create a Python REST API"
   perspt agent -y -w /tmp/demo "Build a Rust CLI tool"

A run without a terminal (CI, pipes) requires ``--yes``: promotion approval
cannot be prompted without one, so the CLI fails fast instead of silently
escalating a fully verified run. Any outcome other than a hard pass exits
with a nonzero process status, so scripts never read a stopped node as
success.

Core Workflow
-------------

The agent runs a closed loop in which every model proposal is governed and
every acceptance is measured:

1. **Planning** - One governed architect turn, restricted to the
   ``update_graph`` tool, decomposes the task into a work graph of typed
   nodes. Simple tasks stay a single node.
2. **Proposal** - The actuator proposes typed tool calls (file writes,
   edits, commands) against a disposable candidate overlay. The real
   workspace is never touched by a proposal.
3. **Admission** - Every proposal passes the deterministic five-clause
   kernel — authority, contract, effect scope, barrier increment, and risk
   budget — before it takes effect. A denied call is recorded in the ledger
   and costs the model a correction, never the workspace a mutation.
4. **Measurement** - Admitted mutations are measured on the realized
   filesystem by the language plugins (compilers, tests, linters, LSP
   diagnostics), which produce the scalar energy :math:`V(y)`.
5. **Acceptance** - A candidate is accepted when it is a hard pass, or when
   it achieves the required measured descent
   :math:`V(y) \leq V(\text{best}) - \rho_{\text{gate}}` (``--rho-gate``,
   default ``0.5``). A rejected candidate restores the best accepted
   checkpoint.
6. **Ledger** - Every proposal, denial, measurement, gate decision, and
   checkpoint is appended to a hash-chained ledger *before* it is used.

On a gate failure the runtime can open a bounded search forest: a small
number of eager branches with exact-keyed no-goods, pre-action reservations
that settle against observed actuals, and exactly one candidate committed
through the ordinary gate. See :doc:`advanced-features` for the
``[exploration]`` configuration.

Multi-Node Dispatch
-------------------

By default the work graph executes one node at a time. ``--max-parallel-nodes``
raises the number of concurrent nodes; values above 1 require ``--yes``
because promotion approval cannot be prompted per node while other nodes
run. The dispatcher schedules by footprint conflict — two nodes whose
declared file footprints overlap never run concurrently — and each node
writes into content-addressed staging. Conflict detection is
dependency-aware (downstream refinements win by edge precedence), and one
global integration gate measures the merged result before anything is
promoted.

Node Classes
------------

Each work-graph node carries a class describing the kind of work it
performs:

.. list-table::
   :header-rows: 1
   :widths: 25 75

   * - Class
     - Description
   * - **Explore**
     - Read-only investigation of the workspace.
   * - **Plan**
     - Task decomposition and graph revision.
   * - **Implement**
     - Code changes toward the node goal.
   * - **Verify**
     - Measurement-only verification work.
   * - **Test**
     - Test authoring and execution.
   * - **Integrate**
     - Merging staged work across nodes.
   * - **Repair**
     - Recovery from a failed or rejected node.
   * - **Interface**
     - Public API definitions, type signatures, traits.

The planner currently emits ``Implement`` nodes; the full taxonomy is the
SDK contract that dispatch and the ledger record.

Review Modal (Interactive)
--------------------------

When running without ``--yes``, the review modal presents the affected
files, the verification gate results (syntax, build, tests, lint, plus
test pass/fail counts), the scalar energy ``V(x)``, and a degraded flag
listing any sensors that could not run.

- **y** - Approve and promote
- **n** - Reject and restore the best checkpoint
- **c** - Send feedback for correction
- **e** - Open files in your editor
- **d** - Toggle full diff view
- **s** - Skip this review

``Left``/``Right`` move the button selection, ``Enter`` confirms the
selected decision, and ``Esc`` dismisses the modal.

Session Management
------------------

.. code-block:: bash

   # Check session state (ledger, measurements, energy, gate, denials)
   perspt status

   # Abort the current session by revoking its authority epoch
   perspt abort

   # Resume the most recent session
   perspt resume --last

``perspt status`` prints the ledger event count and head, measurement and
denial counts, the last energy and gate decision, search forest, branch,
and no-good counts, adjudicator verdicts, the capacity :math:`\Phi(W)`, and
validator independence statistics. ``perspt resume`` prints the session id,
task, working directory, and status, then verifies the ledger chain before
continuing; live authority is never serialized, so resume always mints
fresh capabilities.

Domains
-------

``--domain`` selects the domain package (``coding``, ``research``); by
default the best-matching domain is detected from the workspace, with
``coding`` as the fallback.

Exploration-Only Runs
---------------------

``--exploration-only`` runs only the read-only exploration phase: a
deterministic repository map plus an interactive explorer tool loop.
Nothing is mutated or promoted, and the run never prompts, so it works
without ``--yes`` even in non-interactive contexts.

Headless Summary
----------------

In headless mode the agent prints the terminal summary lines — outcome,
session id, turns used, ledger head, and promoted paths:

.. code-block:: text

   Outcome: HardPass
   Session: 019820f3-...
   Turns: 6
   Ledger head: 4f2c9a1b8d7e6a50
   Promoted paths: src/api.py, tests/test_api.py

``--output-summary <FILE>`` writes the same summary as JSON for CI
integration.

Dashboard Monitoring
--------------------

While an agent session runs, you can observe it in a browser:

.. code-block:: bash

   # In a separate terminal
   perspt dashboard

Open ``http://localhost:3000`` to see the Overview, Session detail,
Topology, Backlog, Energy, Decisions, and Governance pages. The Decisions
page is a flat Merkle-chained event trace. The dashboard reads the session
store in read-only mode so it never interferes with the running agent.

See :doc:`dashboard` for full details.

Live Dashboard Monitoring
--------------------------

Use ``--dashboard`` to start the web monitoring dashboard alongside the
agent:

.. code-block:: bash

   perspt agent --dashboard "Build a REST server"
   # Open http://127.0.0.1:3000 in a browser

The embedded dashboard reads through the same database connection the agent
writes to, providing real-time views of work-graph topology, energy
convergence, and the ledgered decision trace. Use ``--dashboard-port`` to
change the port. The server stops when the agent exits.

.. note::

   The embedded dashboard runs unauthenticated on ``127.0.0.1``. It is
   intended for local monitoring only; do not expose the port beyond the
   local machine.

See :doc:`advanced-features` for per-role model routing, the acceptance
gate, budgets, and search configuration, and :doc:`../howto/agent-options`
for the full flag reference.
