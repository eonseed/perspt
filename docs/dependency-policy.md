# Dependency policy and review matrix

Perspt 0.6.6 builds with Rust 1.97.1 and edition 2021. Release dependencies
must be stable crates.io releases: no beta, release-candidate, Git, path-to-an
external-checkout, or wildcard requirements. Ordinary crates use compatible
semver requirements plus the committed `Cargo.lock`; DuckDB is exact-pinned
because its Rust package and native ABI must move together.

The matrix below records the direct dependencies that define the release
boundary. “Locked” is the reviewed lockfile version at the 0.6.6 baseline.
The workspace manifests and `cargo deny check` are authoritative for the full
graph.

| Dependency | Requirement | Locked / latest reviewed stable | MSRV | License | Important features | Pin rationale |
|---|---:|---:|---:|---|---|---|
| Rust | `1.97.1` | `1.97.1` | — | Apache-2.0/MIT | clippy, rustfmt | Reproducible release toolchain |
| duckdb / libduckdb-sys | `=1.10505.0` | `1.10505.0` | 1.84 | MIT | optional `bundled` | Native DuckDB 1.5.5 ABI must match exactly |
| genai | `0.6.5` | `0.6.5` | 1.80 | MIT | provider adapters | Retain stable line; exclude 0.7 beta |
| srbn, srbn-serde, srbn-ledger | `0.3.0` | `0.3.0` | 1.85 | LGPL-3.0-only | kernel, serde, ledger | Current published Paper I–III kernel family |
| tokio | `1.52` | `1.52.3` | 1.71 | MIT | full | Shared async/process runtime |
| reqwest | `0.13` | `0.13.4` | 1.64 | Apache-2.0/MIT | json, rustls | MCP Streamable HTTP; redirects disabled in code |
| serde / serde_json | `1.0` | `1.0.228` / `1.0.150` | 1.56 | Apache-2.0/MIT | derive | Durable event and protocol formats |
| dirs | `6.0` | `6.0.0` | 1.71 | Apache-2.0/MIT | — | One direct major across the workspace |
| ratatui | `0.30.2` | `0.30.2` | 1.85 | MIT | crossterm | TUI baseline |
| clap | `4.6` | `4.6.1` | 1.85 | Apache-2.0/MIT | derive | CLI surface |
| rustls | `0.23` | `0.23.41` | 1.71 | Apache-2.0/ISC/MIT | ring | Explicit process crypto provider |
| starlark | `0.14` | `0.14.2` | 1.77 | Apache-2.0/MIT | — | Bounded tool-program policy |

## Review rules

- Run `cargo update` in reviewable cohorts: database; async/network;
  CLI/TUI/dashboard; cryptography; remaining patches.
- Run `cargo deny check`, strict workspace Clippy, all targets, and mechanism
  tests after every cohort.
- A release may stay below the newest stable version only when this document
  names the upstream issue, newest passing stable version, and review date.
- Advisory exceptions require an owner, justification, and expiry date in
  `deny.toml`; there are no standing advisory exceptions.
- Dynamic DuckDB builds must report an incompatible runtime ABI before schema
  access. Bundled and dynamic builds are both acceptance configurations.

Last reviewed: 2026-08-17.

## Time-bounded transitive exceptions

| Dependency / advisory | Upstream path and rationale | Owner | Review by |
|---|---|---|---|
| `spin 0.9.8` (yanked) | `starlark 0.14.2 → pagable → postcard → heapless`; every compatible `spin 0.9.2–0.9.8` release is yanked and the dependency cannot select 0.10. Recheck the next stable Starlark release. | Perspt maintainers | 2026-11-17 |
| `derivative 2.2.0` / RUSTSEC-2024-0388 | Unmaintained transitive of the current stable `starlark 0.14.2`; no safe compatible upgrade is published. | Perspt maintainers | 2026-11-17 |
| `fxhash 0.2.1` / RUSTSEC-2025-0057 | Unmaintained transitive of `starlark_map 0.14.2`; no safe compatible upgrade is published. | Perspt maintainers | 2026-11-17 |
| `paste 1.0.15` / RUSTSEC-2024-0436 | Unmaintained transitive of pinned `genai 0.6.5` and stable `starlark 0.14.2`; no safe compatible upgrade is published. | Perspt maintainers | 2026-11-17 |

Transitive unmaintained notices remain visible but fail the gate only if they
enter a workspace package directly. Vulnerabilities and unsoundness continue
to fail at every depth. The TUI disables `tui-markdown`'s optional Syntect
highlighter because its current graph carried two remediated-only-in-0.41
`quick-xml` vulnerabilities through a `plist` constraint capped at 0.39.
