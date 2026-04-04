.. _user-guide-dashboard:

Dashboard
=========

The Perspt web dashboard provides real-time browser-based monitoring of
agent execution. Launch it alongside a running agent session to observe
DAG topology, energy convergence, LLM telemetry, sandbox branches, and
decision traces.

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

**Overview** — Lists all recent sessions with status badges (running,
completed, failed), node completion counts, and budget consumption.
Click any session to drill into its sub-pages.

**DAG Topology** — Shows node cards colored by state (green for
committed/verified, red for failed, blue for running). Displays the
task graph edges below. Useful for understanding the agent's
decomposition of work.

**Energy Convergence** — Displays per-node energy components: V_syn
(syntax), V_str (structure), V_log (tests), V_boot (bootstrap), and
V_sheaf (sheaf validation). The V_total column shows the combined
energy value.

**LLM Telemetry** — Summary stats bar showing total requests, tokens
in/out, and cumulative latency. Below, a table of individual LLM
requests with model, node, token counts, latency, and prompt/response
previews.

**Sandbox Monitoring** — Lists provisional branches with their state
(active, merged, flushed) and sandbox directories. Useful for tracking
which code changes are being explored.

**Decision Trace** — Collapsible sections for each decision category:
escalation reports, sheaf validations, DAG rewrites, plan revisions,
repair footprints, and verification results. Each section shows relevant
details in tabular form.

Authentication
--------------

By default, the dashboard runs without authentication on localhost.
This is safe for local development.

To require a password, add to your ``config.toml``:

.. code-block:: toml

   [dashboard]
   password = "your-secret-password"

When a password is configured, visiting any page redirects to ``/login``.
After entering the correct password, a session cookie is set and
subsequent requests pass through.

Cookie attributes:

- ``HttpOnly`` — not accessible via JavaScript
- ``SameSite=Lax`` — CSRF protection
- ``Secure`` — set when not on localhost
- ``Path=/`` — applies to all dashboard routes

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

The Overview page shows the 50 most recent sessions. Sessions from past
agent runs remain in the DuckDB database and can be browsed even after
the agent has stopped.
