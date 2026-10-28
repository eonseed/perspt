#import "style.typ": *
#show: diagram-page.with(width: 720pt, margin: 18pt)
#section-banner(
  "A coding task in Perspt",
  "Propose. Check. Repair. Review.",
  subtitle: [Speculative edits stay in a candidate copy. Promotion copies an approved result into the project; it does not create a Git commit.],
)
#v(12pt)
#grid(
  columns: (1fr, 1fr, 1fr, 1fr),
  gutter: 12pt,
  card("Describe the change", [Give a concrete task and project directory. The default run uses one work node. Optional parallel dispatch can plan several.], kicker: "01 / task", accent-color: teal),
  card("Edit a candidate copy", [The model proposes tool calls. Policy and capability checks govern admitted effects before they run in the candidate workspace.], kicker: "02 / propose", accent-color: violet),
  card("Run project checks", [Detected tools measure the result. A full suite runs at gate boundaries; cheaper syntax checks run at mutation boundaries.], kicker: "03 / measure", accent-color: amber),
  card("Review and promote", [Delivery requires a hard pass and approval. Review the diff interactively or use --yes to approve promotion automatically.], kicker: "04 / deliver", accent-color: emerald),
)
#v(12pt)
#block(width: 100%, fill: panel, inset: 11pt, radius: 4pt)[
  #text(size: 9pt)[*When checks fail:* diagnostics guide another attempt within the run's limits. Sufficient descent can keep an intermediate checkpoint. A failed gate restores the best accepted checkpoint; an exhausted run stops with errors remaining.]
]
