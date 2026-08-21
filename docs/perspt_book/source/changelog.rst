.. _changelog:

Changelog
=========

Version 0.6.6 - "沈黛君 (Shen DaiJun)"
--------------------------------------

*In memory of a mother who always gave and never asked anything in return.*

This release implements PSP-9's governed agentic platform: every stochastic action is a proposal, every effect is mediated by a deterministic kernel, every accepted state is measured on the realized candidate, and every reliability claim is conditional on recorded evidence — following the *Stability is All You Need* papers I, II, and III. On top of it, PSP-10 makes the loop's prompts, resident context, and search governed artifacts of the same record.

**Governed Tool Loop & Admissibility Kernel (PSP-9):**

- **SRBN Tool Loop** - New ``perspt-agent::toolloop`` implements Paper II's harness contract as an executable loop: model-issued tool calls become Paper III proposals, the five-clause admissibility kernel (Definition 3.2) decides each one, and the acceptance gate reads the re-measured candidate — never the model's account of it. An adversarial "lying model" fixture proves a model claiming success while every effect is denied can never reach a hard pass.
- **Five-Clause Kernel** - ``check_full_admissibility`` replaces hardcoded contract/barrier passes with registered evaluators; an absent clause is recorded absent, never true. Promotion is one transaction debiting exactly the barrier's certified increment ``c_t`` — the allowance and the debit are the same number.
- **Operational Safety Barrier** - ``perspt-coding`` registers six versioned policy channels (protected paths, sandbox escape, secret exposure, network reachability, dependency policy, resource limits) with exact deterministic increments; correctness evidence stays in ``V``.
- **Recovery Lattice** - One shared, non-replenishing cascade budget across all five levels (Definition 8.1): exhaustion forces strict escalation, containment is unconditional, and every cascade terminates within ``b + k`` steps in one of Theorem 6's four terminal classes.

**Model Portfolio & Transport:**

- **Multi-Provider Portfolio** - The ``[providers]`` table holds several credentialed routes in one process; ``perspt providers`` prints the capability matrix with declared-vs-probed honesty. A one-entry portfolio keeps single-provider setups unchanged.
- **ModelTransport Port** - The SDK declares the object-safe transport port; ``perspt-core`` provides the ``genai`` tool-calling driver; ``perspt-agent::transport`` is the single adapter — the tool loop cannot name a vendor type or credential (Gate S).
- **Portfolio Routing** - Fully qualified ``provider::model`` routes with capability objectives, decorrelated-family review preference, and cross-provider failover chains. Failover is recovery, not retry, and draws on the shared cascade pool.
- **Typed Tool Catalog** - Declarative footprints (``writes ⊆ reads`` by construction), capability-filtered tool specs, and an open ``LanguageId`` adapter registry. ``sed_replace``/``awk_filter`` leave the model-facing catalog; ``edit_file`` fails closed on ambiguity. External tool servers register under the kernel and can never exceed session authority.

**Measured Certificates:**

- **Realizability Interface** - ``srbn`` upgraded 0.1.1 → 0.3.0 (``stabilize_realized``, restore-best policy, hash-chain ledger). Coding uses unmeasured realization with ``projection_mismatch`` telemetry; the analytic Theorem 12.4 path is admitted only when every hypothesis — including Theorem 12.1's step-size bounds — is represented by evidence.
- **Independence Statistics** - Matched-stratum pairwise misses, degenerate marginals using direct joint misses, conservative Hoeffding upper bounds, and certification gating; ``ρ_eff`` reports ``n`` beside it and the correlation-driven energy attenuation helper is removed.
- **Conformal Feasibility Floor** - A declared budget ``ρ`` fixes ``n ≥ 1/ρ − 1`` labeled samples before anything can be autonomously accepted; reject-all is reported as *insufficient calibration*, never as a satisfied budget. Calibration is keyed by full exchangeability strata with shadow/active/stale readiness epochs.
- **Audit Replay** - ``perspt replay <session>`` folds the durable event stream deterministically with tamper detection and no provider credentials; context compaction preserves an exact ``ControlFrame`` and unresolved tool calls, and a stale checkpoint is rebuilt, never patched.

**Recovery, Resume & Calibration:**

