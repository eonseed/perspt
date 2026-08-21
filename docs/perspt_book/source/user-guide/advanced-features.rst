.. _user-guide-advanced-features:

Advanced Features
=================

Per-Role Model Routing
----------------------

The runtime routes each role through the ``[models]`` table of the config
file. Values are fully qualified ``provider::model`` routes so identity,
calibration, and replay never depend on an ambient default provider:

.. code-block:: toml

   [models]
   architect = "vertex::gemini-3.1-pro"
   actuator = "vertex::gemini-3.5-flash"
   verifier = "vertex::gemini-3.1-pro"
   speculator = "vertex::gemini-3.5-flash-lite"
   adjudicator = "vertex::gemini-3.1-pro"

The ``speculator`` entry is consumed only as the fallback route for the
explorer role. On the command line, three roles can be overridden per run:

.. code-block:: bash

   perspt agent \
     --actuator-model vertex::gemini-3.5-flash \
     --explorer-model vertex::gemini-3.5-flash-lite \
     --adjudicator-model vertex::gemini-3.1-pro \
     -w ./project "Task description"

``--model`` is an alias for ``--actuator-model``. ``--fallback-model``
adds an ordered sticky failover route for the actuator; repeat the option
to add more routes. Role-specific models are never silently reused as
actuator fallbacks:

.. code-block:: bash

   perspt agent \
     --actuator-model vertex::gemini-3.5-flash-lite \
     --fallback-model vertex::gemini-3.5-flash \
     --fallback-model vertex::gemini-3.1-pro \
     -w ./project "Task"

The adjudicator, when configured, is a no-tool validator of the realized
diff. Its verdict is an uncalibrated conjunctive veto, never an energy
measurement.


The Acceptance Gate
-------------------

A candidate is accepted when it is a hard pass — every sensor clean — or
when it achieves the required measured energy descent

.. math::

   V(y) \leq V(\text{best}) - \rho_{\text{gate}}.

The energy is measured on the realized filesystem by the language plugins
(compilers, tests, linters, LSP diagnostics), never estimated by a model.
``--rho-gate`` sets the required descent per accepted checkpoint:

.. code-block:: bash

   # Default: rho = 0.5
   perspt agent --rho-gate 0.5 -w . "Precise task"

   # Accept smaller measured improvements
   perspt agent --rho-gate 0.25 -w . "Incremental refactor"

A rejected candidate restores the best accepted checkpoint, so the
workspace never regresses past a measured state.


Budgets
-------

The loop is finite by construction. Each budget is a hard bound, and
exhausting one escalates with a residual certificate rather than looping:

.. list-table::
   :header-rows: 1
   :widths: 38 62

   * - Flag
     - Description
   * - ``--max-turns <N>``
     - Maximum model turns per node (default ``12``).
   * - ``--max-calls-per-turn <N>``
     - Maximum direct and nested tool calls per turn (default ``8``).
   * - ``--rejection-budget <N>``
     - Shared non-descending and recovery budget (default ``4``).
   * - ``--max-parallel <N>``
     - Maximum independent compiler, test, and lint sensors (default ``4``).
   * - ``--max-parallel-nodes <N>``
     - Concurrent work-graph nodes (default ``1``; above 1 requires
       ``--yes``).


Bounded Search
--------------

On a gate failure the runtime can open a bounded search forest instead of
retrying blindly. Branches are sequential and eager, with a hard cap of
three identities per forest; every dead end is recorded as an exact-keyed
no-good so it is never re-proposed, every branch takes pre-action
reservations that settle against observed actuals, and exactly one
candidate is committed through the ordinary acceptance gate. The
``[exploration]`` config block bounds the search:

.. code-block:: toml

   [exploration]
   initial_branches = 1     # branches opened before any expansion trigger
   max_branches = 3         # branch identities per forest (hard cap 3)
   distinct_family = true   # prefer a distinct model family on expansion
   max_workspace_files = 2000
   max_workspace_bytes = 50000000

``distinct_family`` is a prior, never a certificate. The workspace caps
bound the cumulative eager-copy file and byte reservations. Search
activity appears in ``perspt status`` as forest, branch, committed, and
no-good counts.


Prompt Programs
---------------

Every model call is compiled from typed prompt sections into a per-call
prompt program. The program's route, dialect, section provenance, and
tool-surface hash are ledgered, so a session's exact prompts can be
audited after the fact:

.. code-block:: bash

   # List every compiled section: id, version, stage, role, hash
   perspt prompts list

   # Show the programs a session actually compiled, with digests
   perspt prompts explain-session --db-path <FILE> <SESSION_ID>

``--allow-experimental-prompts`` substitutes validated ``[prompts]``
bundle sections live. This is Gate AE and remains experimental until a
change record passes paired evaluation.


Resident Context
----------------

The conversation the model sees is paged: content is stored as
content-addressed pages, and the transported conversation carries
tombstones in place of folded content. The composed request is checked
against the route's input allowance and dialect byte limit before any
call, and the model recalls folded pages only through the governed
``context_recall`` tool. Compactions and refusals are recorded in the
ledger:

.. code-block:: bash

   # Show a session's recorded context events (compactions, refusals)
   perspt context explain-turn --db-path <FILE> <SESSION_ID>


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


Run Summaries
-------------

``--output-summary <FILE>`` writes the terminal run summary as JSON
(session id, node id, outcome, turns used, ledger head, promoted paths)
for CI integration. Any outcome other than a hard pass exits with a
nonzero process status:

.. code-block:: bash

   perspt agent -y --output-summary summary.json -w . "Task"
   cat summary.json


Resume and Replay
-----------------

.. code-block:: bash

   # Resume the most recent session
   perspt resume --last

   # Credential-free audit replay of a session
   perspt replay <SESSION_ID>

Resume verifies the ledger chain, rebuilds staging by folding the ledger,
and re-enters dispatch. Live authority is intentionally never serialized:
resume always rechecks the durable authority epoch and mints fresh
capabilities. ``--persistent-grants`` on the original run stores signed
grant intent so an unattended resume can complete bracketed promotion
intents.


Exploration-Only Runs
---------------------

``--exploration-only`` runs only the read-only exploration phase: a
deterministic repository map plus an interactive explorer tool loop.
Nothing is mutated or promoted:

.. code-block:: bash

   perspt agent --exploration-only -w . "Map the request handling path"


Governed Dependency Mutation
----------------------------

Dependency changes (``cargo add``, ``uv add``, ``npm install``) are an
external effect and are denied by default. ``--allow-dependency-mutation``
grants them, still subject to the kernel and the ledger:

.. code-block:: bash

   perspt agent --allow-dependency-mutation -w . "Add pandas and use it"
