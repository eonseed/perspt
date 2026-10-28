.. _user-guide-dashboard:

Dashboard
=========

**Watch a live agent run in the browser, or inspect completed sessions.** The dashboard shows recorded measurements and decisions from the session store.

Watch a live run
----------------

Start the dashboard inside the agent process:

.. code-block:: bash

   perspt agent --dashboard -w ./myproject "Fix the failing parser test"

Open ``http://127.0.0.1:3000`` while the agent runs.
The embedded dashboard shares the agent's store connection and stops when the agent exits.
Use ``--dashboard-port 8080`` to select another port.

**Use the embedded dashboard for live monitoring.** A separate ``perspt dashboard`` process opens DuckDB read-only and can conflict with an active writer's database lock.

Inspect completed sessions
--------------------------

After the agent exits:

.. code-block:: bash

   perspt dashboard
   # Or select a database and port
   perspt dashboard --db-path /path/to/perspt.db --port 8080

The database must already exist.
The standalone dashboard opens it read-only.
Recent sessions are listed with pagination.

Find the evidence
-----------------

.. list-table::
   :header-rows: 1
   :widths: 28 72

   * - Page
     - What to inspect
   * - **Overview and session detail**
     - Session status, task, work directory, node counts, and measurement summaries.
   * - **Topology**
     - Recorded graph revisions, node states, and dependency edges.
   * - **Energy**
     - One row per candidate measurement with its node, generation, score, and hard-pass flag. Observed scores can rise even when accepted progress descends.
   * - **Backlog**
     - Latest node states, backlog energy, and conditional-capacity diagnostics.
   * - **Decisions**
     - A flat hash-chained event trace with sequence, kind, summary, and hash.
   * - **Governance**
     - Authority, calibration, validator verdicts, and pending audit labels.

The live session statistics use Server-Sent Events every two seconds.
These measurements describe the checks that ran; they do not establish requirements beyond their coverage.

Local access
------------

The CLI binds the dashboard to ``127.0.0.1`` without authentication.
It is intended for local inspection; do not forward the port to other machines.
The read-only dashboard interface does not promote edits.
See :doc:`../howto/dashboard-setup` for setup details.