- **Recovery Ladder** - The runtime now holds Theorem 6's higher rungs live: level 2 revises the work graph with the exhausted attempt's evidence and re-runs at a new generation, level 3 hands the node to the configured architect route, and level 4 durably revokes the session's authority epoch — also wired to ``perspt abort`` and the TUI quit path.
- **Exact Mid-Loop Resume** - Every gate acceptance writes a durable candidate checkpoint (control frame plus content-addressed file artifacts). ``perspt resume`` rebuilds the accepted candidate from it, re-mints a fresh capability from the grant intersection at the current durable epoch, and re-enters the loop with exactly the remaining budgets; a bumped epoch refuses the resume.
- **Delayed Audit Labels & Conformal Activation** - Every promotion persists an audit-selected, unlabeled calibration sample; ``perspt audit`` ingests delayed labels (single-assignment) and recomputes the stratum threshold over labeled samples only. At the finite sample floor a new immutable epoch activates, after which hard-pass candidates above ``theta`` commit without a prompt with the certified accept ledgered; any other state backs off to approval.
- **Grant Ceilings Enforced** - ``GrantPolicy::mint`` intersects every live capability with the grant ceilings (effects, paths, commands, deny-by-default network, approval strictness) and fails closed; persisted grant intent is signed over canonical bytes and verified against a local trust anchor on resume, never the embedded key.
- **Behavioral Provider Probes** - ``perspt providers --probe`` observes each configured route on a live two-tool round trip (tool calling, multi-tool selection, parallel batching, schema validity) with evidence labelled ``behavioral``; all three configured routes passed live.
- **Exploration-Only Mode** - ``perspt agent --exploration-only`` runs an interactive explorer tool loop under a strictly read-only capability: every call passes the kernel and mutation attempts are recorded denials.
- **Parity Gate Passed** - The governed tool loop ran against the ungoverned whole-file ablation baseline live: 100% hard-pass with zero false stability, failures preserved. No measurement justifies a typed whole-file tool yet; PSP-9 status moves to Final. The ``parity_bench`` harness is retired by PSP-10; evaluation moves to the optional ``perspt-benchmark`` crate, run manually.
- **Governance Surfaces** - A dashboard Governance page shows authority epochs, signed grant intent, calibration epochs, pending delayed audits, and validator verdicts; ``perspt status``, ``perspt ledger``, and ``perspt abort`` now read and act on the PSP-9 surfaces instead of printing placeholders.


**PSP-9 Completion (all phases Done):**

