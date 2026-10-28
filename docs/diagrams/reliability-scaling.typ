#import "style.typ": *
#show: diagram-page.with(width: 720pt, margin: 18pt)
#section-banner(
  "What verification tells us",
  "A pass is evidence within the checks we run.",
  subtitle: [Coverage comes from the project tools and test policy. No success-rate estimate is implied.],
)
#v(12pt)
#grid(
  columns: (1fr, 1fr, 1fr),
  gutter: 12pt,
  card("Does it build?", [Syntax and build checks catch the errors reported by the detected language toolchain. Missing required tools block a hard pass.], kicker: "01 / syntax and build", accent-color: teal),
  card("Do the tests pass?", [The resulting project suite checks the behavior it covers. Historical regression or external acceptance evidence can add another test view.], kicker: "02 / tested behavior", accent-color: amber),
  card("Are declared goals present?", [Structural checks can find missing declared symbols and interface mismatches. Coverage depends on the contracts and sensors in the run.], kicker: "03 / declared contracts", accent-color: violet),
)
#v(12pt)
#block(width: 100%, fill: teal-soft, inset: 12pt, radius: 4pt)[
  #text(size: 11pt, weight: "bold", fill: teal-dark)[Hard pass: the required configured checks passed.]
  #v(4pt)
  #text(size: 9pt)[Review the diff and the tests. Passing checks cannot establish requirements they do not cover. Tests authored with the implementation are not independent acceptance evidence.]
]
