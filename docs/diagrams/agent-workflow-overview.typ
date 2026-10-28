#import "style.typ": *

#show: diagram-page.with(width: 720pt, margin: 16pt)

#section-banner(
  "Safe Sandbox Workflow",
  "How perspt agent Turns a Task into Working Code",
  subtitle: [Your working tree stays untouched while Perspt edits an isolated copy and verifies every change with your real compiler and tests.],
)

#v(10pt)

#let step-card(num, title, cmd, desc, accent, soft) = block(
  width: 100%,
  height: 118pt,
  fill: paper,
  stroke: (top: 3pt + accent, rest: 0.8pt + rule),
  radius: 5pt,
  inset: (x: 10pt, y: 9pt),
  [
    #grid(
      columns: (20pt, 1fr),
      column-gutter: 6pt,
      align: horizon,
      box(
        width: 20pt,
        height: 20pt,
        radius: 10pt,
        fill: accent,
        align(center + horizon, text(size: 8.5pt, weight: "black", fill: paper)[#num]),
      ),
      text(size: 9.6pt, weight: "black", fill: ink)[#title],
    )
    #v(6pt)
    #block(
      width: 100%,
      fill: soft,
      stroke: 0.6pt + accent,
      radius: 3pt,
      inset: (x: 6pt, y: 4pt),
      text(font: ("DejaVu Sans Mono", "Liberation Mono"), size: 7.2pt, weight: "bold", fill: accent)[#cmd],
    )
    #v(6pt)
    #text(size: 8pt, fill: ink-Mid)[#desc]
  ],
)

#grid(
  columns: (1fr, 16pt, 1fr, 16pt, 1fr, 16pt, 1fr),
  align: (top, center + horizon, top, center + horizon, top, center + horizon, top),
  step-card(
    "1",
    "Give a Task",
    "perspt agent -w ./app",
    [Detects your language toolchain and plans which files need to change.],
    teal,
    teal-soft,
  ),
  text(size: 13pt, weight: "black", fill: teal)[$->$],
  step-card(
    "2",
    "Edit in Sandbox",
    "Candidate Overlay",
    [Applies edits to an isolated copy of your repo. Your real files stay untouched.],
    violet,
    violet-soft,
  ),
  text(size: 13pt, weight: "black", fill: violet)[$->$],
  step-card(
    "3",
    "Compile & Test",
    "cargo / pytest / tsc",
    [Runs your real compiler, LSP, and tests, scoring remaining errors as $V(x)$.],
    amber,
    amber-soft,
  ),
  text(size: 13pt, weight: "black", fill: emerald)[$->$],
  step-card(
    "4",
    "Promote Clean Code",
    "V(x) = 0 -> Commit",
    [Once all checks pass, writes the verified code to your repo with 1-step undo.],
    emerald,
    emerald-soft,
  ),
)

#v(9pt)

// Clean self-repair feedback callout bar connecting Step 3 -> Step 2
#block(
  width: 100%,
  fill: panel,
  stroke: (left: 3pt + amber, rest: 0.8pt + rule),
  radius: 4pt,
  inset: (x: 12pt, y: 8pt),
  grid(
    columns: (auto, 1fr),
    column-gutter: 10pt,
    align: horizon,
    badge("Self-Repair Loop (Step 3 -> Step 2)", fg: amber-dark, bg: amber-soft, border: amber),
    text(size: 8.1pt, fill: ink-Mid)[
      If Step 3 catches a compiler error or failing test ($V(x) > 0$), Perspt automatically discards the broken edit, feeds the exact compiler/test diagnostics back to Step 2, and repairs the code before promoting.
    ],
  ),
)