- **Open Execution Plane** - Tool execution moved behind an exact-name handler registry with a conformance fixture proving a new family registers at the composition root (catalog entries + handlers + derived grants) without editing the loop, the candidate, or the node assembly. The legacy 15-arm executor dispatch and its ungoverned ``sed_replace``/``awk_filter``/``run_command`` paths are deleted; grants derive from the assembled catalog intersected with an explicit withheld set.
- **Shipped Tool Families** - A read-only system explorer (``sys_info``, ``sys_processes``, ``sys_disk``, ``sys_env`` — variable names only, values withheld) and a local DB explorer (``db_list``, ``db_schema``, ``db_query`` — SELECT-only over an in-memory read-only DuckDB view with statement allowlisting and row/byte caps) under two new deliberate read-only effect kinds, ``SystemProbe`` and ``DataRead``.
- **Domain Registry Wired** - The runtime selects its domain package from the open ``DomainRegistry`` (``--domain`` or detection); the research domain is registered and reachable; the closed ``CodingLanguage`` enum is retired in favor of the ``LanguageId`` adapter registry; ``PluginRegistry`` accepts registrations.
- **Governed Dependency Mutation** - ``mutate_dependencies`` resolves through each language plugin's dependency commands (``cargo add``, ``uv add``/``uv lock``+``uv sync``, ``npm install``) behind an explicit ``--allow-dependency-mutation`` grant, with Manifest/Lockfile footprints, package-name injection guards, pre-image journaling, and write-ahead external-effect bracketing; manifest and lockfile promote like any governed mutation.
- **Ledger-Folded Conversation (Gate O)** - Model context is a pure fold over digest-chained conversation deltas recorded before they are applied; the durable checkpoint's control frame carries the rolling fold digest, and resume refolds the ledger and refuses a conversation that is not derivable from the record.
- **Commuting Tool-Call Batches (Gate P)** - Same-turn calls are planned over declared footprints: commuting mutators run in the model's own arrival order, precise-footprint reads execute concurrently, and a colliding pair is returned as a ``ToolBatchConflict`` observation, never given an invented order.
- **Bounded Multi-Node Dispatch (Gate P)** - ``--max-parallel-nodes`` (default 1, requires ``--yes``) enables a governed forced-choice ``update_graph`` planning turn and a dispatcher running per-node lifecycle futures on borrowed structured concurrency with cancellation tokens, one shared non-replenishing recovery pool, first live ``GraphWrite`` lease use, and single-flight promotion; ``--max-parallel`` keeps its verifier-fan-out meaning.
- **Proposal Ensembles Superseded (Gate M/T)** - The live proposal ensemble (``[ensemble]`` / ``--ensemble-width``) is removed by PSP-10 in favor of the bounded search forest: each branch is still one ledgered gate decision in its own reversible overlay with selection strictly by measured energy, and a lingering ``[ensemble]`` block is now a fatal startup error pointing to ``[exploration]``.
- **Certified Pairwise Bounds (Gates R/T)** - The deterministic verifier suite records a verdict row beside the adjudicator's for the same candidate and stratum; ``perspt audit`` labels verdicts with the calibration sample in one pass; the independence estimator runs over labeled matched verdicts and surfaces per-pair statistics in ``perspt status`` and the governance dashboard — Certified with ``n`` or the literal ``insufficient evidence``.
- **MCP Wired Into Runtime and Chat (Gates K/L/U)** - ``[[external_tools]]`` servers (stdio and Streamable HTTP) are discovered at node assembly, admitted against the session's derived grant surface, bracketed per call, and replayed from recorded observations without reconnecting; dual-transport conformance fixtures prove the reject/downgrade/execute triple. ``perspt chat`` reuses the same servers under a read-only admission ceiling with inline tool-activity display; ``simple-chat`` is unchanged.
- **Diagnostics and Rollback (Gate R)** - The dashboard renders the work-graph revision lineage (Topology), a Backlog page with the arriving-potential gauge Φ(W) labeled as conditional diagnostics, and the governance independence pair table; ``perspt status`` shows Φ(W), the withheld ``topology_gap``, and certification state; ``perspt ledger --rollback <session>`` restores promoted pre-images through the hardened descriptor-relative path, extends the hash chain with the rollback event, and labels the candidate UNSAFE — the delayed-label source for calibration and independence.
- **Promotion TOCTOU Hardening** - Promotion, resume recovery, and rollback all share one descriptor-relative engine (``openat``/``renameat``/``unlinkat`` with ``O_NOFOLLOW`` component walks): checks and writes share a held directory descriptor, an ancestor swapped for a symlink after admission is refused, and rollback failures are reported, never swallowed. The scheduler now tracks refined generations exactly.
- **Recovery Continuity** - Transient provider errors (429, 5xx, timeouts) are retried with bounded exponential backoff inside the transport, per Gate P's rule that rate-limit waits are throughput delays; recovery-ladder rungs reseed from the previous attempt's best accepted candidate state instead of re-paying the whole cost (``ladder_reseeded``); and containment caused by exhausted transport preserves the session's authority epoch so ``perspt resume`` remains viable after an outage, while governance containment still revokes.
- **Legacy Retirement** - ~19,000 lines of orphaned PSP-5 orchestrator code, dead TUI entry points, the legacy dashboard LLM/Sandbox pages, and the empty legacy status branch are removed; remaining pages read the PSP-9 ledger.

**Typed Prompts, Paged Context & Bounded Search (PSP-10):**

- **Typed Prompt Sections & Codegen** - The new ``perspt-prompt-macros`` crate compiles typed prompt sections into prompt programs; every compiled program and every model call is ledgered with ordered section provenance, the tool-surface hash, and its digest. ``perspt prompts`` (``list``, ``render``, ``lint``, ``manifest``, ``explain-session``) inspects and maintains the section libraries, and ``perspt agent --allow-experimental-prompts`` substitutes validated ``[prompts]`` bundle sections live — experimental until a change record passes paired evaluation (Gate AE).
- **Resident-Context Paging** - Model context is a paged resident set with reserves declared in ``[context]``: evicted pages stay addressable through the ``context_recall`` tool, every compaction and refusal is a recorded context event, and ``perspt context explain-turn`` explains a session's recorded events.
- **Bounded Search Forests** - After a gate failure the runtime can open a bounded search forest (``[exploration]``; at most three branch identities, sequential eager branches): exact no-goods are learned from rejected candidates, and every branch's workspace reservation is durable and pre-action with cost-free refusals.
- **Multi-Node Dispatch with Integration Gate** - ``--max-parallel-nodes`` dispatches work-graph nodes into per-node staging overlays behind one global integration gate, so concurrent nodes promote through a single measured decision point.
- **Crash Resume** - ``perspt resume`` rebuilds a crashed session from the durable record, judging the work graph by its latest recorded state and refolding the conversation by digest chain.
- **Optional Benchmark Crate** - The new ``perspt-benchmark`` crate (perspt-cli Cargo feature ``benchmark``, non-default) evaluates and compares Perspt with other coding agents over a 30-task hidden-oracle corpus: suites ``smoke`` (1 arm, 8 tasks), ``adaptive`` (2 arms), and ``full`` (7 arms). It is credentialed and run manually only — never part of ``cargo test`` or CI.

