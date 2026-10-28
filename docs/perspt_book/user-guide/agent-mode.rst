.. _user-guide-agent-mode:

Agent Mode
==========

**Give Perspt a coding task and a project directory.** The agent edits a candidate copy, runs detected project checks, and requests approval before promotion.
Agent mode is experimental.
A hard pass means the required checks passed; a run can also stop with unresolved errors.
The runtime contracts are described in :doc:`../concepts/stability-agent-mode`.

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

How a task runs
---------------

1. **Read the project.** Perspt explores the files and starts a task. The default run works on one task at a time; optional parallel execution can split larger work into several tasks.
2. **Edit a separate copy.** The agent reads and changes files in a temporary project copy. Permission and command rules limit what it can do.
3. **Build and test.** Perspt runs the detected language tools. The required checks run before each decision to keep an attempt; smaller syntax checks also run during editing.
4. **Try corrections.** Build errors and failing tests tell the agent what went wrong. While checks still fail, an attempt must improve the recorded error score enough to be kept. Otherwise, Perspt restores its best saved version.
5. **Review and apply.** When the required checks pass, an interactive run asks for approval before copying edits into the project. A run that cannot finish within its limits stops and reports the remaining errors.

Applying edits does not create a Git commit.
We review the code and tests before committing, because passing checks can miss bugs.
The internal names for the temporary copy, progress decisions, and file copying are ``CandidateWorkspace``, the acceptance gate, and promotion.
:doc:`../concepts/srbn-architecture` explains these implementation details.

Test Evidence Policies
----------------------

Tests are part of the project being developed, not an automatically immutable
artifact. The coding gate always runs the resulting project suite. The
``[verification] test_policy`` setting controls whether another test-evidence
view is also required:

.. list-table::
   :header-rows: 1
   :widths: 24 76

   * - Policy
     - When to use it
   * - ``evolving`` (default)
     - Feature work, refactoring, API migrations, and corrected behavior where
       implementation and tests legitimately evolve together. New tests run;
       changed or obsolete historical expectations are not resurrected.
   * - ``backward-compatible``
     - Additive changes and bug fixes that promise recognized pre-existing
       test files will continue to pass. The resulting suite and the restored
       regression view must both pass.
   * - ``external-oracle``
     - Work with independently maintained acceptance evidence. Perspt copies
       the configured overlay onto a private candidate view and runs its
       command as an additional blocking test stage. Put withheld material
       outside the workspace.

The first policy says the project's configured suite passed for its resulting
contract; it does not turn tests authored by the actuator into independent
proof. The external policy supplies that independence when the user, CI, or a
release process owns the acceptance suite. Configuration and complete examples
are in :doc:`../configuration`.

Windows Process Isolation
-------------------------

On native Windows, TUI chat and ``simple-chat`` require no special option.
Coding remains strict by default and refuses to start verifier or inspection
processes because Perspt does not yet implement a restricted-token/ACL sandbox.
Use ``--allow-unisolated`` for a single acknowledged run, or configure
``[verification] allow_unisolated = true`` when durable resume is required.
The candidate overlay, deterministic admission kernel, measurement gates,
ledger, and promotion approval remain active, but child processes can access
anything the host user can access. WSL provides the Linux sandbox path.

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

For a single-node run without ``--yes``, the review modal presents the affected
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

After a session finishes, ``perspt dashboard`` opens its database read-only for inspection.
A separate dashboard process can conflict with an active writer's DuckDB lock.
For live monitoring, use the embedded option below.
See :doc:`dashboard` for the page map and interpretation.

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
