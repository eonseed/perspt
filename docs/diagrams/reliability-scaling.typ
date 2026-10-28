#import "style.typ": *

#show: diagram-page.with(width: 720pt, margin: 16pt)

#section-banner(
  "Compounding Error vs. Closed-Loop Control",
  "Multi-Step Task Reliability: Open-Loop Guessing vs. SRBN Verification",
  subtitle: [Comparison of unverified multi-step LLM accuracy $P_("open")(N) = (1 - epsilon)^N$ ($epsilon = 0.05$) against Perspt's compiler- and test-gated SRBN loop across $N in [1, 50]$ steps.],
)

#v(8pt)

#grid(
  columns: (1fr, 195pt),
  gutter: 14pt,
  block(
    width: 100%,
    fill: panel,
    stroke: 0.8pt + rule,
    radius: 5pt,
    inset: 10pt,
    [
      #let W = 460pt
      #let H = 225pt
      // Map n in [1, 50], p in [0, 100]
      #let px(n) = 38pt + ((n - 1.0) / 49.0) * (W - 54pt)
      #let py(p) = 12pt + ((100.0 - p) / 100.0) * (H - 34pt)

      #box(width: W, height: H)[
        // Shaded Reliability Gain Polygon between SRBN and Open-Loop
        #let srbn-pts = (
          (1, 99.8), (5, 99.3), (10, 98.7), (15, 98.2), (20, 97.6),
          (25, 97.1), (30, 96.6), (35, 96.1), (40, 95.6), (45, 95.2), (50, 94.8),
        )
        #let open-pts = (
          (1, 95.0), (5, 77.4), (10, 59.9), (15, 46.3), (20, 35.8),
          (25, 27.7), (30, 21.5), (35, 16.6), (40, 12.9), (45, 9.9), (50, 7.7),
        )

        #place(
          top + left,
          polygon(
            fill: teal-bright.transparentize(84%),
            stroke: none,
            ..srbn-pts.map(pt => (px(pt.at(0)), py(pt.at(1)))),
            ..open-pts.rev().map(pt => (px(pt.at(0)), py(pt.at(1)))),
          ),
        )

        // Horizontal grid lines
        #for pval in (0, 25, 50, 75, 100) {
          place(
            top + left,
            line(
              start: (px(1), py(pval)),
              end: (px(50), py(pval)),
              stroke: if pval == 50 {
                (paint: amber, thickness: 1pt, dash: "dashed")
              } else {
                (paint: rule, thickness: 0.5pt, dash: "dotted")
              },
            ),
          )
          place(
            dx: 2pt,
            dy: py(pval) - 4.5pt,
            text(size: 7pt, fill: muted)[#pval%],
          )
        }

        // Vertical grid lines
        #for nval in (1, 10, 20, 30, 40, 50) {
          place(
            top + left,
            line(
              start: (px(nval), py(100)),
              end: (px(nval), py(0)),
              stroke: (paint: rule, thickness: 0.5pt, dash: "dotted"),
            ),
          )
          place(
            dx: px(nval) - 6pt,
            dy: py(0) + 4pt,
            text(size: 7pt, fill: muted)[#nval],
          )
        }

        // Open-loop curve
        #place(
          top + left,
          curve(
            stroke: (paint: crimson, thickness: 1.8pt, dash: "densely-dashed"),
            curve.move((px(1), py(95.0))),
            ..open-pts.slice(1).map(pt => curve.line((px(pt.at(0)), py(pt.at(1))))),
          ),
        )

        // SRBN closed-loop curve
        #place(
          top + left,
          curve(
            stroke: (paint: teal, thickness: 2.2pt),
            curve.move((px(1), py(99.8))),
            ..srbn-pts.slice(1).map(pt => curve.line((px(pt.at(0)), py(pt.at(1))))),
          ),
        )

        // Callout badges
        #place(
          dx: px(16),
          dy: py(91),
          badge("SRBN Closed-Loop Control (>= 94.8% at N=50)", fg: teal-dark, bg: teal-soft, border: teal),
        )
        #place(
          dx: px(22),
          dy: py(53),
          badge("50% Usability Threshold (Open-Loop Fails at N=14)", fg: amber-dark, bg: amber-soft, border: amber),
        )
        #place(
          dx: px(24),
          dy: py(24),
          badge("Open-Loop Decay (1 - e)^N -> 7.7% at N=50", fg: crimson-dark, bg: crimson-soft, border: crimson),
        )

        // Frame
        #place(
          dx: px(1),
          dy: py(100),
          rect(width: px(50) - px(1), height: py(0) - py(100), stroke: 0.9pt + ink, fill: none),
        )
        #place(
          dx: px(17),
          dy: H - 10pt,
          text(size: 7.8pt, weight: "bold", fill: ink)[Sequential Task Horizon $N$ (Dependent Coding Steps)],
        )
      ]
    ],
  ),

  stack(
    spacing: 8pt,
    card(
      "Why Open-Loop Fails",
      [
        At 95% per-step accuracy ($epsilon = 0.05$), an unverified agent drops below 50% success after just *14 steps* and reaches *7.7%* at 50 steps.
      ],
      kicker: "Exponential Decay",
      accent-color: crimson,
    ),
    card(
      "Bounded Decision Horizon",
      [
        With minimum energy descent $rho_("gate") > 0$ and finite rejection budget $B$, each node terminates in at most:
        $ floor(V_0 / rho_("gate")) + B + 1 $
        gate evaluations.
      ],
      kicker: "Finite-Step Guarantee",
      accent-color: teal,
    ),
    card(
      "Independent Verifiers",
      [
        Compiler, LSP, test runner, and structural AST sensors catch errors immediately at step $k$ before they contaminate step $k+1$.
      ],
      kicker: "Orthogonal Sensors",
      accent-color: emerald,
    ),
  ),
)
