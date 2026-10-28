#import "style.typ": *

#show: diagram-page.with(width: 720pt, margin: 16pt)

#section-banner(
  "Figure 3 · PSP-000010 Bounded Speculative Search",
  "Lyapunov Energy Trajectories: Unguided Retries vs. Bounded Search Forest",
  subtitle: [Comparison of (a) blind single-path prompt retries under persistent compiler/test failures vs. (b) PSP-10 System 21 isolated strategy branches ($B <= 3$) with exact no-good pruning and single gate commit.],
)

#v(8pt)

#grid(
  columns: (1fr, 1fr),
  gutter: 12pt,
  // Panel A: Unguided Retries
  block(
    width: 100%,
    fill: panel,
    stroke: (top: 2.5pt + crimson, rest: 0.8pt + rule),
    radius: 4pt,
    inset: 10pt,
    [
      #text(size: 9.5pt, weight: "black", fill: crimson-dark)[(a) Unguided Single-Path Retries]
      #v(2pt)
      #text(size: 7.6pt, fill: muted)[Repeatedly prompting the same context oscillates between syntax and logic regressions until budget exhaustion.]
      #v(6pt)
      #let W = 315pt
      #let H = 165pt
      #let px(k) = 28pt + (k / 7.0) * (W - 42pt)
      #let py(v) = 10pt + ((12.0 - v) / 12.0) * (H - 28pt)
      #box(width: W, height: H)[
        // Grid
        #for v in (0, 3, 6, 9, 12) {
          place(
            top + left,
            line(
              start: (px(0), py(v)),
              end: (px(7), py(v)),
              stroke: if v == 0 {
                (paint: emerald, thickness: 1pt, dash: "dashed")
              } else {
                (paint: rule, thickness: 0.5pt, dash: "dotted")
              },
            ),
          )
          place(dx: 2pt, dy: py(v) - 4pt, text(size: 6.8pt, fill: muted)[$V=#v$])
        }
        // Oscillating high-energy paths
        #let p1 = ((0, 8.5), (1, 6.2), (2, 7.8), (3, 9.1), (4, 7.4), (5, 8.8), (6, 9.6), (7, 10.2))
        #let p2 = ((0, 8.5), (1, 9.4), (2, 6.8), (3, 8.2), (4, 9.9), (5, 7.5), (6, 8.4), (7, 9.1))
        #place(
          top + left,
          curve(
            stroke: (paint: crimson, thickness: 1.7pt),
            curve.move((px(0), py(8.5))),
            ..p1.slice(1).map(pt => curve.line((px(pt.at(0)), py(pt.at(1))))),
          ),
        )
        #place(
          top + left,
          curve(
            stroke: (paint: amber, thickness: 1.3pt, dash: "dashed"),
            curve.move((px(0), py(8.5))),
            ..p2.slice(1).map(pt => curve.line((px(pt.at(0)), py(pt.at(1))))),
          ),
        )
        #place(
          dx: px(2.1),
          dy: py(11.3),
          badge("Repeated No-Goods · Budget Exhausted (V = 10.2)", fg: crimson-dark, bg: crimson-soft, border: crimson),
        )
        #place(
          dx: px(0),
          dy: py(12),
          rect(width: px(7) - px(0), height: py(0) - py(12), stroke: 0.8pt + ink, fill: none),
        )
        #place(
          dx: px(2.2),
          dy: H - 9pt,
          text(size: 7.2pt, weight: "bold", fill: ink)[Retry Turn $k in [0, 7]$],
        )
      ]
    ],
  ),

  // Panel B: Bounded Search Forest
  block(
    width: 100%,
    fill: panel,
    stroke: (top: 2.5pt + teal, rest: 0.8pt + rule),
    radius: 4pt,
    inset: 10pt,
    [
      #text(size: 9.5pt, weight: "black", fill: teal-dark)[(b) Bounded Search Forest (PSP-10 System 21)]
      #v(2pt)
      #text(size: 7.6pt, fill: muted)[At most 3 isolated branches from accepted root $x_0$; failed branches record exact no-goods; winner commits via single gate.]
      #v(6pt)
      #let W = 315pt
      #let H = 165pt
      #let px(k) = 28pt + (k / 7.0) * (W - 42pt)
      #let py(v) = 10pt + ((12.0 - v) / 12.0) * (H - 28pt)
      #box(width: W, height: H)[
        #for v in (0, 3, 6, 9, 12) {
          place(
            top + left,
            line(
              start: (px(0), py(v)),
              end: (px(7), py(v)),
              stroke: if v == 0 {
                (paint: emerald, thickness: 1pt, dash: "dashed")
              } else {
                (paint: rule, thickness: 0.5pt, dash: "dotted")
              },
            ),
          )
          place(dx: 2pt, dy: py(v) - 4pt, text(size: 6.8pt, fill: muted)[$V=#v$])
        }
        // Branch b1 (DirectMinimal) -> stalls at k=2, pruned, emits NoGood
        #place(
          top + left,
          curve(
            stroke: (paint: muted, thickness: 1.3pt, dash: "dashed"),
            curve.move((px(0), py(8.5))),
            curve.line((px(1), py(5.5))),
            curve.line((px(2), py(5.8))),
          ),
        )
        #place(
          dx: px(2.1),
          dy: py(6.3),
          badge("b1 Pruned -> NoGood Recorded", fg: ink-Mid, bg: paper, border: muted),
        )

        // Branch b2 (InterfaceFirst) -> steady descent to V=0 at k=5
        #let b2 = ((0, 8.5), (1, 4.8), (2, 2.6), (3, 1.1), (4, 0.3), (5, 0.0))
        #place(
          top + left,
          curve(
            stroke: (paint: teal, thickness: 2.1pt),
            curve.move((px(0), py(8.5))),
            ..b2.slice(1).map(pt => curve.line((px(pt.at(0)), py(pt.at(1))))),
          ),
        )
        // Branch b3 (TestGuided) -> descends to V=1.8
        #let b3 = ((0, 8.5), (1, 6.0), (2, 3.9), (3, 2.4), (4, 1.8))
        #place(
          top + left,
          curve(
            stroke: (paint: violet, thickness: 1.3pt, dash: "densely-dotted"),
            curve.move((px(0), py(8.5))),
            ..b3.slice(1).map(pt => curve.line((px(pt.at(0)), py(pt.at(1))))),
          ),
        )

        #place(
          dx: px(2.7),
          dy: py(1.3),
          badge("b2 Winner Selected -> Single Gate Commit (V = 0)", fg: emerald-dark, bg: emerald-soft, border: emerald),
        )
        #place(
          dx: px(0),
          dy: py(12),
          rect(width: px(7) - px(0), height: py(0) - py(12), stroke: 0.8pt + ink, fill: none),
        )
        #place(
          dx: px(1.8),
          dy: H - 9pt,
          text(size: 7.2pt, weight: "bold", fill: ink)[Forest Branch Step $k$ (Pre-Reserved Budget)],
        )
      ]
    ],
  ),
)
