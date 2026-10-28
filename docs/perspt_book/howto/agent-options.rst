.. _howto-agent-options:

Agent Options Reference
=======================

``perspt agent`` runs the governed PSP-9 tool loop, extended by PSP-10 with
bounded search forests, resident-context paging, prompt programs, and
multi-node dispatch. The CLI exposes only controls consumed by that runtime.

Core Options
------------

.. list-table::
   :header-rows: 1
   :widths: 38 62

   * - Flag
     - Description
   * - ``<TASK>``
     - Task description.
   * - ``-w, --workdir <DIR>``
     - Workspace root.
   * - ``-y, --yes``
     - Approve final promotion within already-granted authority. It does not
       grant network, shell, policy, or out-of-workspace effects. A
       non-interactive run requires ``--yes``: promotion approval cannot be
       prompted without a terminal. ``--max-parallel-nodes`` above ``1`` also
       requires it.
   * - ``--model <PROVIDER::MODEL>``
     - Override the primary actuator route.
   * - ``--domain <DOMAIN>``
     - Domain package to run (e.g. ``coding``, ``research``); default:
       detect.
   * - ``--exploration-only``
     - Run only the read-only exploration phase: deterministic map plus an
       interactive explorer tool loop; nothing is mutated or promoted.
   * - ``--allow-dependency-mutation``
     - Grant governed dependency mutation (``cargo add``, ``uv add``,
       ``npm install``).
   * - ``--allow-unisolated``
     - Explicitly permit verifier, inspection, and LSP child processes to run
       without an OS sandbox. Intended for acknowledged native Windows use or
       an embedder that isolates the complete Perspt process. Candidate,
       capability, gate, ledger, and approval controls remain active, but the
       child has the host user's filesystem and network authority.
   * - ``--allow-experimental-prompts``
     - Substitute validated ``[prompts]`` bundle sections live (Gate AE:
       experimental until a change record passes paired evaluation).
   * - ``--db-path <PATH>``
     - Path to the PSP-9 ledger database (defaults to the platform data
       directory).
   * - ``--output-summary <FILE>``
     - Write the terminal PSP-9 run summary as JSON.
   * - ``--persistent-grants``
     - Store signed grant intent. Resume always rechecks the durable authority
       epoch and must mint a fresh live capability.

Finite Harness
--------------

.. list-table::
   :header-rows: 1
   :widths: 38 62

   * - Flag
     - Description
   * - ``--rho-gate <VALUE>``
     - Required measured energy descent per accepted checkpoint (default
       ``0.5``).
   * - ``--max-turns <N>``
     - Maximum model turns per node (default ``12``).
   * - ``--max-calls-per-turn <N>``
     - Maximum direct and nested tool calls per turn (default ``8``).
   * - ``--rejection-budget <N>``
     - Shared non-descending and recovery budget (default ``4``).
   * - ``--max-parallel <N>``
     - Maximum independent compiler, test, and lint sensors (default ``4``).
   * - ``--max-parallel-nodes <N>``
     - Concurrent work-graph nodes (default ``1``). Values above ``1``
       require ``--yes``.

Model Portfolio
---------------

.. list-table::
   :header-rows: 1
   :widths: 42 58

   * - Flag
     - Description
   * - ``--actuator-model <M>``
     - Primary implementation route.
   * - ``--explorer-model <M>``
     - Cheap, no-tool repository-map summarizer.
   * - ``--adjudicator-model <M>``
     - Optional no-tool validator of the realized diff. Its verdict is an
       uncalibrated conjunctive veto, never an energy measurement.
   * - ``--fallback-model <M>``
     - Add an ordered sticky actuator failover route. Repeat the option to add
       more routes. Role-specific models are never silently reused as
       actuator fallbacks.

Dashboard
---------

``--dashboard`` starts the read-only monitoring server alongside the agent.
``--dashboard-port <PORT>`` selects its port (default ``3000``).

Examples
--------

.. code-block:: bash

   perspt agent -y -w /tmp/proj "Create a Python calculator"

   perspt agent \
     --actuator-model vertex::gemini-3.5-flash-lite \
     --explorer-model vertex::gemini-3.5-flash-lite \
     --fallback-model vertex::gemini-3.5-flash \
     --rho-gate 0.25 \
     --max-turns 16 \
     --max-calls-per-turn 8 \
     --rejection-budget 5 \
     --max-parallel 4 \
     -w ./project "Build a web server"

   perspt agent --dashboard --dashboard-port 8080 -w ./myapp "Add unit tests"

   # Native Windows reduced-isolation mode
   perspt agent --allow-unisolated -w . "Fix the parser"