**Engineering Discipline:**

- **Reproducible Toolchain** - Added ``rust-toolchain.toml`` pinned to Rust 1.97.1 while retaining edition 2021; workspace packages inherit the same MSRV.
- **DuckDB 1.5.5 Boundary** - Exact-pinned ``duckdb`` and ``libduckdb-sys`` at ``1.10505.0``, added dynamic ABI rejection, transactional versioned migrations, catalog-checked nullability changes, and post-DDL checkpoints.
- **Recoverable WAL Repair** - ``perspt db repair --db-path PATH --discard-wal`` records WAL size and SHA-256, makes durable timestamped database/WAL backups, quarantines the WAL, verifies recovery read-only, and restores the original WAL on failure.
- **Shared MCP Runtime Foundation** - Added lazy stdio and Streamable HTTP transports behind one runtime, mode filtering, local effect/footprint admission, namespacing, bounded JSON/SSE results, environment-referenced secrets, replay-only observations, and explicit reconciliation after uncertain completion. Agent/TUI composition remains an acceptance item.
- **PSP-5 API Retirement** - Removed the legacy orchestrator and Merkle-ledger facade from the public agent crate and refused resume of PSP-5 sessions; their existing rows remain inert forensic data.

- **PSP Code Check** - New ``xtask`` checker enforces file ≤ 1408 lines, function ≤ 70 code lines (measured with ``syn``, not brace counting), and line ≤ 108 columns, with a shrink-only baseline ratchet wired into CI (``./check-rules.sh``).
- **Workspace Decomposition** - The 5,889-line orchestrator, ``types.rs``, ``plugin.rs``, ``store.rs``, ``tools.rs``, ``ledger.rs``, and ``verification.rs`` split into focused submodules; workspace version fields now inherit from ``[workspace.package]``.
- **Audit-Driven Hardening** - A full-code audit closed a candidate-restore gap (files created after a checkpoint now roll back exactly via a pre-image journal), separated read paths from the promotable mutated set, routed promotion itself through the five-clause kernel, made the policy engine consume canonical command lines, inverted network-scope delegation to deny-by-default, and made ledger appends O(1) via stage-then-commit.

Version 0.6.2 - "Hózhó"
-----------------------

*Hózhó (Navajo) — A state of perfect balance, harmony, and continuous self-improvement.*

This iteration focuses on bringing greater stability, balance, and refinement to the core architecture. It introduces the full platform SDK implementation alongside the mathematical constructs outlined in the *Stability is All You Need* paper series (Papers I, II, and III) and formalizes PSP-8.

**Stability Is All You Need & Platform SDK (PSP-8):**

- **SDK Platform Crates** - Introduced ``perspt-sdk``, ``perspt-coding``, and ``perspt-research`` to separate domain-neutral orchestration logic from target execution domains.
- **Measured Acceptance Gate** - Implemented the PSP-8 spectral acceptance gate using symmetric eigensolvers (via ``nalgebra``) to compute energy-slope convergence guarantees.
- **Goal Verification Loop** - Integrated target validation kernels with directed LLM corrections and structured ``GoalVerdict``/``TaskType`` contracts.
- **Symbol Extraction & Verification** - Added runtime parsing, dependency tracing, and semantic verification to enforce correct output states.

**Core Capabilities & Providers:**

- **Vertex AI Support** - Added seamless Google Cloud Vertex AI provider support with automatic OAuth2 Bearer token resolution via ``gcp_auth`` ADC.
- **29 LLM Adapters** - Expanded provider configurations to handle 29 unique genai adapters, modernizing model defaults (e.g., ``gemini-3.5-flash``, ``gpt-5.5``, ``claude-fable``).
- **Pipenv Integration** - Added support for ``pipenv`` environment isolation to the Python execution plugin.

