#import "style.typ": *

#show: diagram-page.with(width: 720pt, margin: 18pt)

#section-banner(
  "How build and test results guide the agent",
  "Keep improvements. Reject regressions.",
  subtitle: [An example of successive attempts at one coding task. The scores are illustrative, not benchmark results.],
)
#v(12pt)

#grid(
  columns: (450pt, 1fr),
  gutter: 16pt,
  [
    #text(size: 9pt, weight: "bold")[Error score $V$ / lower is better]
    #v(8pt)
    #let px(k) = 24pt + k * 65pt
    #let py(v) = 174pt - v * 14pt
    #let observed = ((0, 10), (1, 7), (2, 9), (3, 4), (4, 6), (5, 1), (6, 0))
    #let accepted = ((0, 10), (1, 7), (3, 4), (5, 1), (6, 0))
    // Horizontal-tangent cubic segments avoid overshooting the sample values.
    #let smooth(points, color, dash: none, thickness: 1.8pt) = curve(
      stroke: (paint: color, thickness: thickness, dash: dash),
      curve.move((px(points.first().at(0)), py(points.first().at(1)))),
      ..range(1, points.len()).map(i => {
        let a = points.at(i - 1)
        let b = points.at(i)
        let mx = (px(a.at(0)) + px(b.at(0))) / 2
        curve.cubic((mx, py(a.at(1))), (mx, py(b.at(1))), (px(b.at(0)), py(b.at(1))))
      }),
    )
    #box(width: 450pt, height: 210pt)[
      #for v in (0, 5, 10) {
        place(dx: 0pt, dy: py(v) - 4pt, text(size: 8pt, fill: muted)[#v])
        place(top + left, line(start: (px(0), py(v)), end: (px(6), py(v)), stroke: 0.6pt + rule))
      }
      #place(top + left, smooth(observed, amber, dash: "dashed", thickness: 1.3pt))
      #place(top + left, smooth(accepted, teal, thickness: 2.6pt))
      #for (k, v) in accepted {
        place(dx: px(k) - 3.5pt, dy: py(v) - 3.5pt, circle(radius: 3.5pt, fill: teal, stroke: 1pt + paper))
      }
      #for (k, v) in ((2, 9), (4, 6)) {
        place(dx: px(k) - 4pt, dy: py(v) - 4pt, circle(radius: 4pt, fill: paper, stroke: 1.8pt + amber))
        place(dx: px(k) - 24pt, dy: py(v) - 20pt, text(size: 8pt, weight: "bold", fill: amber-dark)[Rejected])
      }
      #place(dx: px(3) - 30pt, dy: py(4) + 14pt, text(size: 8pt, weight: "bold", fill: teal-dark)[Kept checkpoint])
      #place(dx: px(6) - 24pt, dy: py(0) - 23pt, text(size: 8pt, weight: "bold", fill: teal-dark)[Checks passed])
      #for k in range(7) {
        place(dx: px(k) - 2pt, dy: 185pt, text(size: 8pt, fill: muted)[#k])
      }
      #place(dx: 150pt, dy: 201pt, text(size: 8pt, fill: muted)[Verification attempt])
    ]
    #text(size: 8pt, fill: teal-dark, weight: "bold")[Filled dot: kept checkpoint]
    #h(10pt)
    #text(size: 8pt, fill: amber-dark, weight: "bold")[Open dot: rejected attempt]
  ],
  stack(
    spacing: 10pt,
    card("1. Run the checks", [Build errors, failing tests, and other reported problems contribute to the error score $V$. Lower is better.], accent-color: amber),
    card("2. Keep improvements", [Perspt can keep an intermediate version that reduces the error score enough. This does not yet update the original project.], accent-color: teal),
    card("3. Review the result", [The required checks must pass before edits can be applied. If the agent runs out of attempts, it reports the remaining errors.], accent-color: violet),
  ),
)
#v(12pt)
#block(fill: panel, inset: 9pt, radius: 4pt, width: 100%)[
  #text(size: 8.5pt)[*Smooth formula, discrete decisions.* $V(r) = sum_e w_e r_e^2$ is continuous in the residual magnitudes. The coding runtime samples discrete candidate states; residual counts can jump. Curves connect samples for readability and do not claim a continuous path through code space.]
]
