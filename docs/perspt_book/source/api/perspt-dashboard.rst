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
       Store(anyhow::Error),     // 503 — DB unavailable
       Template(askama::Error),  // 500 — render failure
       Internal(String),         // 500 — generic
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
     - ``auth::login_page`` — render login form
   * - POST
     - ``/login``
     - ``auth::login_handler`` — validate password, set session cookie
   * - GET
     - ``/``
     - ``handlers::overview::overview_handler`` — session list
   * - GET
     - ``/sessions/{session_id}/dag``
     - ``handlers::dag::dag_handler`` — DAG topology
   * - GET
     - ``/sessions/{session_id}/energy``
     - ``handlers::energy::energy_handler`` — energy convergence
   * - GET
     - ``/sessions/{session_id}/llm``
     - ``handlers::llm::llm_handler`` — LLM telemetry
   * - GET
     - ``/sessions/{session_id}/sandbox``
     - ``handlers::sandbox::sandbox_handler`` — provisional branches
   * - GET
     - ``/sessions/{session_id}/decisions``
     - ``handlers::decisions::decisions_handler`` — decision trace
   * - GET
     - ``/sse/{session_id}``
     - ``sse::sse_handler`` — SSE event stream

All routes except ``/login`` are behind ``auth::auth_middleware``.
If no password is configured, all requests pass through.

Auth Middleware
--------------

Cookie-based authentication with random session tokens:

- On successful login, generates a 32-character alphanumeric token
- Stores token in ``AppState::session_token``
- Sets ``perspt_session`` cookie: ``HttpOnly``, ``SameSite=Lax``, ``Path=/``,
  ``Secure`` (when not localhost)
- Middleware checks cookie value against stored token
- No password configured → open access mode

SSE Stream
----------

The SSE endpoint pushes named events every 2 seconds:

- ``node-stats`` — live summary of node states (total, done, running, failed)

Each event contains an HTML fragment suitable for HTMX ``sse-swap``.

Templates
---------

Askama templates live in ``crates/perspt-dashboard/templates/``:

- ``base.html`` — layout with navigation, HTMX, and DaisyUI theme
- ``login.html`` — login form
- ``pages/overview.html`` — session list table
- ``pages/dag.html`` — node cards and edge table
- ``pages/energy.html`` — energy component table
- ``pages/llm.html`` — stats bar and request table
- ``pages/sandbox.html`` — provisional branch table
- ``pages/decisions.html`` — collapsible sections for escalations,
  sheaf validations, rewrites, plan revisions, repair footprints,
  and verification results

View Models
-----------

Each page has a corresponding view model in ``src/views/``:

- ``OverviewViewModel`` — sessions, node counts, budgets
- ``DagViewModel`` — nodes with state/energy, edges
- ``EnergyViewModel`` — per-node energy components
- ``LlmViewModel`` — requests with token counts, latency, previews
- ``SandboxViewModel`` — provisional branches with sandbox directories
- ``DecisionsViewModel`` — all six decision categories