**Documentation Overhaul:**

- **Stability Foundations** - Reworked core concept and developer documentation to align with the theoretical foundations of the Stability paper series.
- **Sphinx Book Polish** - Redesigned the Sphinx book layout featuring a retro scientific textbook LaTeX cover, custom plots, and enhanced tables.
- **Refined Clinical Tone** - Rewrote the user manual, tutorials, and configuration guides in a formal, clinical documentation style.

**Workspace Maintenance & Upgrades:**

- **Ratatui Upgrade** - Bumped ``ratatui`` dependency to ``0.30.2`` in ``perspt-tui`` to support modern terminal layouts and components.
- **TUI Textarea Fork Migration** - Migrated from the unmaintained ``tui-textarea`` to ``tui-textarea-2`` version ``0.11.0`` to resolve API compatibility conflicts.
- **Dependency Upgrades** - Bumped ``rustyline`` to ``18.0``, ``toml`` to ``1.1``, and ``tower-http`` to ``0.7`` across the workspace.
- **GitHub Actions Security** - Upgraded GitHub Action workflows to use ``actions/checkout@v7`` for improved stability and security.

Version 0.6.1 - "AKU"
---------------------

*AKU - sharp fixes from sharp ears.*

**Config Coherency & Schema-Driven Settings:**

- **TOML Config Schema & Resolution** - Added a robust TOML configuration schema with smart config-driven provider resolution.
- **Unified Provider Binding** - Bound the configured provider cohesively across all modes including ``chat`` (TUI), ``simple-chat`` (CLI), and the autonomous ``agent`` mode.
- **Refined Config Commands** - Redesigned `perspt config` command to be fully structured, interactive, and improved key initialization/init prompts.

**TUI & CLI Slash Commands:**

- **CLI Simple-Chat Enhancements** - Integrated ``rustyline`` file-based history and introduced interactive slash commands inside simple-chat.
- **TUI Input Navigation & Persistence** - Added persistent history paths and implemented fully UTF-8 safe input navigation in TUI inputs.
- **Conversation Persistence** - Added new conversation save commands to easily export dialogues to local files.

**Advanced Terminal UI Rendering:**

- **LaTeX Math Transpilation** - Integrated real-time LaTeX mathematical transpilation into the markdown rendering pipeline.
- **Intelligent ASCII Table Wrapping** - Implemented self-wrapping logic for ASCII tables in the chat UI to ensure readable presentation on narrow views.

**Documentation:**

- **Comprehensive Feature Guides** - Fully documented TUI/CLI slash commands, config schemas, and keyboard shortcut matrices in the user guides.
- **PSP-8 Stability Documentation** - Added a dedicated Perspt Book chapter explaining how the three *Stability is All You Need* papers shape agent mode, the SDK-first roadmap, and future domain plugins.

**Workspace Maintenance:**

- **Crate Version Alignment** - Bumped version of all workspace crates to ``0.6.1``.

Version 0.6.0 - "kukuza"
------------------------

*Nurturing the foundation, empowering the core.*

**Workspace-Wide Dependency Upgrades:**

- **duckdb Upgrade** - Bumped ``duckdb`` requirement to ``=1.10503.1`` in the workspace root.
- **askama Upgrade** - Bumped ``askama`` requirement to ``0.16`` in ``perspt-dashboard``.
- **diffy Upgrade** - Bumped ``diffy`` requirement to ``0.5`` in ``perspt-agent``.
- **genai Upgrade** - Bumped ``genai`` requirement to ``0.6.1`` in ``perspt-core``.
- **starlark Upgrade** - Bumped ``starlark`` requirement to ``0.14`` in ``perspt-policy``.

**API Alignments & Adaptations:**

- **genai API Alignment** - Updated ``all_model_names`` inside ``llm_provider.rs`` to pass ``()`` for the new ``ProviderConfig`` parameter.
- **starlark API Alignment** - Adapted policy loading in ``engine.rs`` to use ``Module::with_temp_heap`` since ``Module::new`` was deprecated and made private in ``0.14``.

**Specification & Ecosystem Maintenance:**

- **PSP-7 Finalization** - Formally transitioned the PSP-7 specification to ``Final`` status under the PSP-000001 process.

Version 0.5.9 - "xinli guanghua"
----------------------------------

*Perfecting the essence until the work needs no words to shine.*

**PSP-7: Robust Correction Loop Contracts:**

