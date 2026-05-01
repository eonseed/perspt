.. _changelog:

Changelog
=========

Version 0.5.9 — "心砺光华"
--------------------------

*Perfecting the essence until the work needs no words to shine.*

**PSP-7: Robust Correction Loop Contracts:**

- **Structured Artifact Bundle Format** — Switched correction prompt from free-form ``File: ...`` output to a strict JSON ``{ artifacts: [], commands: [] }`` schema. Includes exact target paths from evidence so the LLM targets the correct files, reducing parse failures.
- **AgentTools Integration** — Routed correction commands through ``execute_correction_command()`` to integrate with plugin policy, user approval gates, and tool failure tracking.
- **Typed Parse Pipeline** — Replaced Option-based bundle extraction with a 5-layer fail-closed typed parse pipeline. Added ``RetryClassification`` (Retarget, MalformedRetry, SupportFileViolation, Replan) population in ``CorrectionAttemptRecord``.
- **Manifest Policy Enforcement** — Added semantic validation to prevent implicit mutation of root manifests (Cargo.toml, package.json) unless explicitly listed as output targets, while preserving legal support files.
- **Strict Budget Exhaustion** — Widened budget exhaustion checks from cost-only to ``any_exhausted()`` to properly respect step and revision caps before attempting LLM calls.

**LLM Provider Maintenance:**

- **genai Upgrade** — Bumped ``genai`` dependency from 0.5.1 to 0.5.3 (stable patch release with bug fixes).
- **Dead Code Cleanup** — Removed ``generate_response_with_history()`` and ``generate_response_with_options()`` which had zero callers across the workspace and were the only methods leaking ``genai::ChatOptions`` into the public API surface.
- **Clippy Fixes** — Fixed `clippy::unnecessary-sort-by` and applied `clippy::collapsible-match` auto-fixes for Rust 1.95 compatibility.

Version 0.5.8 — "Qualitätsveredelung"
----------------------------------------

*Orchestration State Overhaul Release*

   "Qualitätsveredelung — the craft of refining what exists until its quality speaks
   for itself. Not new features, but the quiet discipline of making every state
   transition truthful, every metric honest, and every dead path removed."

