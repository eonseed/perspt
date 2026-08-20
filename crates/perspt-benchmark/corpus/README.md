# PSP-10 coding evaluation corpus

This directory is a matched, content-addressed release corpus for Perspt's
optional live evaluator. It is separate from runtime conformance tests and is
never sent to model routes by CI.

Each task contains:

- `task.json`: the user goal, shell-free hidden-check argv, expectation, one
  language tag (`rust`, `python`, or `mixed`), one scale tag (`small`,
  `medium`, or `large`), and capability tags;
- `fixture/`: the only tree copied into the coding agent's workspace;
- `hidden/`: an oracle overlay copied only after the agent finishes; and
- `solution/`: a withheld reference overlay used only by corpus validation.

Rust fixture manifests are stored as `Cargo.toml.fixture` so Cargo includes
nested fixture packages in a published crate archive. The runner restores the
ordinary `Cargo.toml` name inside its temporary materialization before any
validation or live cell runs.

Build or install the CLI with the optional `benchmark` feature. Then run the
credential-free authoring gate with:

```console
cargo run -p perspt-cli --features benchmark -- benchmark validate
```

The gate refuses fewer than 30 tasks, an imbalanced language/scale mix,
insufficient capability coverage, a fixture that already passes its oracle,
or a reference solution that does not pass. Its digest binds every semantic
task record and every byte under `fixture/`, `hidden/`, and `solution/`.

Live runs use production role resolution from the normal Perspt configuration
and record actuator, architect, speculator, verifier, and adjudicator entries
where configured. Coding verification remains deterministic, so the verifier
entry is provenance rather than a model call. There are no model-name or
family-label benchmark arguments. A quick run uses one production-topology arm
and eight tasks by default:

```console
cargo run -p perspt-cli --features benchmark -- \
  --config config.local.toml benchmark run --output smoke.json
```

Use `--suite adaptive` for the paging/adaptive pair, or `--suite full` for the
complete seven-arm diagnostic ladder. `--tasks` can explicitly select a
smaller prefix; acceptance evidence still requires the declared full corpus.

```console
cargo run -p perspt-cli --features benchmark -- \
  --config config.local.toml benchmark run --suite full --tasks 30 \
  --output report.json
```

Do not infer platform acceptance from one topology. Aggregate at least two
accepted full reports whose configured actuator routes belong to distinct
model families:

```console
cargo run -p perspt-cli --features benchmark -- benchmark aggregate \
  report-family-a.json report-family-b.json
```
