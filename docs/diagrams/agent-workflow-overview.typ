#import "style.typ": *
#show: diagram-page.with(width: 720pt, margin: 18pt)
#section-banner(
  "How the coding agent works",
  "Describe a task. Build and test. Review the changes.",
  subtitle: [Perspt edits a separate copy of the project and asks for approval before updating the original files.],
)
#v(12pt)
#grid(
  columns: (1fr, 1fr, 1fr, 1fr),
  gutter: 12pt,
  card("Give it a task", [Describe a bug to fix, a feature to add, or tests to write. Set the project folder with -w.], kicker: "01 / describe", accent-color: teal),
  card("Edit a project copy", [The agent reads the code and makes edits in a separate copy. The original project files stay unchanged while it works.], kicker: "02 / edit", accent-color: violet),
  card("Run builds and tests", [Perspt runs the project's required checks. Build errors and failing tests help the agent work out what to fix next.], kicker: "03 / test", accent-color: amber),
  card("Review and apply", [Review the diff and test results. Approve to copy the changes into the project, or ask for a correction.], kicker: "04 / review", accent-color: emerald),
)
#v(12pt)
#block(width: 100%, fill: panel, inset: 11pt, radius: 4pt)[
  #text(size: 9pt)[*If the agent cannot finish:* it stops at the run's attempt limit and reports the remaining problems. The --yes option approves applying a successful result automatically. Applying edits does not create a Git commit.]
]
