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

- **Status badge** — "running" (blue) while the agent works
- **Node count** — completed/total with failed count if any
- **Budget** — steps used and cost consumed

Step 4: Explore the DAG
------------------------

Click **DAG** next to your session. Node cards show the task
decomposition:

- Green border — committed/verified nodes
- Blue border — currently running
- Red border — failed (will be retried)

The edge table below shows parent → child relationships.

Step 5: Watch Energy Convergence
--------------------------------

Click **Energy** to see per-node energy components. The agent aims
to minimize V_total across all nodes. Watch values decrease as the
verifier-guided correction loop runs.

Step 6: Review LLM Telemetry
-----------------------------

The **LLM** page shows:

- Total requests, tokens in/out, and cumulative latency in the stats bar
- Individual request details with model, prompt preview, and response preview

This is useful for auditing LLM usage and identifying expensive calls.

Step 7: Check Sandbox Branches
------------------------------

The **Sandbox** page lists provisional branches the agent is exploring.
Active branches have sandbox directories; merged/flushed branches show
their final state.

Step 8: Understand Decisions
----------------------------

The **Decisions** page reveals the agent's internal reasoning:

- **Escalation Reports** — when nodes were escalated for re-planning
- **Sheaf Validations** — multi-source consistency checks
- **Rewrites** — DAG modifications (requeued/inserted nodes)
- **Plan Revisions** — architect plan amendments with reasons
- **Repair Footprints** — correction attempts and diagnoses
- **Verification Results** — syntax, build, test, and lint results

Step 9: Monitor Live Updates
-----------------------------

The dashboard receives Server-Sent Events (SSE) from the server every
2 seconds. You can keep the browser open and watch as the agent
progresses through its task.

When the agent completes, the session status changes to "completed"
(green badge) on the Overview page.
