.. _tutorial-dashboard-monitoring:

Monitoring Agent Execution with the Dashboard
==============================================

This tutorial walks through using the Perspt web dashboard to observe
an agent session in real time.

Prerequisites
-------------

- Perspt installed (``cargo install --path crates/perspt-cli``)
- A project directory to work in

Step 1: Start an Agent Session
------------------------------

.. code-block:: bash

   perspt agent -w ./myproject "Create a REST API in Rust"

The agent will begin planning and executing tasks. Leave this terminal
running.

Step 2: Launch the Dashboard
----------------------------

Open a **new terminal** and run:

.. code-block:: bash

   perspt dashboard

You should see:

.. code-block:: text

   Perspt dashboard listening on http://127.0.0.1:3000

Step 3: Open the Overview
-------------------------

Navigate to ``http://localhost:3000`` in your browser. The Overview
page shows your active session with:

- **Status badge** - "running" while the agent works
- **Node count** - stable/total nodes projected from the session's
  latest graph revision
- **Events** - how many ledger events the session has recorded

Step 4: Open the Session Detail
-------------------------------

Click your session to open ``/sessions/{id}``. The detail page shows
the task, working directory, status, and detected toolchain, plus node
totals, event and measurement counts, the last and average energy, and
a per-node summary table.

Step 5: Explore the Topology
----------------------------

Click **Topology** (``/sessions/{id}/topology``). The page shows the
work graph as the ledger recorded it:

- **Revision lineage** - every graph revision, newest marked "latest"
- **Nodes of the latest revision** - each node with its state badge
  (stable, running, stopped, blocked, retired)
- **Edge table** - parent -> child relationships

Step 6: Watch Energy Convergence
--------------------------------

Click **Energy** to see the trajectory of measured candidates, in
ledger order: one row per measurement with its node, generation,
energy, and whether it was a hard pass. The summary bar shows the
count, average, minimum, and maximum energy and how many measurements
passed hard. Watch values decrease as accepted checkpoints descend.

Step 7: Check the Backlog
-------------------------

Click **Backlog** for the conditional-capacity diagnostics: node state
counts, how many nodes sit in the backlog (and how many of those are
still unmeasured), the backlog potential Φ(W), its drift, and a
per-node energy table.

Step 8: Understand Decisions
----------------------------

The **Decisions** page is the raw PSP-9 event trace: a flat,
Merkle-chained table with one row per ledger record — its sequence
number, event kind, a truncated payload summary, and the short hash
that chains it to its predecessor. This is the same record that
``perspt replay`` reconstructs.

Step 9: Review Governance
-------------------------

The **Governance** page shows the session's authority epoch and grant
status, recent calibration epochs, adjudication verdicts, and delayed
audit samples still pending a label.

Step 10: Monitor Live Updates
-----------------------------

The dashboard receives Server-Sent Events (SSE) from the server every
2 seconds. You can keep the browser open and watch as the agent
progresses through its task.

When the agent completes, the session status changes to "completed"
(green badge) on the Overview page.
