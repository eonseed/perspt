<div align="center">

<img src="docs/shared/_static/perspt-logo.svg" alt="Perspt Logo" width="96">

# Perspt

**Simple CLI · Interactive Terminal UI · Compiler-Verified Coding Agent · Rust SDK**

[![Build Status](https://github.com/eonseed/perspt/actions/workflows/ci.yml/badge.svg)](https://github.com/eonseed/perspt/actions/workflows/ci.yml)
[![Documentation](https://github.com/eonseed/perspt/actions/workflows/docs.yml/badge.svg)](https://eonseed.github.io/perspt/)
[![Version](https://img.shields.io/badge/version-0.6.6%20(H%C3%B3zh%C3%B3)-059669)](https://github.com/eonseed/perspt/releases)
[![License: LGPL-3.0](https://img.shields.io/badge/license-LGPL--3.0-blue.svg)](LICENSE)

[**Website & Docs**](https://eonseed.github.io/perspt/) · [**The Perspt Book**](https://eonseed.github.io/perspt/book/index.html) · [**PDF Handbook**](https://eonseed.github.io/perspt/books/perspt-book-en.pdf) · [**Rust SDK Docs**](https://eonseed.github.io/perspt/perspt/index.html) · [**PSP Archive**](https://eonseed.github.io/perspt/psps/index.html)

[![Perspt in Action](docs/screencast/perspt_terminal_ui.jpg)](https://github.com/user-attachments/assets/f80f7109-1615-487b-b2a8-b76e16ebf6a7)

</div>

---

**Perspt** is a fast, single-binary Rust tool and embeddable SDK for working with Large Language Models in your terminal. Built for everyday developer productivity, Perspt gives you **three ways to use AI in the terminal** plus a **Rust SDK** for custom integration:

1. **Simple CLI (`perspt simple-chat`)** — Plain terminal output with zero UI chrome. Pipe logs, files, or `git diff` straight from `stdin` and redirect clean text output into shell scripts.
2. **Terminal UI (`perspt` / `perspt chat`)** — A full-screen interactive terminal chat interface with streaming Markdown, syntax highlighting, tables, LaTeX math, read-only MCP tools, and instant switching across 9+ LLM providers.
3. **SRBN Coding Agent (`perspt agent`)** — An autonomous coding agent that plans tasks, edits files in an isolated sandbox, and runs your real compiler, LSP, and test suite to fix errors automatically before committing changes.
4. **Rust SDK (`perspt-sdk`, `perspt-coding`, `perspt-research`)** — Modular Rust crates that let you embed Perspt's verification loop, capability sandbox, and DuckDB audit ledger into your own applications.

> **Why is the coding agent called SRBN?**
> Internally, `perspt agent` is powered by **SRBN (Stabilized Recursive Barrier Networks)**, based on the *Stability is All You Need* research paper series. Standard LLM agents guess in an open loop and drift when errors accumulate across files. SRBN wraps the model in a deterministic compiler, LSP, and test feedback loop—scoring remaining errors and rolling back bad edits—so the code it generates actually compiles, runs, and stays close to the task you gave it.

## Features

- **NO-CHROME SIMPLE CLI**: Stream plain text to `stdout` for Unix pipes, shell scripts, and quick terminal Q&A (`perspt simple-chat`).
- **RICH TERMINAL UI**: Full-screen `ratatui` interface with Markdown, code blocks, math rendering, multiline paste, and `/mcp` inspection (`perspt chat`).
- **VERIFIED CODING AGENT**: Edits code in a safe candidate overlay and verifies every step with real compilers, LSPs (`rust-analyzer`, `pyright`, `tsc`, `gopls`), and test suites (`cargo test`, `pytest`).
- **9+ LLM PROVIDERS**: Works with OpenAI, Anthropic, Google Gemini, Vertex AI, Groq, Cohere, xAI, DeepSeek, local Ollama, and any OpenAI-compatible server.
- **PER-ROLE MODEL ROUTING**: Assign different models for read-only repo exploration (`--explorer-model`), coding (`--actuator-model`), and diff review (`--adjudicator-model`).
- **SAFE BY DEFAULT**: Starlark security policies, workspace path confinement, and OS process sandboxing (Bubblewrap on Linux, `sandbox-exec` on macOS).
- **FULL AUDIT & ROLLBACK**: Local DuckDB ledger records every session for live browser monitoring (`--dashboard`), offline replay (`perspt replay`), crash resume (`perspt resume`), and one-command undo (`perspt ledger --rollback`).
- **EMBEDDABLE RUST SDK**: 14 modular workspace crates with zero Python runtime dependencies.

## Quick Start

Build Perspt from source (requires Rust 1.97.1+):

```bash
git clone https://github.com/eonseed/perspt.git
cd perspt
cargo build --release --features bundled

export GEMINI_API_KEY="your-api-key"   # Or OPENAI_API_KEY, ANTHROPIC_API_KEY, or local Ollama
```

Try all three modes:

```bash
# 1. Simple CLI — plain output for Unix pipes and scripts
git diff | ./target/release/perspt simple-chat "Write a concise commit message"

# 2. Terminal UI — interactive full-screen chat
./target/release/perspt chat

# 3. SRBN Coding Agent — autonomous multi-file coding with compiler & test verification
./target/release/perspt agent -w ./my-project "Fix the failing parser tests"

# Headless agent run + live browser dashboard on http://127.0.0.1:3000
./target/release/perspt agent --yes --dashboard -w ./my-project "Add input validation and unit tests"
```

> **Note on Windows**: `chat` and `simple-chat` work natively on Linux, macOS, and Windows. `perspt agent` uses Bubblewrap on Linux and `sandbox-exec` on macOS for OS process isolation; on native Windows, pass `--allow-unisolated` or run inside WSL2.

## Embed with the Perspt SDK

You can integrate Perspt's verification engine and domain packages directly into your own Rust applications:

```toml
[dependencies]
perspt-sdk = { git = "https://github.com/eonseed/perspt" }
perspt-coding = { git = "https://github.com/eonseed/perspt" }
```

```rust
use perspt_coding::CodingDomain;
use perspt_sdk::{
    domain::{AgentDomainPackage, DomainScope},
    energy::score_candidate,
    gate::evaluate_gate,
    residual::ResidualEvent,
};

fn verify_candidate(scope: &DomainScope, residuals: &[ResidualEvent], best_v: f64) -> perspt_sdk::error::Result<bool> {
    let domain = CodingDomain;
    let model = domain.energy_model(scope);
    let score = score_candidate(residuals, &model)?;
    let decision = evaluate_gate(score.hard_violations.is_empty(), score.total, best_v, model.rho_gate)?;
    Ok(decision.is_accepted())
}
```

## Supported LLM Providers

Set any one provider credential to start chatting, or configure a multi-model portfolio in `config.toml`:

| Provider | Environment Variable / Auth | Example Model ID |
|---|---|---|
| **Google Gemini** | `GEMINI_API_KEY` | `gemini::gemini-3.5-flash` |
| **OpenAI / Compatible** | `OPENAI_API_KEY` (`OPENAI_BASE_URL`) | `openai::gpt-4o` |
| **Anthropic** | `ANTHROPIC_API_KEY` | `anthropic::claude-3-5-sonnet-20241022` |
| **Google Vertex AI** | Google Cloud ADC (`gcloud auth`) | `vertex::gemini-3.5-flash-lite` |
| **Ollama (Local)** | None (local server) | `ollama::qwen2.5-coder` |
| **Groq** | `GROQ_API_KEY` | `groq::llama-3.1-8b-instant` |
| **DeepSeek** | `DEEPSEEK_API_KEY` | `deepseek::deepseek-chat` |
| **xAI** | `XAI_API_KEY` | `xai::grok-beta` |
| **Cohere** | `COHERE_API_KEY` | `cohere::command-r-plus` |

## CLI Commands

| Command | Description |
|---|---|
| `perspt chat` | Interactive full-screen Terminal UI chat (default when running `perspt`) |
| `perspt simple-chat` | Plain terminal output with no UI chrome; ideal for pipes and scripts |
| `perspt agent <TASK>` | Autonomous SRBN coding agent with compiler, LSP, and test verification |
| `perspt dashboard` | Real-time web dashboard for monitoring sessions, diffs, and energy |
| `perspt status` | Show active session progress and verification status |
| `perspt ledger` | Inspect session history or undo the newest promotion (`--rollback <ID>`) |
| `perspt resume <ID>` | Resume an interrupted agent session from the DuckDB ledger |
| `perspt replay <ID>` | Deterministically replay and audit a session offline without API keys |
| `perspt providers` | Show configured providers (`--probe` tests live connectivity) |
| `perspt config` | View (`--show`) or edit (`--edit`) configuration |
| `perspt init` | Initialize project memory and Starlark security policy rules |

Run `perspt <COMMAND> --help` for full options.

## Workspace Crates

| Crate | Role |
|---|---|
| `perspt-sdk` | Domain-neutral SRBN control plane: residuals, energy, gates, capabilities, replay |
| `perspt-coding` | Coding domain package: Rust/Python/TypeScript verifiers, diagnostics, and adapters |
| `perspt-research` | Research domain package built on `perspt-sdk` |
| `perspt-agent` | Governed candidate runtime, tool loop, LSP client, and work-graph scheduler |
| `perspt-core` | Shared configuration, provider portfolio, events, and prompt libraries |
| `perspt-tui` | Ratatui terminal UI for interactive chat and agent review |
| `perspt-cli` | Command-line binary entry point (`perspt`) |
| `perspt-store` | DuckDB session ledger, content-addressed artifacts, and calibration store |
| `perspt-policy` | Starlark security policy engine and command guards |
| `perspt-sandbox` | OS process isolation (Bubblewrap / `sandbox-exec`) |
| `perspt-dashboard` | Axum + HTMX real-time web observability dashboard |
| `perspt-prompt-macros` | Compile-time code generator for typed prompt section libraries |
| `perspt-benchmark` | Optional manual evaluation suite (`--features benchmark`) |
| `perspt` | Top-level meta-crate |

## Building & Testing

```bash
# Format, lint, test, and check code rules
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace --all-targets -- --test-threads=1
./check-rules.sh check

# Build unified documentation site & PDF book (requires guidedog and typst)
./generate-docs.sh
```

## Documentation & License

- **Unified Documentation Site**: [https://eonseed.github.io/perspt/](https://eonseed.github.io/perspt/)
- **The Perspt Book (HTML & PDF)**: [Book Online](https://eonseed.github.io/perspt/book/index.html) · [Book PDF](https://eonseed.github.io/perspt/books/perspt-book-en.pdf)
- **Specification Proposals (PSPs)**: [PSP Catalog](https://eonseed.github.io/perspt/psps/index.html)
- **Contributing**: See [CONTRIBUTING](docs/perspt_book/developer-guide/contributing.rst)
- **License**: Perspt is licensed under the [GNU Lesser General Public License v3.0 (LGPL-3.0)](LICENSE).
