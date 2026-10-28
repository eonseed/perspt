.. _api-perspt-dashboard:

``perspt-dashboard``
====================

Real-time web dashboard for Perspt agent monitoring, built with
Axum + Askama + HTMX.

Core Types
----------

.. code-block:: rust

   pub struct AppState {
       pub store: Arc<SessionStore>,        // read-only DuckDB
       pub password: Option<String>,         // optional auth password
       pub session_token: Arc<Mutex<Option<String>>>,
       pub working_dir: PathBuf,
       pub is_localhost: bool,               // controls Secure cookie flag
   }

   pub enum DashboardError {
       Store(anyhow::Error),     // 503 - DB unavailable
       Template(askama::Error),  // 500 - render failure
       Internal(String),         // 500 - generic
   }

``DashboardError`` implements ``IntoResponse`` and renders a styled HTML
error page. Store errors return ``503 Service Unavailable``.

Router
------

``build_router(state: AppState) -> Router`` constructs the full Axum router:

.. list-table::
   :header-rows: 1
   :widths: 15 20 65

   * - Method
     - Route
     - Handler
   * - GET
     - ``/login``
     - ``auth::login_page`` - render login form
   * - POST
     - ``/login``
     - ``auth::login_handler`` - validate password, set session cookie
   * - GET
     - ``/``
     - ``handlers::overview::overview_handler`` - session list
   * - GET
     - ``/sessions/{session_id}``
     - ``handlers::session_detail::session_detail_handler`` - session detail
   * - GET
     - ``/sessions/{session_id}/topology``
     - ``handlers::dag::topology_handler`` - work-graph topology
   * - GET
     - ``/sessions/{session_id}/backlog``
     - ``handlers::backlog::backlog_handler`` - backlog diagnostics
   * - GET
     - ``/sessions/{session_id}/energy``
     - ``handlers::energy::energy_handler`` - energy convergence
   * - GET
     - ``/sessions/{session_id}/decisions``
     - ``handlers::decisions::decisions_handler`` - decision trace
   * - GET
     - ``/sessions/{session_id}/governance``
     - ``handlers::governance::governance_handler`` - governance view
   * - GET
     - ``/sse/{session_id}``
     - ``sse::sse_handler`` - SSE event stream

Static assets are served from ``crates/perspt-dashboard/static/`` under
``/static`` via ``tower_http::services::ServeDir``.

All routes except ``/login`` and ``/static`` are behind
``auth::auth_middleware``. If no password is configured, all requests pass
through.

Auth Middleware
---------------

Cookie-based authentication with random session tokens:

- On successful login, generates a 32-character alphanumeric token
- Stores token in ``AppState::session_token``
- Sets ``perspt_session`` cookie: ``HttpOnly``, ``SameSite=Lax``, ``Path=/``,
  ``Secure`` (when not localhost)
- Middleware checks cookie value against stored token
- No password configured -> open access mode

SSE Stream
----------

The SSE endpoint pushes named events every 2 seconds:

- ``psp9-stats`` - live session summary (ledger event count, measurement
  count, last measured energy)

Each event contains an HTML fragment suitable for HTMX ``sse-swap``.

Templates
---------

Askama templates live in ``crates/perspt-dashboard/templates/``:

- ``base.html`` - layout with navigation, HTMX, and DaisyUI theme
- ``login.html`` - login form
- ``session_base.html`` - shared per-session layout with tab navigation
- ``pages/overview.html`` - session list table
- ``pages/session_detail.html`` - single-session summary
- ``pages/dag.html`` - work-graph topology view
- ``pages/backlog.html`` - backlog diagnostics
- ``pages/energy.html`` - energy component table
- ``pages/governance.html`` - governance view
- ``pages/decisions.html`` - flat Merkle-chained PSP-9 event trace

View Models
-----------

Each page has a corresponding view model in ``src/views/``:

- ``OverviewViewModel`` - sessions, node counts, budgets
- ``SessionDetailViewModel`` - single-session summary
- ``TopologyViewModel`` - work-graph nodes and edges
- ``BacklogViewModel`` - backlog diagnostics
- ``EnergyViewModel`` - per-node energy components
- ``GovernanceViewModel`` - governance evidence
- ``DecisionsViewModel`` - the flat Merkle-chained PSP-9 event trace

``views/psp9.rs`` provides the shared ``LedgerProjection`` these view models
and the SSE stream are built from.
