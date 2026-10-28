#import "style.typ": *
#show: diagram-page.with(width: 720pt, margin: 18pt)
#section-banner(
  "What builds and tests can tell us",
  "Passing tests can still miss bugs.",
  subtitle: [Review the code and the tests together. These are the kinds of problems the tools can find.],
)
#v(12pt)
#grid(
  columns: (1fr, 1fr, 1fr),
  gutter: 12pt,
  card("Does it build?", [Syntax and build checks catch the errors reported by the detected language toolchain. A required check must be able to run before the result can be accepted.], kicker: "01 / syntax and build", accent-color: teal),
  card("Do the tests pass?", [The project test suite checks the cases it contains. Existing tests or a separately maintained acceptance suite can add coverage.], kicker: "02 / tested behavior", accent-color: amber),
  card("Are required functions present?", [Where a task declares required names and interfaces, structural checks can find missing functions or incompatible definitions.], kicker: "03 / declared contracts", accent-color: violet),
)
#v(12pt)
#block(width: 100%, fill: teal-soft, inset: 12pt, radius: 4pt)[
  #text(size: 11pt, weight: "bold", fill: teal-dark)[A successful check means the configured tools passed.]
  #v(4pt)
  #text(size: 9pt)[A compiler can accept code that has a logic error. A test suite can omit an important case. Review the result, and use separately maintained tests when the task needs them.]
]
