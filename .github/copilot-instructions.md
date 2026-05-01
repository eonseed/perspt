# Perspt: Workspace Coding Guide

Concise, repo-specific guidance for the current multi-crate workspace.

## Big picture
- Perspt is now a Rust workspace, not a single-crate app. The active code lives under `crates/`; the legacy top-level `src/` tree is no longer the main runtime surface.
- Root workspace members in `Cargo.toml`: `crates/perspt-core`, `crates/perspt-agent`, `crates/perspt-tui`, `crates/perspt-cli`, `crates/perspt-store`, `crates/perspt-policy`, `crates/perspt-sandbox`, and `crates/perspt`.
- The `perspt` binary entry point is `crates/perspt-cli/src/main.rs`. The umbrella library crate is `crates/perspt/src/lib.rs`.

## Crate boundaries
- `perspt-core`: shared config, events, provider abstraction, plugin registry, normalization helpers, and workspace-wide types.
- `perspt-agent`: SRBN orchestrator, Architect/Actuator/Verifier/Speculator agents, context retrieval, ledger, LSP integration, test runners, and agent tool execution.
- `perspt-tui`: chat TUI, agent monitoring TUI, review modal, logs viewer, dashboard/task tree, theme, and terminal lifecycle helpers.
- `perspt-store`: DuckDB-backed persistence for sessions, nodes, verification history, structural digests, and review outcomes.
- `perspt-policy`: Starlark-based policy engine plus command sanitization/workspace-bound checks.
- `perspt-sandbox`: sandboxed command execution primitives.
- `perspt-cli`: Clap subcommands, mode dispatch, logging initialization, and user-facing command entrypoints.
- `perspt`: meta-package that re-exports the workspace libraries.

## CLI surface
- Commands live in `crates/perspt-cli/src/main.rs`: `chat`, `simple-chat`, `agent`, `init`, `config`, `ledger`, `status`, `abort`, `resume`, and `logs`.
- Running `perspt` with no subcommand defaults to `chat`.
- The `agent` subcommand carries the current control surface: working dir selection, approval flags, complexity, mode, tier-specific models, fallback models, energy weights, stability threshold, cost/step caps, deferred tests, single-file mode, verifier strictness, and `--output-plan`.
- CLI logging is intentionally mode-specific: TUI-heavy modes suppress logs, `simple-chat` shows errors only, and non-TUI admin commands use `info`/`debug`.

## Streaming contract (critical)
- Shared EOT lives in `perspt_core::llm_provider::EOT_SIGNAL` and is currently `<|EOT|>`. Do not reintroduce the old `<<EOT>>` sentinel in prompts or UI code.
- `GenAIProvider` is responsible for streaming content and sending the terminal EOT marker.
- Consumers in `crates/perspt-tui/src/chat_app.rs` and `crates/perspt-cli/src/commands/simple_chat.rs` must stop on the first EOT and ignore duplicates.
- `ChatApp` keeps a `streaming_buffer`, renders assistant output live, and flushes/reset state when streaming completes. If you change chunk termination behavior, update provider and both consumers together.

## Agent and orchestration conventions
- `crates/perspt-agent/src/orchestrator.rs` is the execution center: planning, workspace classification, greenfield project init, context assembly, bundle application, verification, sheaf validation, ledger commits, and TUI event/action wiring.
- Agent prompts and role behavior live in `crates/perspt-agent/src/agent.rs`; keep them aligned with `perspt_core::types` contracts.
- Greenfield project initialization is plugin-driven through `perspt_core::plugin::{PluginRegistry, LanguagePlugin}`. If you add or change language support, update plugin prerequisites, init commands, run/test commands, and workspace detection together.
- Persistence and auditability run through `perspt-agent::ledger` and `perspt-store`. Do not add new verification or review paths that bypass session/ledger recording.
- Current implementation detail: the correction loop is verifier-guided prompting over one shared provider. Do not describe it as an already-independent multi-provider correction barrier unless the code actually changes.

## Workspace-specific notes
- `.perspt-eval/` contains generated evaluation artifacts and scratch sandboxes. Keep it out of commits unless a task explicitly targets evaluation fixtures.
- Prefer fixing logic in the owning crate rather than patching re-export layers.

## CI and verification (match PR gates)
- The pull-request gate is `.github/workflows/ci.yml`.
- Run these exact Rust checks before handing off code that can affect CI:
  - `cargo fmt --all -- --check`
  - `cargo clippy --all-targets --all-features -- -D warnings`
  - `cargo build --verbose --all-features`
  - `cargo test --verbose --all-features -- --test-threads=1`
  - `cargo doc --no-deps --all-features`
- CI runs formatting on stable Ubuntu only, clippy on stable across the OS matrix, and build/tests on all configured matrix entries. Avoid OS-specific assumptions.
- CI also runs a separate `cargo audit` job and a Sphinx documentation build stage. If you change dependencies or docs, validate the relevant parts locally when practical.
- Clippy warnings are CI failures. Keep Rust test modules at the end of files, and prefer arrays/slices over unnecessary `vec![]` allocations in tests.

## DuckDB build: bundled vs system
- DuckDB is pinned in the workspace root `Cargo.toml` (`duckdb = "=1.10501.0"`) **without** the `bundled` feature by default.
- Each crate that (transitively) depends on DuckDB exposes a `bundled` cargo feature that activates `duckdb/bundled` through `perspt-store/bundled`. The chain: `perspt-store → perspt-agent, perspt-tui, perspt-dashboard, perspt-cli, perspt`.
- **Local dev** (default features): links against a system-installed DuckDB library. On macOS with Homebrew: `brew install duckdb`. The `.cargo/config.toml` sets `DUCKDB_LIB_DIR` and `DUCKDB_INCLUDE_DIR` to Homebrew paths (override via env vars if needed).
- **CI / release** (`--all-features` or `--features bundled`): compiles DuckDB from C source for a fully self-contained binary.
- For fast iteration use `cargo clippy --all-targets`, `cargo test`, or `cargo test -p <crate>` (no `--all-features`). Only add `--all-features` when validating the full CI gate.

## Docs and local workflows
- Rust API docs: `cargo doc --open --no-deps --all-features`.
- Sphinx book: `cd docs/perspt_book && uv run make html`.
- VS Code tasks already expose doc generation, PDF build, and link validation; prefer those tasks when working on documentation.

## Editing tips
- Keep provider/config changes centralized in `perspt-core`; avoid duplicating env/config logic in CLI or TUI crates.
- Respect the streaming/EOT contract and avoid blocking TUI event loops.
- When changing manifests, features, or workspace-level dependencies, make sure the checked-in workspace crates still compile under `--all-features`.
- Avoid editing generated logs, `target/`, conversation transcripts, or scratch sandbox data unless the task explicitly targets them.

Questions or mismatches in these instructions should be resolved in favor of the checked-in workspace layout and `.github/workflows/ci.yml`.