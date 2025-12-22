# PSP 4 Implementation Plan

Tracks step-by-step implementation of [PSP-000004](docs/psps/source/psp-000004.rst).

## Phase 1: Workspace Infrastructure ✅

- [x] Create branch `feat/psp-4-implementation`
- [x] Initialize Cargo Workspace
- [x] Create crates: `perspt-core`, `perspt-tui`, `perspt-agent`, `perspt-policy`, `perspt-sandbox`, `perspt-cli`
- [x] Migrate: `config.rs`, `llm_provider.rs` -> `perspt-core`
- [x] Migrate: TUI stubs -> `perspt-tui`

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
- [x] `StabilityMonitor`, `ModelTier`, `AgentContext`, `AgentMessage`

### Agents + LLM Integration ✅

- [x] `Agent` trait with `process()`, `name()`, `can_handle()`
- [x] `ArchitectAgent` - planning with structured prompts
- [x] `ActuatorAgent` - code generation with contract enforcement
- [x] `VerifierAgent` - stability verification
- [x] `SpeculatorAgent` - fast lookahead
- [x] GenAIProvider injected to all agents

### Agent Tooling ✅

- [x] `read_file`, `write_file`, `list_files`
- [x] `search_code` (ripgrep/grep)
- [x] `apply_patch`, `run_command`

### Merkle Ledger ✅

- [x] `MerkleLedger` with session/commit management
- [x] `start_session()`, `commit_node()`, `end_session()`

### LSP Client ✅

- [x] JSON-RPC Client (stdio)
- [x] `textDocument/publishDiagnostics` ($V_{syn}$)
- [x] `calculate_syntactic_energy()`

### Control Loop ✅

- [x] 7-Step SRBN Loop
- [x] Lyapunov Energy: $V(x) = αV_{syn} + βV_{str} + γV_{log}$

## Phase 5: Agent TUI (`perspt-tui`) ✅

- [x] Dashboard: Progress, Energy sparkline, Logs
- [x] Diff Viewer: Syntax highlighting, scrollbar
- [x] Task Tree: DAG visualization with status icons
- [x] Review Modal: Approve/Reject/Request Changes
- [x] Agent App: Tab navigation, keyboard shortcuts

## Phase 6: CLI Integration (`perspt-cli`) ✅

### Subcommands ✅

- [x] `chat`, `agent`, `init`, `config`, `ledger`, `status`, `abort`, `resume`

### Flags ✅

- [x] `--yes`, `--auto-approve-safe`, `--energy-weights`
- [x] `--stability-threshold`, `--max-cost`, `--max-steps`, `-k`, `-m`

## Phase 7: Documentation & Integration

- [ ] Update `perspt_book`
- [ ] Integration tests
- [ ] Wire legacy TUI

---

## Completion Summary

| Phase | Status | Notes |
|-------|--------|-------|
| 1. Workspace | ✅ 100% | 6 crates created |
| 2. Core | ✅ 100% | GenAIProvider thread-safe |
| 3. Security | ✅ 100% | Starlark + sanitization |
| 4. SRBN Engine | ✅ 95% | LLM integrated, all agents |
| 5. Agent TUI | ✅ 100% | Dashboard, diffs, tree |
| 6. CLI | ✅ 100% | 8 subcommands, all flags |
| 7. Documentation | ⏳ 0% | Deferred |

**Total: ~95% Complete**