- **Structured Artifact Bundle Format** - Switched correction prompt from free-form ``File: ...`` output to a strict JSON ``{ artifacts: [], commands: [] }`` schema. Includes exact target paths from evidence so the LLM targets the correct files, reducing parse failures.
- **AgentTools Integration** - Routed correction commands through ``execute_correction_command()`` to integrate with plugin policy, user approval gates, and tool failure tracking.
- **Typed Parse Pipeline** - Replaced Option-based bundle extraction with a 5-layer fail-closed typed parse pipeline. Added ``RetryClassification`` (Retarget, MalformedRetry, SupportFileViolation, Replan) population in ``CorrectionAttemptRecord``.
- **Manifest Policy Enforcement** - Added semantic validation to prevent implicit mutation of root manifests (Cargo.toml, package.json) unless explicitly listed as output targets, while preserving legal support files.
- **Strict Budget Exhaustion** - Widened budget exhaustion checks from cost-only to ``any_exhausted()`` to properly respect step and revision caps before attempting LLM calls.

**LLM Provider Maintenance:**

- **genai Upgrade** - Bumped ``genai`` dependency from 0.5.1 to 0.5.3 (stable patch release with bug fixes).
- **Dead Code Cleanup** - Removed ``generate_response_with_history()`` and ``generate_response_with_options()`` which had zero callers across the workspace and were the only methods leaking ``genai::ChatOptions`` into the public API surface.
- **Clippy Fixes** - Fixed `clippy::unnecessary-sort-by` and applied `clippy::collapsible-match` auto-fixes for Rust 1.95 compatibility.

Version 0.5.8 - "Qualitaetsveredelung"
----------------------------------------

*Orchestration State Overhaul Release*

   "Qualitaetsveredelung - the craft of refining what exists until its quality speaks
   for itself. Not new features, but the quiet discipline of making every state
   transition truthful, every metric honest, and every dead path removed."

