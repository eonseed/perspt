#import "style.typ": *

#show: diagram-page.with(width: 740pt, margin: 16pt)

#section-banner(
  "Modular Rust Workspace Architecture",
  "Fourteen-Crate Layered Architecture and SDK Boundaries",
  subtitle: [Strict acyclic crate layers separating user interfaces (`cli`, `tui`, `dashboard`), closed-loop orchestration (`agent`, `benchmark`), reusable SDK & domain crates (`sdk`, `coding`, `research`), and core runtime (`core`, `policy`, `sandbox`, `store`).],
)

#v(8pt)

#let crate-pill(name, desc, color, bg) = block(
  width: 100%,
  fill: bg,
  stroke: (left: 2.6pt + color, rest: 0.7pt + rule),
  radius: 3pt,
  inset: (x: 7pt, y: 5.5pt),
  [
    #text(font: ("DejaVu Sans Mono", "Liberation Mono"), size: 7.8pt, weight: "bold", fill: color)[#name]
    #v(1.5pt)
    #text(size: 7.1pt, fill: ink-Mid)[#desc]
  ],
)

#stack(
  spacing: 8pt,
  // Layer 1: Operator & UI Surfaces
  block(
    width: 100%,
    fill: panel,
    stroke: 0.8pt + rule,
    radius: 4pt,
    inset: 8pt,
    [
      #text(size: 7.2pt, weight: "black", tracking: 0.12em, fill: teal)[LAYER 1 · OPERATOR INTERFACES & UMBRELLA FACADE]
      #v(5pt)
      #grid(
        columns: (1fr, 1fr, 1fr, 1fr),
        gutter: 7pt,
        crate-pill("perspt-cli", "Clap CLI binary (`perspt`), subcommands, auth & ledger ops", teal-dark, teal-soft),
        crate-pill("perspt-tui", "Ratatui chat & agent TUI, diff review modal, LaTeX math", teal-dark, teal-soft),
        crate-pill("perspt-dashboard", "Axum + Askama + HTMX live telemetry & governance web UI", teal-dark, teal-soft),
        crate-pill("perspt", "Umbrella library crate re-exporting public workspace APIs", teal-dark, teal-soft),
      )
    ],
  ),

  // Layer 2: Closed-Loop Orchestration & Evaluation
  block(
    width: 100%,
    fill: panel,
    stroke: 0.8pt + rule,
    radius: 4pt,
    inset: 8pt,
    [
      #text(size: 7.2pt, weight: "black", tracking: 0.12em, fill: violet)[LAYER 2 · CLOSED-LOOP SRBN ORCHESTRATION & BENCHMARK HARNESS]
      #v(5pt)
      #grid(
        columns: (2fr, 1fr),
        gutter: 7pt,
        crate-pill(
          "perspt-agent",
          "Governed tool loop, `CandidateWorkspace` overlay, `SearchForest`, multi-node `StagingRoot` dispatcher, LSP & verifier runner, MCP `ExternalToolRuntime`, crash resume",
          violet-dark,
          violet-soft,
        ),
        crate-pill(
          "perspt-benchmark",
          "Feature-gated 30-task evaluation corpus (Rust, Python, mixed) with offline hidden oracles",
          violet-dark,
          violet-soft,
        ),
      )
    ],
  ),

  // Layer 3: Domain Packages & Mathematical Control Plane
  block(
    width: 100%,
    fill: panel,
    stroke: 0.8pt + rule,
    radius: 4pt,
    inset: 8pt,
    [
      #text(size: 7.2pt, weight: "black", tracking: 0.12em, fill: emerald)[LAYER 3 · MATHEMATICAL CONTROL PLANE & DOMAIN ADAPTERS]
      #v(5pt)
      #grid(
        columns: (1.4fr, 1.3fr, 1fr),
        gutter: 7pt,
        crate-pill(
          "perspt-sdk (+ srbn 0.3)",
          "Domain-neutral SRBN kernel: quadratic energy V(x), `AcceptGate`, `WorkGraph`, Ed25519 capabilities, spectral & conformal diagnostics, Merkle hash chain",
          emerald-dark,
          emerald-soft,
        ),
        crate-pill(
          "perspt-coding",
          "Coding domain package: 18 residual classes, Rust/Python/TypeScript adapters, compiled coding prompt sections",
          emerald-dark,
          emerald-soft,
        ),
        crate-pill(
          "perspt-research",
          "Second-domain proof package: citation verification, claim-graph residuals, domain generality check",
          emerald-dark,
          emerald-soft,
        ),
      )
    ],
  ),

  // Layer 4: Core Infrastructure, Governance & Persistence
  block(
    width: 100%,
    fill: panel,
    stroke: 0.8pt + rule,
    radius: 4pt,
    inset: 8pt,
    [
      #text(size: 7.2pt, weight: "black", tracking: 0.12em, fill: amber)[LAYER 4 · CORE RUNTIME, SECURITY BOUNDARIES & DURABLE STORAGE]
      #v(5pt)
      #grid(
        columns: (1.2fr, 1fr, 1fr, 1fr, 1fr),
        gutter: 6pt,
        crate-pill("perspt-core", "GenAI multi-provider router, model portfolio, config, language plugins", amber-dark, amber-soft),
        crate-pill("perspt-store", "DuckDB 2.0 Merkle ledger, artifacts, checkpoints, WAL repair", amber-dark, amber-soft),
        crate-pill("perspt-policy", "Starlark `.star` execution policy engine (`allow`/`prompt`/`forbidden`)", amber-dark, amber-soft),
        crate-pill("perspt-sandbox", "OS containment (`bwrap`, Seatbelt `sandbox-exec`, Windows JobObject)", amber-dark, amber-soft),
        crate-pill("perspt-prompt-macros", "Build-time compiler & validator for typed prompt section libraries", amber-dark, amber-soft),
      )
    ],
  ),
)