**Orchestration Correctness (Refs: #112, #113, #114, #116):**

- **SessionOutcome enum** — New ``SessionOutcome`` type (Success, PartialSuccess,
  Failed) derived from actual completed/escalated node counts. The ``Complete``
  event now carries truthful outcomes instead of unconditional ``success: true``.
- **NodeOutcome enum** — ``execute_node()`` returns ``Result<NodeOutcome>`` where
  ``NodeOutcome`` is ``Completed`` or ``Escalated``, replacing the previous
  ``Result<()>`` that could not distinguish outcomes.
- **Correct session outcome derivation** — ``run_orchestration()`` and
  ``run_resumed_inner()`` track completed/escalated counts per node and derive
  the final ``SessionOutcome`` accordingly.
- **Always-on LLM telemetry** — ``call_llm_with_logging()`` now records token
  usage (in/out), latency, and estimated cost via ``record_llm_usage()`` after
  every LLM call, regardless of ``--log-llm``. The flag now only controls verbose
  prompt/response text persistence.
- **Budget envelope persistence** — ``upsert_budget_envelope()`` called after each
  ``BudgetUpdated`` event to persist cost/step tracking to the database.
- **Sandbox-aware context retrieval** — ``ContextRetriever`` in ``step_speculate()``
  uses ``effective_working_dir(idx)`` (the node's sandbox directory) instead of the
  workspace root. Sandbox file tree listings included in actuator and correction
  prompts for better generation grounding.

**Type-Safe State Management (Refs: #114):**

- **NodeState::from_display_str()** — Case-insensitive canonical parser with legacy
  aliases ("running" → Coding, "stable" → Completed, "retrying" → Retry). Replaces
  all ad-hoc string parsing across the codebase.
- **NodeState helpers** — ``is_success()`` (true only for Completed), ``is_active()``
  (true for Coding, Verifying, Planning, Retry, SheafCheck, Committing), and
  ``Display`` impl producing lowercase labels.
- **CLI state cleanup** — All string-based state comparisons in ``status.rs``,
  ``agent.rs``, and ``resume.rs`` replaced with ``NodeState::from_display_str()``
  and type-safe helper methods.

**Dead Code Elimination:**

- Removed 16 unused functions across ``perspt-core``, ``perspt-store``,
  ``perspt-agent``, ``perspt-policy``, ``perspt-tui`` (~234 lines)
- Downgraded ``canonicalize`` to ``pub(crate)`` in ``perspt-policy``
- Removed orphaned ``sha2`` dependency from ``perspt-store``

**Bug Fixes (Refs: #107, #111):**

- **Session status stuck at RUNNING** — Status now persisted in ``end_session()``
  with ``COALESCE``-based finalization guarantee (#111)
- **LLM token counts always zero** — Real provider token usage (prompt +
  completion tokens) extracted from ``genai`` ``ChatResponse::usage`` and
  persisted per request (#107, #110)

**Tests:**

- 7 new tests covering ``NodeState`` parsing round-trips, ``SessionOutcome``
  equality, ``NodeOutcome`` discriminants, and session outcome derivation from
  completed/escalated counts
- Total test count: 359 (up from 352)

**Documentation:**

- Updated README: fixed crate count (8 → 9), deduplicated dashboard command,
  added missing agent flags (``--single-file``, ``--verifier-strictness``,
  ``--output-plan``, all ``--*-fallback-model``), aligned contributing commands
  with CI gates
- Updated Perspt Book: SRBN architecture (Phase 7 → Commit & Outcome),
  CLI reference (``logs`` always shows token metrics), developer architecture
  guide (orchestrator lifecycle, NodeOutcome, SessionOutcome in type inventory
  and data flow), workspace crates (removed dead ``is_safe_for_auto_exec``)
- Updated PSP-5: execution flow steps 8–11 with Completed/Escalated paths and
  SessionOutcome derivation, headless output with ``OUTCOME`` line, added
  Orchestration State Overhaul implementation appendix


Version 0.5.7 — "navikaran नवीकरण"
-------------------------------------

*Dashboard UX Polish Release*

   "Bridging the purpose of Ikigai with the momentum of Kaizen — renewal through
   continuous, intentional refinement."

**Dashboard UX Improvements (PSP-6 continued):**

- **Custom DaisyUI 5 themes** — ``perspt-light`` and ``perspt-dark`` themes with
  orange/pink oklch palette (WCAG AA compliant), powered by
  ``@plugin "daisyui/theme"`` blocks
- **Theme toggle** — Navbar button with sun/moon icons, localStorage persistence,
  and migration from legacy theme names
- **Friendly session names** — Deterministic human-readable names (e.g.
  "bold-hawk") derived from session UUIDs via hash-indexed adjective+noun arrays
- **Breadcrumb friendly names** — All six session sub-pages show friendly name
  with UUID-on-hover tooltip
- **Session card layout** — Stacked vertical cards with ``btn-outline`` sub-page
  buttons replacing ghost buttons
- **Task text formatting** — ``whitespace-pre-line`` rendering for readable
  multi-line task descriptions
- **Collapse arrow fix** — ``pe-10`` padding on DAG and LLM collapse summaries
  to prevent arrow overlap with text
- **Decisions page resilience** — All six store queries use
  ``unwrap_or_default()`` instead of ``?`` early-return, preventing 503 errors
  on partial data
- **Paginated overview** — 20 sessions per page with DaisyUI ``join`` pagination
  controls, backed by ``list_sessions_paginated()`` and ``count_sessions()``
  store methods
- **Login page theme** — Updated to ``perspt-light`` default

**CI & Build:**

- **Node.js in CI** — Added ``actions/setup-node@v4`` (Node 22) to CI test matrix
  and release workflows so ``npx @tailwindcss/cli`` runs on all runners

**Store:**

- ``list_sessions_paginated(limit, offset)`` — LIMIT/OFFSET SQL for paginated
  session listing
- ``count_sessions()`` — Total session count for pagination controls


Version 0.5.6 — "ikigai 生き甲斐"
-----------------------------------

*SRBN Sandbox Revision Flow Release*

   "A reason for being — the happiness of always being busy with what you love."

**Real-Time Web Dashboard (PSP-6):**

- **perspt-dashboard crate** — Axum 0.8 + Askama 0.15 + HTMX 2 + Tailwind v4/DaisyUI 5
  web interface for monitoring agent execution
- **Read-only store access** — ``SessionStore::open_read_only()`` with DuckDB
  ``AccessMode::ReadOnly`` for safe concurrent reads alongside the agent
- **Six monitoring pages** — Overview (sessions), DAG (task graph), Energy
  (Lyapunov components), LLM (request telemetry), Sandbox (provisional branches),
  Decisions (escalations, sheaf validations, rewrites, plan revisions, repairs,
  verifications)
- **SSE live updates** — Server-Sent Events stream node statistics every 2 seconds
- **Password authentication** — Random token, HttpOnly/SameSite cookie, Secure flag
  on non-localhost deployments
- **``perspt dashboard`` CLI command** — Launches the dashboard server on a
  configurable port
- **12 integration tests** — Route smoke tests, SSE content-type, auth flow
- **Store extensions** — ``get_session_energy_history()``,
  ``get_all_sheaf_validations()``, ``get_all_repair_footprints()``

**SRBN Sandbox Revision Flow (PSP-5 Phases 3-12):**

- **PlanningPolicy** — Adaptive agent gating with 5 policies (LocalEdit,
  FeatureIncrement, LargeFeature, GreenfieldBuild, ArchitecturalRevision).
  ``needs_architect()`` and ``needs_speculator()`` gate agent tier activation
- **FeatureCharter auto-creation** — Policy-derived file/module/revision limits
  created before architect planning so the plan gate has bounds to enforce
- **Speculator lookahead gating** — Speculator tier only activates for LargeFeature,
  GreenfieldBuild, and ArchitecturalRevision policies
- **BudgetEnvelope session restore** — Step/cost/revision caps restored from DB
  during ``resume`` so interrupted sessions honour the original limits
- **Bundle path normalization** — ``filter_bundle_to_declared_paths`` uses
  ``normalize_artifact_path`` for correct comparison of path variants (e.g.
  ``./src/main.rs`` vs ``src/main.rs``)
- **NodeState::Superseded** — New terminal state for plan amendment (Phase 14
  preparation). Updated ``is_terminal()``, ``parse_node_state()``, and
  ``NodeStatus`` conversion
- **Orchestrator module extraction** — ``orchestrator.rs`` split into 9 submodules:
  ``mod.rs``, ``bundle.rs``, ``commit.rs``, ``convergence.rs``, ``init.rs``,
  ``planning.rs``, ``repair.rs``, ``solo.rs``, ``verification.rs``
- **Centralized prompts** — All 15 agent prompts consolidated in ``prompts.rs``
  with constants and ``render_*`` helpers; duplicates removed from ``agent.rs``
- **RepairFootprint-backed correction** — ``build_correction_prompt`` uses
  ``RepairFootprint`` for precise, grounded repair context
- **Greenfield bootstrap ordering** — Plugin-driven project initialization with
  correct pre-sheafify plugin re-detection
- **Provisional branch lifecycle** — Sandbox-first execution with branch creation,
  merge, and flush cascade
- **Escalation classification** — 5 categories with 9 rewrite actions and
  graph surgery support

**Documentation:**

- Updated architecture docs with PlanningPolicy, FeatureCharter, and
  NodeState::Superseded documentation
- Added Planning Policy and Feature Charter sections to SRBN architecture guide
- Updated workspace crates docs with orchestrator submodule structure
- Added speculator lookahead and budget restore documentation to agent mode guide
- Fixed energy weight default (gamma=2.0) in advanced features guide
- Refreshed all version references to 0.5.6


Version 0.5.5
-------------

*PSP-5 Cross-Platform and CI Stabilization Release*

**Cross-Platform Fixes:**

- **Windows sandbox path normalization** — ``list_sandbox_files`` now returns
  forward-slash-separated relative paths on all platforms
- **Windows workspace-bound validation** — ``validate_workspace_bound`` correctly
  detects absolute paths with Windows drive prefixes (``C:\...``) and normalizes
  backslash path separators before POSIX shell tokenization
- **Clippy and fmt CI compliance** — Resolved ``items_after_test_module`` and
  ``useless_vec`` warnings that were failing CI on all platforms

**Build and CI Improvements:**

- **Removed accidental eval workspace member** — ``.perspt-eval/rust_cli`` removed
  from workspace members; ``.perspt-eval/`` added to ``.gitignore``
- **Stabilized cargo doc** — Added ``doc = false`` to CLI bin target to prevent
  output collision with the ``perspt`` library crate
- **Removed deprecated atty dependency** — Replaced with ``std::io::IsTerminal``
  for TTY detection
- **Lockfile refresh** — Cleared hard ``cargo audit`` vulnerability failures via
  dependency updates

**Documentation:**

- Updated workspace coding instructions to match current multi-crate architecture
- Refreshed all version references across the Perspt Book and Sphinx configuration

Version 0.5.4
-------------

*PSP-5 Compliance Release*

**SRBN Agent Enhancements:**

- **Per-node error recovery** — Retry logic respects ``ErrorType`` classification
  (``Compilation``, ``ToolFailure``, ``ReviewRejection``) with separate counters
- **Multi-file extraction** — Actuator reliably extracts artifact bundles with
  multiple files from LLM responses
- **Multi-artifact bundles** — Bundle protocol correctly handles write, diff, and
  command artifacts in a single node
- **Plugin-driven project initialization** — All plugins use ``uv init --lib``
  (Python), ``cargo init`` (Rust), ``npm init`` (JS) for proper project scaffolding
- **Degraded verification mode** — When tool binaries are missing, falls back to
  heuristic verification with clear warnings
- **Sheaf validation** — 7 validator classes for cross-node contract verification

**Bug Fixes:**

- Fixed ``uv init`` to ``uv init --lib`` in ``plugin.rs`` (2 locations) and
  ``test_runner.rs`` (1 location) for correct ``src/`` layout with ``[build-system]``
- Removed dead ``test_check_workspace_requirement`` test from ``orchestrator.rs``

**Documentation:**

- Complete rewrite of the Perspt Book for PSP-5 accuracy
- Updated developer guide with full type inventory and architecture diagrams
- Added tutorials for headless mode, Python ETL, Rust CLI, and scientific computing
- Updated all model names and provider defaults to current versions


Version 0.5.3
-------------

- SRBN energy convergence improvements
- Provisional branch lifecycle management
- Interface seal and flush cascade support
- DuckDB migration from SQLite


Version 0.5.2
-------------

- Initial SRBN orchestrator implementation
- Per-tier model selection (``--architect-model``, etc.)
- Merkle ledger with DuckDB backend
- Plugin system for Python, Rust, and JavaScript
- Agent TUI with dashboard, task tree, and review modal
- Starlark policy engine
- Basic sandbox command execution
- 10 CLI subcommands (chat, agent, simple-chat, init, config, ledger, status,
  abort, resume, logs)


Version 0.5.1
-------------

- Multi-provider chat support
- TUI markdown rendering with code blocks
- Simple CLI mode for scripting
- Streaming response protocol with EOT signal


Version 0.5.0
-------------

- Initial release
- Basic chat interface with OpenAI
- Configuration auto-detection from environment