**Orchestration Correctness (Refs: #112, #113, #114, #116):**

- **SessionOutcome enum** - New ``SessionOutcome`` type (Success, PartialSuccess,
  Failed) derived from actual completed/escalated node counts. The ``Complete``
  event now carries truthful outcomes instead of unconditional ``success: true``.
- **NodeOutcome enum** - ``execute_node()`` returns ``Result<NodeOutcome>`` where
  ``NodeOutcome`` is ``Completed`` or ``Escalated``, replacing the previous
  ``Result<()>`` that could not distinguish outcomes.
- **Correct session outcome derivation** - ``run_orchestration()`` and
  ``run_resumed_inner()`` track completed/escalated counts per node and derive
  the final ``SessionOutcome`` accordingly.
- **Always-on LLM telemetry** - ``call_llm_with_logging()`` now records token
  usage (in/out), latency, and estimated cost via ``record_llm_usage()`` after
  every LLM call, regardless of ``--log-llm``. The flag now only controls verbose
  prompt/response text persistence.
- **Budget envelope persistence** - ``upsert_budget_envelope()`` called after each
  ``BudgetUpdated`` event to persist cost/step tracking to the database.
- **Sandbox-aware context retrieval** - ``ContextRetriever`` in ``step_speculate()``
  uses ``effective_working_dir(idx)`` (the node's sandbox directory) instead of the
  workspace root. Sandbox file tree listings included in actuator and correction
  prompts for better generation grounding.

**Type-Safe State Management (Refs: #114):**

- **NodeState::from_display_str()** - Case-insensitive canonical parser with legacy
  aliases ("running" -> Coding, "stable" -> Completed, "retrying" -> Retry). Replaces
  all ad-hoc string parsing across the codebase.
- **NodeState helpers** - ``is_success()`` (true only for Completed), ``is_active()``
  (true for Coding, Verifying, Planning, Retry, SheafCheck, Committing), and
  ``Display`` impl producing lowercase labels.
- **CLI state cleanup** - All string-based state comparisons in ``status.rs``,
  ``agent.rs``, and ``resume.rs`` replaced with ``NodeState::from_display_str()``
  and type-safe helper methods.

**Dead Code Elimination:**

- Removed 16 unused functions across ``perspt-core``, ``perspt-store``,
  ``perspt-agent``, ``perspt-policy``, ``perspt-tui`` (~234 lines)
- Downgraded ``canonicalize`` to ``pub(crate)`` in ``perspt-policy``
- Removed orphaned ``sha2`` dependency from ``perspt-store``

**Bug Fixes (Refs: #107, #111):**

- **Session status stuck at RUNNING** - Status now persisted in ``end_session()``
  with ``COALESCE``-based finalization guarantee (#111)
- **LLM token counts always zero** - Real provider token usage (prompt +
  completion tokens) extracted from ``genai`` ``ChatResponse::usage`` and
  persisted per request (#107, #110)

**Tests:**

- 7 new tests covering ``NodeState`` parsing round-trips, ``SessionOutcome``
  equality, ``NodeOutcome`` discriminants, and session outcome derivation from
  completed/escalated counts
- Total test count: 359 (up from 352)

**Documentation:**

- Updated README: fixed crate count (8 -> 9), deduplicated dashboard command,
  added missing agent flags (``--single-file``, ``--verifier-strictness``,
  ``--output-plan``, all ``--*-fallback-model``), aligned contributing commands
  with CI gates
- Updated Perspt Book: SRBN architecture (Phase 7 -> Commit & Outcome),
  CLI reference (``logs`` always shows token metrics), developer architecture
  guide (orchestrator lifecycle, NodeOutcome, SessionOutcome in type inventory
  and data flow), workspace crates (removed dead ``is_safe_for_auto_exec``)
- Updated PSP-5: execution flow steps 8-11 with Completed/Escalated paths and
  SessionOutcome derivation, headless output with ``OUTCOME`` line, added
  Orchestration State Overhaul implementation appendix


Version 0.5.7 - "navikaran"
-------------------------------------

*Dashboard UX Polish Release*

   "Bridging the purpose of Ikigai with the momentum of Kaizen - renewal through
   continuous, intentional refinement."

**Dashboard UX Improvements (PSP-6 continued):**

- **Custom DaisyUI 5 themes** - ``perspt-light`` and ``perspt-dark`` themes with
  orange/pink oklch palette (WCAG AA compliant), powered by
  ``@plugin "daisyui/theme"`` blocks
- **Theme toggle** - Navbar button with sun/moon icons, localStorage persistence,
  and migration from legacy theme names
- **Friendly session names** - Deterministic human-readable names (e.g.
  "bold-hawk") derived from session UUIDs via hash-indexed adjective+noun arrays
- **Breadcrumb friendly names** - All six session sub-pages show friendly name
  with UUID-on-hover tooltip
- **Session card layout** - Stacked vertical cards with ``btn-outline`` sub-page
  buttons replacing ghost buttons
- **Task text formatting** - ``whitespace-pre-line`` rendering for readable
  multi-line task descriptions
- **Collapse arrow fix** - ``pe-10`` padding on DAG and LLM collapse summaries
  to prevent arrow overlap with text
- **Decisions page resilience** - All six store queries use
  ``unwrap_or_default()`` instead of ``?`` early-return, preventing 503 errors
  on partial data
- **Paginated overview** - 20 sessions per page with DaisyUI ``join`` pagination
  controls, backed by ``list_sessions_paginated()`` and ``count_sessions()``
  store methods
- **Login page theme** - Updated to ``perspt-light`` default

**CI & Build:**

- **Node.js in CI** - Added ``actions/setup-node@v4`` (Node 22) to CI test matrix
  and release workflows so ``npx @tailwindcss/cli`` runs on all runners

**Store:**

- ``list_sessions_paginated(limit, offset)`` - LIMIT/OFFSET SQL for paginated
  session listing
- ``count_sessions()`` - Total session count for pagination controls


Version 0.5.6 - "ikigai"
-----------------------------------

*SRBN Sandbox Revision Flow Release*

   "A reason for being - the happiness of always being busy with what you love."

**Real-Time Web Dashboard (PSP-6):**

- **perspt-dashboard crate** - Axum 0.8 + Askama 0.15 + HTMX 2 + Tailwind v4/DaisyUI 5
  web interface for monitoring agent execution
- **Read-only store access** - ``SessionStore::open_read_only()`` with DuckDB
  ``AccessMode::ReadOnly`` for safe concurrent reads alongside the agent
- **Six monitoring pages** - Overview (sessions), DAG (task graph), Energy
  (Lyapunov components), LLM (request telemetry), Sandbox (provisional branches),
  Decisions (escalations, sheaf validations, rewrites, plan revisions, repairs,
  verifications)
- **SSE live updates** - Server-Sent Events stream node statistics every 2 seconds
- **Password authentication** - Random token, HttpOnly/SameSite cookie, Secure flag
  on non-localhost deployments
- **``perspt dashboard`` CLI command** - Launches the dashboard server on a
  configurable port
- **12 integration tests** - Route smoke tests, SSE content-type, auth flow
- **Store extensions** - ``get_session_energy_history()``,
  ``get_all_sheaf_validations()``, ``get_all_repair_footprints()``

**SRBN Sandbox Revision Flow (PSP-5 Phases 3-12):**

- **PlanningPolicy** - Adaptive agent gating with 5 policies (LocalEdit,
  FeatureIncrement, LargeFeature, GreenfieldBuild, ArchitecturalRevision).
  ``needs_architect()`` and ``needs_speculator()`` gate agent tier activation
- **FeatureCharter auto-creation** - Policy-derived file/module/revision limits
  created before architect planning so the plan gate has bounds to enforce
- **Speculator lookahead gating** - Speculator tier only activates for LargeFeature,
  GreenfieldBuild, and ArchitecturalRevision policies
- **BudgetEnvelope session restore** - Step/cost/revision caps restored from DB
  during ``resume`` so interrupted sessions honour the original limits
- **Bundle path normalization** - ``filter_bundle_to_declared_paths`` uses
  ``normalize_artifact_path`` for correct comparison of path variants (e.g.
  ``./src/main.rs`` vs ``src/main.rs``)
- **NodeState::Superseded** - New terminal state for plan amendment (Phase 14
  preparation). Updated ``is_terminal()``, ``parse_node_state()``, and
  ``NodeStatus`` conversion
- **Orchestrator module extraction** - ``orchestrator.rs`` split into 9 submodules:
  ``mod.rs``, ``bundle.rs``, ``commit.rs``, ``convergence.rs``, ``init.rs``,
  ``planning.rs``, ``repair.rs``, ``solo.rs``, ``verification.rs``
- **Centralized prompts** - All 15 agent prompts consolidated in ``prompts.rs``
  with constants and ``render_*`` helpers; duplicates removed from ``agent.rs``
- **RepairFootprint-backed correction** - ``build_correction_prompt`` uses
  ``RepairFootprint`` for precise, grounded repair context
- **Greenfield bootstrap ordering** - Plugin-driven project initialization with
  correct pre-sheafify plugin re-detection
- **Provisional branch lifecycle** - Sandbox-first execution with branch creation,
  merge, and flush cascade
- **Escalation classification** - 5 categories with 9 rewrite actions and
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

- **Windows sandbox path normalization** - ``list_sandbox_files`` now returns
  forward-slash-separated relative paths on all platforms
- **Windows workspace-bound validation** - ``validate_workspace_bound`` correctly
  detects absolute paths with Windows drive prefixes (``C:\...``) and normalizes
  backslash path separators before POSIX shell tokenization
- **Clippy and fmt CI compliance** - Resolved ``items_after_test_module`` and
  ``useless_vec`` warnings that were failing CI on all platforms

**Build and CI Improvements:**

- **Removed accidental eval workspace member** - ``.perspt-eval/rust_cli`` removed
  from workspace members; ``.perspt-eval/`` added to ``.gitignore``
- **Stabilized cargo doc** - Added ``doc = false`` to CLI bin target to prevent
  output collision with the ``perspt`` library crate
- **Removed deprecated atty dependency** - Replaced with ``std::io::IsTerminal``
  for TTY detection
- **Lockfile refresh** - Cleared hard ``cargo audit`` vulnerability failures via
  dependency updates

**Documentation:**

- Updated workspace coding instructions to match current multi-crate architecture
- Refreshed all version references across the Perspt Book and Sphinx configuration

Version 0.5.4
-------------

*PSP-5 Compliance Release*

**SRBN Agent Enhancements:**

- **Per-node error recovery** - Retry logic respects ``ErrorType`` classification
  (``Compilation``, ``ToolFailure``, ``ReviewRejection``) with separate counters
- **Multi-file extraction** - Actuator reliably extracts artifact bundles with
  multiple files from LLM responses
- **Multi-artifact bundles** - Bundle protocol correctly handles write, diff, and
  command artifacts in a single node
- **Plugin-driven project initialization** - All plugins use ``uv init --lib``
  (Python), ``cargo init`` (Rust), ``npm init`` (JS) for proper project scaffolding
- **Degraded verification mode** - When tool binaries are missing, falls back to
  heuristic verification with clear warnings
- **Sheaf validation** - 7 validator classes for cross-node contract verification

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
