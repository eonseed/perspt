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
  - `cargo clippy --locked --workspace --all-targets -- -D warnings`
  - `cargo test --locked --workspace --all-targets -- --test-threads=1`
  - `cargo check --locked -p perspt-cli --features benchmark`
  - `cargo doc --locked --workspace --no-deps`
  - `./check-rules.sh check`
- Pull requests run the full credential-free gate once on Ubuntu. Merge-group,
  protected-branch push, and manual full runs add Windows and macOS tests.
  Avoid OS-specific assumptions.
- The Documentation workflow validates both Sphinx trees on relevant PRs and
  publishes one atomic Pages artifact from `master`.
- Clippy warnings are CI failures. Keep Rust test modules at the end of files, and prefer arrays/slices over unnecessary `vec![]` allocations in tests.

## PSP code check (enforced)
Perspt is held to the PSP code check rules. Three constraints are measured by `xtask`, and they apply to **Rust sources only** — `docs/` (PSPs, the Sphinx book) is out of scope.

| Rule | Limit | Measured as |
| --- | --- | --- |
| `PSP-1` file length | 1408 lines | every physical line, comments and blanks included |
| `PSP-2` function length | 70 lines | lines in the function that carry a token and are not comments; nested `fn` items are measured separately and subtracted, closures count toward their enclosing function |
| `PSP-3` line width | 108 columns | Unicode scalar values, not bytes |

`PSP-2` caps functions at 70 code lines, following the TigerBeetle-style guidance Perspt adopted. rustfmt stays at its default width of 100, so `PSP-3` in practice only catches long string literals and trailing comments rustfmt cannot break.

```bash
./check-rules.sh check                    # PR gate; fails on any new violation
./check-rules.sh report                   # every offending file, function, and line
./check-rules.sh report --rule PSP-2     # one rule
./check-rules.sh report --format json     # machine-readable
./check-rules.sh baseline --shrink        # ratchet accepted debt downward
```

`.cargo/config.toml` is gitignored, so `cargo xtask` is not available from a fresh clone — `./check-rules.sh` is the portable entry point and the one CI uses. Add `[alias] xtask = "run --quiet --package xtask --"` to your own `.cargo/config.toml` if you want the shorter form.

`.psp-baseline.toml` records the debt that existed when the rules were adopted. **Counts may only shrink.** `check` fails when a count grows or an untracked file appears; fix that by decomposing the file or shortening the function, never by raising the baseline. When a file improves, `check` says so and `baseline --shrink` records it. The target state is an empty baseline.

`PSP-2` is measured with `syn` rather than by counting braces, because braces inside string literals (`"\\mathbf{"` in `perspt-tui`) make text-based counting wrong by two orders of magnitude. If you extend the tool, keep it parsing.

## DuckDB build: bundled vs system
- DuckDB is pinned in the workspace root `Cargo.toml` (`duckdb = "=1.10505.0"`) **without** the `bundled` feature by default.
- Each crate that (transitively) depends on DuckDB exposes a `bundled` cargo feature that activates `duckdb/bundled` through `perspt-store/bundled`. The chain: `perspt-store → perspt-agent, perspt-tui, perspt-dashboard, perspt-cli, perspt`.
- **Local dev** (default features): links against a system-installed DuckDB library. On macOS with Homebrew: `brew install duckdb`. The `.cargo/config.toml` sets `DUCKDB_LIB_DIR` and `DUCKDB_INCLUDE_DIR` to Homebrew paths (override via env vars if needed).
- **CI** downloads and checksum-verifies the official DuckDB 1.5.5 shared
  library. **Release** builds use `--features bundled` for self-contained
  binaries.
- For fast iteration use `cargo clippy --all-targets`, `cargo test`, or
  `cargo test -p <crate>`. Do not use `--all-features` for routine validation;
  it couples the expensive release-only native build to the optional benchmark.

## Docs and local workflows
- Rust API docs: `cargo doc --open --no-deps`.
- Sphinx book: `cd docs/perspt_book && uv run make html`.
- VS Code tasks already expose doc generation, PDF build, and link validation; prefer those tasks when working on documentation.

## Editing tips
- Keep provider/config changes centralized in `perspt-core`; avoid duplicating env/config logic in CLI or TUI crates.
- Respect the streaming/EOT contract and avoid blocking TUI event loops.
- When changing manifests, features, or workspace-level dependencies, run the
  workspace gate and compile `perspt-cli` with `--features benchmark`.
- Avoid editing generated logs, `target/`, conversation transcripts, or scratch sandbox data unless the task explicitly targets them.

Questions or mismatches in these instructions should be resolved in favor of the checked-in workspace layout and `.github/workflows/ci.yml`.
