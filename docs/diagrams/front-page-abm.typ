#import "style.typ": *

#show: diagram-page.with(width: 720pt, margin: 16pt)

#section-banner(
  "Figure 1 · Stochastic Trajectory Stabilization",
  "Agent-Based Model Dynamics: Uncontrolled Drift vs. SRBN Sheaf Consensus",
  subtitle: [Ensemble of $M = 12$ autonomous coding trajectories over $T = 120$ steps under quadratic Lyapunov energy $V(x) = sum_e w_e norm(r_e (x))^2$.],
)

#v(8pt)

#grid(
  columns: (1fr, 185pt),
  gutter: 14pt,
  // Left: Coordinate plot rendered in native vector primitives
  block(
    width: 100%,
    fill: panel,
    stroke: 0.8pt + rule,
    radius: 5pt,
    inset: 10pt,
    [
      #let W = 470pt
      #let H = 230pt
      // Map t in [0, 100], y in [-12, 12] to (px, py)
      #let px(t) = 36pt + (t / 100.0) * (W - 52pt)
      #let py(y) = 12pt + ((12.0 - y) / 24.0) * (H - 34pt)

      #box(width: W, height: H)[
        // Shaded Unsafe Barrier Regions |y| > 8.5
        #place(
          dx: px(0),
          dy: py(12),
          rect(
            width: px(100) - px(0),
            height: py(8.5) - py(12),
            fill: crimson.transparentize(90%),
            stroke: (bottom: (paint: crimson, thickness: 0.8pt, dash: "dashed")),
          ),
        )
        #place(
          dx: px(0),
          dy: py(-8.5),
          rect(
            width: px(100) - px(0),
            height: py(-12) - py(-8.5),
            fill: crimson.transparentize(90%),
            stroke: (top: (paint: crimson, thickness: 0.8pt, dash: "dashed")),
          ),
        )

        // Shaded SRBN Confidence Funnel
        #place(
          top + left,
          polygon(
            fill: teal-bright.transparentize(82%),
            stroke: none,
            (px(0), py(5.8)),
            (px(15), py(3.4)),
            (px(30), py(2.0)),
            (px(50), py(1.0)),
            (px(75), py(0.45)),
            (px(100), py(0.22)),
            (px(100), py(-0.22)),
            (px(75), py(-0.45)),
            (px(50), py(-1.0)),
            (px(30), py(-2.0)),
            (px(15), py(-3.4)),
            (px(0), py(-5.8)),
          ),
        )

        // Horizontal grid lines
        #for yval in (-10, -5, 0, 5, 10) {
          place(
            top + left,
            line(
              start: (px(0), py(yval)),
              end: (px(100), py(yval)),
              stroke: if yval == 0 {
                (paint: emerald, thickness: 1.1pt, dash: "densely-dashed")
              } else {
                (paint: rule, thickness: 0.5pt, dash: "dotted")
              },
            ),
          )
          place(
            dx: 2pt,
            dy: py(yval) - 4.5pt,
            text(size: 7pt, fill: muted)[#if yval > 0 [+#yval] else [#yval]],
          )
        }

        // Vertical grid lines & time ticks
        #for tval in (0, 20, 40, 60, 80, 100) {
          place(
            top + left,
            line(
              start: (px(tval), py(12)),
              end: (px(tval), py(-12)),
              stroke: (paint: rule, thickness: 0.5pt, dash: "dotted"),
            ),
          )
          place(
            dx: px(tval) - 8pt,
            dy: py(-12) + 4pt,
            text(size: 7pt, fill: muted)[#tval],
          )
        }

        // Uncontrolled drifting trajectories (crimson)
        #let drift-paths = (
          ((0, 2.2), (18, 4.1), (36, 3.3), (54, 6.8), (74, 8.9), (92, 11.1), (100, 11.6)),
          ((0, -1.8), (16, -3.5), (34, -5.9), (52, -4.8), (72, -8.4), (88, -10.4), (100, -11.3)),
          ((0, 3.8), (20, 5.4), (38, 7.6), (58, 6.2), (78, 9.5), (100, 10.4)),
          ((0, -3.2), (22, -2.1), (42, -5.5), (62, -7.8), (82, -9.2), (100, -10.1)),
          ((0, 0.9), (24, 3.2), (46, 5.1), (66, 4.4), (84, 7.2), (100, 8.3)),
          ((0, -0.7), (20, -2.8), (40, -1.9), (60, -4.9), (80, -6.5), (100, -7.9)),
        )
        #for pts in drift-paths {
          place(
            top + left,
            curve(
              stroke: (paint: crimson.transparentize(30%), thickness: 1.15pt),
              curve.move((px(pts.at(0).at(0)), py(pts.at(0).at(1)))),
              ..pts.slice(1).map(p => curve.line((px(p.at(0)), py(p.at(1))))),
            ),
          )
        }

        // SRBN Stabilized trajectories (teal/cyan) converging to V(x) = 0
        #let srbn-paths = (
          ((0, 5.2), (12, 3.1), (25, 1.8), (40, 0.95), (58, 0.42), (78, 0.14), (100, 0.04)),
          ((0, -5.0), (12, -2.9), (25, -1.6), (40, -0.82), (58, -0.35), (78, -0.12), (100, -0.03)),
          ((0, 3.4), (14, 1.7), (28, 0.8), (44, 0.35), (64, 0.12), (84, 0.04), (100, 0.0)),
          ((0, -3.6), (14, -1.9), (28, -0.9), (44, -0.4), (64, -0.15), (84, -0.05), (100, -0.01)),
          ((0, 1.9), (15, 0.8), (32, 0.25), (50, 0.08), (72, 0.02), (100, 0.0)),
          ((0, -2.1), (15, -0.9), (32, -0.3), (50, -0.1), (72, -0.02), (100, 0.0)),
        )
        #for pts in srbn-paths {
          place(
            top + left,
            curve(
              stroke: (paint: teal, thickness: 1.5pt),
              curve.move((px(pts.at(0).at(0)), py(pts.at(0).at(1)))),
              ..pts.slice(1).map(p => curve.line((px(p.at(0)), py(p.at(1))))),
            ),
          )
        }

        // Labels inside plot
        #place(
          dx: px(56),
          dy: py(10.8),
          badge("Barrier Violation B(x) < 0 (Unguided Drift)", fg: crimson-dark, bg: crimson-soft, border: crimson),
        )
        #place(
          dx: px(46),
          dy: py(1.9),
          badge("SRBN Sheaf Consensus Manifold V(x*) = 0", fg: emerald-dark, bg: emerald-soft, border: emerald),
        )

        // Axes border
        #place(
          dx: px(0),
          dy: py(12),
          rect(width: px(100) - px(0), height: py(-12) - py(12), stroke: 0.9pt + ink, fill: none),
        )
        #place(
          dx: px(38),
          dy: H - 10pt,
          text(size: 7.8pt, weight: "bold", fill: ink)[Verification & Tool-Loop Horizon $t$ (Turns)],
        )
      ]
    ],
  ),

  // Right: Mathematical & Operational Legend
  stack(
    spacing: 8pt,
    card(
      "Closed-Loop Descent",
      [
        Every accepted node candidate satisfies strict descent:
        $ V(x_(k+1)) <= V(x_k) - rho_("gate") $
        or achieves a hard pass across all required sensors.
      ],
      kicker: "Lyapunov Gate",
      accent-color: teal,
    ),
    card(
      "Zero-Drift Invariant",
      [
        Non-descending proposals are rejected against the shared budget $B$ and discarded from the `CandidateWorkspace` overlay without touching user files.
      ],
      kicker: "Safety Barrier B(x) >= 0",
      accent-color: crimson,
    ),
    card(
      "Sheaf Consensus",
      [
        Cross-node interface seals and import graphs force stalks to agree on shared boundaries ($V_("sheaf") = 0$) before global promotion.
      ],
      kicker: "Multi-Node Agreement",
      accent-color: emerald,
    ),
  ),
)
