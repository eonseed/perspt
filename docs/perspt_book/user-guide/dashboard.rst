.. _user-guide-dashboard:

Dashboard
=========

The Perspt web dashboard provides real-time browser-based monitoring of
agent execution. Launch it alongside a running agent session to observe
DAG topology, energy convergence, backlog diagnostics, governance state,
and decision traces.

Launching the Dashboard
-----------------------

In a separate terminal from your agent session:

.. code-block:: bash

   perspt dashboard

This starts an Axum web server on ``http://127.0.0.1:3000``. Open that
URL in your browser. The dashboard reads the DuckDB session store in
read-only mode, so it can run safely alongside the agent.

To use a different port:

.. code-block:: bash

   perspt dashboard --port 8080

Dashboard Pages
---------------

**Overview** - Lists all recent sessions with status badges (running,
completed, failed), node completion counts, and budget consumption.
Click any session to drill into its sub-pages.

**DAG Topology** - Shows node cards colored by state (green for
committed/verified, red for failed, blue for running). Displays the
task graph edges below. Useful for understanding the agent's
decomposition of work.

**Backlog** - Conditional-capacity diagnostics over the PSP-9 ledger:
counts of latest-revision nodes per state and the last measured energy
of each backlog node. Every number is a diagnostic projection, not a
stability claim.

**Energy Convergence** - Displays the measured energy trajectory: one
row per candidate measurement with its sequence, node, generation,
energy value, and hard-pass flag. A summary shows the average, minimum,
and maximum energy plus the hard-pass count.

**Decision Trace** - The raw PSP-9 event trace: a flat table of
Merkle-chained ledger records, one row per event with its sequence
number, kind, summary, and short hash.

**Governance** - Calibration epochs with their state, target rho,
threshold, sample count, and model route, plus validator verdicts and
pending delayed audits.

Authentication
--------------

The dashboard binds to ``127.0.0.1`` only and runs without
authentication. This is safe for local development: the server is not
reachable from other machines, and it opens the DuckDB session store in
read-only mode.

Using with a Running Agent
--------------------------

The typical workflow:

1. Start an agent session:

   .. code-block:: bash

      perspt agent -w ./myproject "Create a REST API server"

2. In another terminal, launch the dashboard:

   .. code-block:: bash

      perspt dashboard

3. Open ``http://localhost:3000`` in your browser.

4. The dashboard updates via Server-Sent Events (SSE) every 2 seconds,
   showing live node state changes as the agent works.

Viewing Historical Sessions
----------------------------

The Overview page lists sessions 20 per page, with pagination controls
to browse further back. Sessions from past
agent runs remain in the DuckDB database and can be browsed even after
the agent has stopped.
