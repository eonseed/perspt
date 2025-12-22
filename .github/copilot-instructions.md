# Perspt: AI Assistant Coding Guide

Concise, repo-specific guidance for making high-quality changes fast.

## Big picture
- Rust terminal chat app with two modes: rich TUI (`src/ui.rs`) and simple CLI (`src/cli.rs`).
- Core modules: `src/main.rs` (args, config, provider init, TUI run, panic safety), `src/config.rs` (JSON config + env auto-detect), `src/llm_provider.rs` (genai-based adapter), `src/ui.rs` (Ratatui chat UI), `src/cli.rs` (streaming CLI).

## Streaming contract (critical)
- Requests stream chunks over `mpsc::UnboundedSender<String>`; end with a single `EOT_SIGNAL` (`<<EOT>>`).
- Provider sends EOT (UI never adds its own). UI batches channel messages, handles first EOT, ignores duplicates.
- UI holds `streaming_buffer`, updates the last assistant message live; pending inputs queue until EOT.

## Provider integration (genai)
- `GenAIProvider::generate_response_stream_to_channel(model, prompt, tx)` streams content then sends EOT.
- Model listing/validation: `get_available_models(provider)`, `validate_model(model, provider_type)`; `perspt --list-models` exits early.
- Provider→env mapping set in `new_with_config()`; adapter mapping in `str_to_adapter_kind()`:
  `openai→OPENAI_API_KEY`, `anthropic→ANTHROPIC_API_KEY`, `gemini→GEMINI_API_KEY`, `groq→GROQ_API_KEY`, `cohere→COHERE_API_KEY`, `xai→XAI_API_KEY`, `deepseek→DEEPSEEK_API_KEY`, `ollama` (no key).

## Config patterns
- `AppConfig` keys: `providers`, `api_key`, `default_model`, `default_provider`, `provider_type`.
- `process_loaded_config()` infers `provider_type` from `default_provider`; unknowns default to `openai`.
- `load_config(None)` seeds endpoints for all providers; auto-detects via env when possible.
- CLI flags: `--config`, `--api-key`, `--provider-type`, `--provider`, `--model`, `--list-models`, `--simple-cli`, `--log-file`.

## UI conventions
- Markdown to lines via `markdown_to_lines()`; keep new content types flowing through it for wrapping.
- Scroll math uses `.chars().count()`; keep `max_scroll()` and `update_scroll_state()` logic aligned.
- Commands: only `/save` (writes plain text transcript). Easter egg on exact `l-o-v-e`.
- `start_streaming()` inserts placeholder assistant; `finish_streaming()` flushes buffer, resets flags, scrolls bottom.

## Errors, panics, logs
- Errors categorized via `ErrorType` and shown in chat/status.
- `main` installs a panic hook that restores terminal (raw mode off, leave alt screen) and prints guidance.
- Logging uses `env_logger`; `main` sets `LevelFilter::Error` to avoid TUI noise. Avoid `println!` in UI paths.

## Dev workflows
- Build/lint/test: `cargo build`, `cargo test`, `cargo clippy -- -D warnings`, `cargo fmt -- --check`.
- Run TUI: `cargo run -- [args]`; simple CLI: `cargo run -- --simple-cli [--log-file session.txt]`.
- Docs: `cd docs/perspt_book && uv run make html` (see repo tasks like `validate-docs.sh`).

## Editing tips
- Respect streaming/EOT rules; don’t block the UI select loop—spawn work on tokio tasks, send via the channel.
- Adding providers/models? Update `str_to_adapter_kind()`, env var mapping in `new_with_config()`, and default model fallbacks in `src/main.rs`.
- Reference: `src/main.rs`, `src/llm_provider.rs`, `src/ui.rs`, `src/cli.rs`, `src/config.rs`.

Questions or mismatches (e.g., EOT handling, scroll math, config inference)? Call them out and we’ll align the guide.