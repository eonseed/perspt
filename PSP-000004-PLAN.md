# PSP 4 Implementation Plan

Tracks step-by-step implementation of [PSP-000004](docs/psps/source/psp-000004.rst).

## Phase 1: Workspace Infrastructure ✅

- [x] Create branch `feat/psp-4-implementation`
- [x] Initialize Cargo Workspace
- [x] Create crates: `perspt-core`, `perspt-tui`, `perspt-agent`, `perspt-policy`, `perspt-sandbox`, `perspt-cli`
- [x] Migrate: `config.rs`, `llm_provider.rs` -> `perspt-core`
- [x] Migrate: TUI stubs -> `perspt-tui`
- [ ] **Wire Legacy TUI**: Connect actual TUI to new workspace

## Phase 2: Core Foundation (`perspt-core`) ✅

- [x] Refactor `GenAIProvider` for thread-safety (`Arc<RwLock<...>>`)
- [x] Implement `PERSPT.md` parser (Project Memory)
- [x] Add deps: `petgraph`, `lsp-types`, `futures`

## Phase 3: Security (`perspt-policy` & `perspt-sandbox`) ✅

- [x] `perspt-policy`: Starlark engine (`~/.perspt/rules`)
- [x] `perspt-policy`: Command Sanitization (`shell-words`)
- [x] `perspt-sandbox`: `SandboxedCommand` trait

## Phase 4: The SRBN Engine (`perspt-agent`) ✅

### Types ✅

- [x] `SRBNNode`, `BehavioralContract`, `WeightedTest`
- [x] `StabilityMonitor` (energy_history, attempt_count, stable, epsilon)
- [x] `ModelTier`, `AgentContext`, `AgentMessage`
- [x] `Agent` trait with `process()`, `name()`, `can_handle()`

### Native LSP Client ✅

- [x] JSON-RPC Client (stdio) using `tokio::process`
- [x] `textDocument/publishDiagnostics` ($V_{syn}$)
- [x] `calculate_syntactic_energy()` function

### Agent Tooling ✅

- [x] `read_file`, `write_file`, `list_files`
- [x] `search_code` (ripgrep/grep)
- [x] `apply_patch`
- [x] `run_command`

### Merkle Ledger ✅

- [x] `MerkleLedger` with session/commit management
- [x] `start_session()`, `commit_node()`, `end_session()`
- [x] `rollback_to()`, `get_stats()`

### Control Loop ✅

- [x] 7-Step SRBN Loop
- [x] Lyapunov Energy: $V(x) = αV_{syn} + βV_{str} + γV_{log}$
- [x] Weights: α=1.0, β=0.5, γ=2.0; Threshold: ε=0.1

### Remaining Tasks

- [ ] **Complexity Gating**: Pause if depth>3 or width>5
- [ ] **Wire LLM**: Connect agents to GenAIProvider
- [ ] **LSP $V_{str}$**: Symbol analysis for structural energy

## Phase 5: Agent TUI (`perspt-tui`) ✅

- [x] Dashboard: Progress, Energy sparkline, Logs
- [x] Diff Viewer: Syntax highlighting, scrollbar
- [x] Task Tree: DAG visualization with status icons
- [x] Review Modal: Approve/Reject/Request Changes
- [x] Agent App: Tab navigation, keyboard shortcuts

### Polish (Future)

- [ ] `tree-sitter-highlight` for AST-based highlighting
- [ ] `tachyonfx` animations
- [ ] `ratatui-throbber` progress indicators

## Phase 6: CLI Integration (`perspt-cli`) ✅

### Subcommands ✅

- [x] `chat` (default), `agent`, `init`, `config`
- [x] `ledger`, `status`, `abort`, `resume`

### Flags ✅

- [x] `--yes` (auto-approve)
- [x] `--auto-approve-safe`
- [x] `--energy-weights α,β,γ`
- [x] `--stability-threshold ε`
- [x] `--max-cost`, `--max-steps`
- [x] `-k` (complexity threshold)
- [x] `-m` (execution mode: cautious/balanced/yolo)

### Remaining

- [ ] `file` subcommand (workflow.yaml)
- [ ] `serve` subcommand (wire protocol)

## Phase 7: Documentation & Verification

- [ ] Update `perspt_book`
- [ ] Integration tests
- [ ] Wire legacy TUI to `perspt-tui`

---

## Completion Status

| Phase | Status | Details |
|-------|--------|---------|
| 1. Workspace | ✅ 95% | Legacy TUI wiring pending |
| 2. Core | ✅ 100% | Complete |
| 3. Security | ✅ 100% | Complete |
| 4. SRBN Engine | ✅ 90% | LLM wiring, complexity gating pending |
| 5. Agent TUI | ✅ 100% | Polish items deferred |
| 6. CLI | ✅ 95% | file/serve subcommands pending |
| 7. Documentation | ⏳ 0% | Not started |
