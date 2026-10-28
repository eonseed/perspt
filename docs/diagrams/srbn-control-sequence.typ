#import "style.typ": *

#show: diagram-page.with(width: 740pt, margin: 16pt)

#section-banner(
  "Closed-Loop Execution Sequence",
  "Governed Tool Loop, Verifier Sensors, and Staged Workspace Promotion",
  subtitle: [Every mutation proposed by the Actuator is filtered by the Admissibility Kernel, executed inside an isolated `CandidateWorkspace`, measured by real language tools, and gated on error reduction before touching your repo.],
)

#v(8pt)

#let lanes = (
  ("CLI / Session", "Epoch & Charter", ink, panel),
  ("Architect & Graph", "WorkGraph DAG", violet, violet-soft),
  ("Actuator / Roles", "Compiled Prompts", teal, teal-soft),
  ("Kernel & Policy", "Capabilities + Starlark", amber, amber-soft),
  ("Candidate & Sensors", "Overlay + Sandbox", crimson, crimson-soft),
  ("Gate & Ledger", "AcceptGate + DuckDB", emerald, emerald-soft),
)

// Header boxes for sequence participants
#grid(
  columns: (1fr, 1fr, 1fr, 1fr, 1fr, 1fr),
  gutter: 6pt,
  ..lanes.map(l => block(
    width: 100%,
    fill: l.at(3),
    stroke: (top: 2.5pt + l.at(2), rest: 0.75pt + l.at(2)),
    radius: 3pt,
    inset: (x: 5pt, y: 5pt),
    align(center)[
      #text(size: 7.8pt, weight: "black", fill: l.at(2))[#l.at(0)]
      #v(1.5pt)
      #text(size: 6.5pt, fill: ink-Mid)[#l.at(1)]
    ],
  )),
)

#v(4pt)

#let step-row(num, from-col, to-col, label, detail, color, kind: "sync") = {
  block(
    width: 100%,
    inset: (y: 3.5pt, x: 6pt),
    fill: if calc.rem(num, 2) == 1 { panel } else { paper },
    stroke: (bottom: 0.5pt + rule-light),
    grid(
      columns: (22pt, 128pt, 1fr),
      gutter: 8pt,
      align: (center + horizon, left + horizon, left + horizon),
      box(
        width: 18pt,
        height: 18pt,
        radius: 9pt,
        fill: color,
        align(center + horizon, text(size: 7pt, weight: "black", fill: paper)[#num]),
      ),
      [
        #text(size: 7.4pt, weight: "bold", fill: color)[#from-col $->$ #to-col]
        #v(1pt)
        #text(size: 6.6pt, fill: muted)[#kind]
      ],
      [
        #text(size: 8.1pt, weight: "bold", fill: ink)[#label]
        #h(5pt)
        #text(size: 7.5pt, fill: ink-Mid)[— #detail]
      ],
    ),
  )
}

#block(
  width: 100%,
  stroke: 0.8pt + rule,
  radius: 4pt,
  clip: true,
  stack(
    spacing: 0pt,
    step-row(
      1,
      "CLI / Session",
      "Architect & Graph",
      "Bootstrap Session & Mint Capabilities",
      [Create DuckDB ledger session, verify Ed25519 grant policy at authority epoch $e$, and compile `CompiledPromptProgram` digests.],
      ink,
      kind: "bootstrap",
    ),
    step-row(
      2,
      "Architect & Graph",
      "Actuator / Roles",
      "Propose & Validate WorkGraph Revision",
      [Forced-tool `update_graph` turn emits acyclic `WorkNode` DAG (`Implement`, `Interface`, `Integrate`, `Test`, ...); verify disjoint file footprints.],
      violet,
      kind: "graph-plan",
    ),
    step-row(
      3,
      "Actuator / Roles",
      "Kernel & Policy",
      "Emit Typed Tool Proposal",
      [Actuator proposes typed tool call (`edit_file`, `write_file`, `run_build`, `run_test`); Explorer supplies read-only repository evidence.],
      teal,
      kind: "proposal",
    ),
    step-row(
      4,
      "Kernel & Policy",
      "Candidate & Sensors",
      "Deterministic Admissibility & Starlark Gate",
      [Check capability scope (`EffectKind`, path globs, command rules) and Starlark `.perspt/rules/*.star` policy before mutating `CandidateWorkspace`.],
      amber,
      kind: "admit / deny",
    ),
    step-row(
      5,
      "Candidate & Sensors",
      "Gate & Ledger",
      "Execute Sandboxed Verifier Suite",
      [Run syntax, type, and build checks (`V_syn`), contracts (`V_str`), tests (`V_log`), sensor checks (`V_boot`), and boundary checks (`V_sheaf`) inside `bwrap` / `sandbox-exec`.],
      crimson,
      kind: "measurement",
    ),
    step-row(
      6,
      "Gate & Ledger",
      "Actuator / Roles",
      "Evaluate AcceptGate: Hard Pass or Descent",
      [Accept if all required stages pass or $V(x) <= V(x_("best")) - rho_("gate")$; otherwise consume rejection budget $B$ or open `SearchForest` ($<= 3$ branches).],
      emerald,
      kind: "gate-verdict",
    ),
    step-row(
      7,
      "Gate & Ledger",
      "CLI / Session",
      "Global Integration Gate & Journaled Promotion",
      [Merge node winners in content-addressed `StagingRoot`, pass global integration gate, promote via `openat` descriptors, and append SHA-256 Merkle event.],
      emerald,
      kind: "commit",
    ),
  ),
)
