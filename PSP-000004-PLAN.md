# PSP 4 Implementation Plan

Tracks step-by-step implementation of [PSP-000004](docs/psps/source/psp-000004.rst).

## Phase 1: Workspace Infrastructure
- [ ] Create branch `feat/psp-4-implementation` (Done)
- [ ] Initialize Cargo Workspace
- [ ] Create crates: `perspt-core`, `perspt-tui`, `perspt-agent`, `perspt-policy`, `perspt-sandbox`, `perspt-cli`
- [ ] Migrate: `config.rs`, `llm_provider.rs` -> `perspt-core`
- [ ] Migrate: `ui.rs`, `main.rs` (TUI loop) -> `perspt-tui`
- [ ] **Verify**: `cargo run --bin perspt` launches Chat TUI

## Phase 2: Core Foundation (`perspt-core`)
- [ ] Refactor `GenAIProvider` for thread-safety (`Arc<RwLock<...>>`)
- [ ] Implement `PERSPT.md` parser (Project Memory)
- [ ] Add deps: `petgraph`, `duckdb`, `lsp-types`

## Phase 3: Security (`perspt-policy` & `perspt-sandbox`)
- [ ] `perspt-policy`: Starlark engine (`~/.perspt/rules`)
- [ ] `perspt-policy`: Command Sanitization (`shell-words`)
- [ ] `perspt-sandbox`: `SandboxedCommand` trait

## Phase 4: The SRBN Engine (`perspt-agent`)
**Types:**
- [ ] `SRBNNode` (node_id, goal, context_files, output_targets, contract, tier, monitor)
- [ ] `BehavioralContract` (interface_signature, invariants, forbidden_patterns, weighted_tests, energy_weights)
- [ ] `WeightedTest` (test_name, criticality: Critical/High/Low)
- [ ] `StabilityMonitor` (energy_history, attempt_count, stable, stability_epsilon=0.1)
- [ ] `ModelTier` (Architect, Actuator, Verifier, Speculator)
- [ ] `AgentContext` (working_dir, history, merkle_root, complexity_K)
- [ ] `AgentMessage` (role, content, timestamp)
- [ ] `Agent` trait (`async fn process(&self, ctx: &AgentContext) -> Result<AgentMessage>`)

**Native LSP Client (Sensor):**
- [ ] Implement JSON-RPC Client (stdio) using `tokio::process`
- [ ] Handle `textDocument/publishDiagnostics` ($V_{syn}$)
- [ ] Handle `textDocument/documentSymbol` ($V_{str}$)
- [ ] Implement Stability Hazard: Block if LSP offline

**Stability Logic:**
- [ ] Energy: $V(x) = αV_{syn} + βV_{str} + γV_{log}$
- [ ] Default Weights: α=1.0, β=0.5, γ=2.0
- [ ] Default Threshold: ε=0.1

**Control Loop:**
- [ ] 7-Step SRBN Loop (Sheafify -> Execute -> Speculate -> Verify -> Converge -> Sheaf Validate -> Commit)
- [ ] Complexity Gating: Pause if depth>3 or width>5 (configurable via K)

**State Machine:**
- [ ] States: TASK_QUEUED, PLANNING, CODING, VERIFYING, RETRY, SHEAF_CHK, COMMITTING, ESCALATED, COMPLETED, FAILED, ABORTED

**Retry/Escalation:**
- [ ] Compilation: 3 retries
- [ ] Tool Use: 5 retries
- [ ] Reviewer Rejection: 3 retries -> escalate

**DuckDB:**
- [ ] Merkle Ledger Schema
- [ ] Session History Table

**Execution Modes:**
- [ ] Solo, Team, Manifest (YAML)

**Agent Tooling:**
- [ ] `read_file`, `search_code`, `apply_patch`, `run_command`

## Phase 5: Agent TUI (`perspt-tui`)
- [ ] Dashboard: Plan View, Token Cost, Active Agent
- [ ] Diffs: `tree-sitter-highlight` + `similar`
- [ ] Review Mode: `[y]`, `[n]`, `[e]`, `[d]`
- [ ] UX: `tachyonfx`, `tui-textarea`, `ratatui-throbber`
- [ ] Accessibility: Keyboard nav, high-contrast

## Phase 6: CLI Integration (`perspt-cli`)
**Subcommands:**
- [ ] `run`, `file`, `list`, `attach`, `status`, `stop`, `resume`, `serve`

**Flags:**
- [ ] `--auto-approve`, `--auto-approve-safe`
- [ ] `--energy-weights α,β,γ`, `--stability-threshold ε`
- [ ] `--max-cost`, `--max-steps`

## Phase 7: Documentation & Verification
- [ ] Update `perspt_book`
- [ ] Integration tests
