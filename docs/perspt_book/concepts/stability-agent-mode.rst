.. _stability-agent-mode:

Stability and Measured Progress
===============================

**Perspt separates a model's proposal from evidence that the proposal works.** The runtime measures candidate states with project tools, then decides whether to keep progress or ask for a correction.
This page describes the implemented coding contract and how it relates to the stability terminology used in the specifications.

The energy score
----------------

A verifier reports a residual: a measured issue such as a compiler diagnostic, failing test, or missing declared symbol.
For residual magnitudes :math:`r_e \geq 0` and positive weights :math:`w_e`, the SDK computes:

.. math::

   V(r) = \sum_e w_e r_e^2

**The quadratic formula is continuous in the residual magnitudes.** The coding runtime, however, measures discrete candidate workspaces.
Counts of diagnostics and failing tests can jump between measurements.
There is no implemented continuous coordinate system over source-code states and no guarantee that every path through code space is smooth.
The coding domain declares its continuous analytic stability constants ``NotClaimed``.

.. figure:: ../../shared/_static/front-page-abm.svg
   :target: ../_static/front-page-abm.svg
   :alt: Smooth illustrative curves connect discrete measured scores. Filled dots are kept checkpoints and open dots are rejected attempts.
   :width: 100%

   The dots are the decisions that matter. Curves guide the eye between illustrative measurements; they are not a simulation, benchmark, or continuous coding trajectory.

The five reported components group the same weighted contributions:

- **Syntax**: Syntax, type, and build residuals.
- **Structure**: Import, symbol, interface, and other structural residuals.
- **Logic**: Test failures, runtime failures, and regression residuals.
- **Bootstrap**: Missing required sensors and tool failures.
- **Sheaf**: Cross-node consistency residuals.

The components sum to the total without a second weighting step.
Capability denials and budget exhaustion are recorded in a separate blocked channel rather than added to this score.
See ``crates/perspt-sdk/src/energy.rs`` and ``crates/perspt-coding/src/lib.rs`` for the scoring implementation and coding weights.

Keep progress without claiming completion
-----------------------------------------

For the ordinary measured gate:

.. math::

   \operatorname{accept}(y) \iff \operatorname{hard\_pass}(y)
   \quad \lor \quad V(y) \leq V(x_{\mathrm{best}}) - \rho_{\mathrm{gate}}

A hard pass means all required verifiers and hard constraints passed.
Otherwise, the candidate must reduce the best accepted energy by at least :math:`\rho_{\mathrm{gate}}` (default ``0.5``).
The comparison uses ``<=``, so a decrease exactly equal to the margin is sufficient.

**Acceptance by descent is an intermediate checkpoint.** It does not authorize delivery of incomplete code.
Final promotion requires the required checks and approval; multi-node runs also check the merged integration result.
Zero measured energy alone is not sufficient evidence that every required stage ran and passed.
Likewise, an empty ``EnergyScore.hard_violations`` list only says no configured residual threshold was exceeded.

The general SDK gate can classify a stop at a declared analytic floor.
That terminal outcome is not verified success.
The coding domain does not claim an analytic floor.
The implemented ordering and outcomes are in ``crates/perspt-sdk/src/gate.rs``.

Bound the attempts
------------------

With baseline energy :math:`V_0`, positive descent margin :math:`\rho_{\mathrm{gate}}`, and rejection budget :math:`B`, the measured node gate has the bound:

.. math::

   N_{\mathrm{gate}} \leq \left\lfloor V_0 / \rho_{\mathrm{gate}} \right\rfloor + B + 1

This bounds gate decisions under the declared contract.
It does not promise that a model will find a passing candidate or establish a model success probability.
Turn, call, and recovery limits also apply.
When a run cannot complete within its limits, it stops with a classified outcome and recorded remaining errors.

Observe, accept, and promote
----------------------------

We distinguish three events:

1. **Observe**: The runtime measures a proposed candidate and records its evidence.
2. **Accept**: A hard pass or sufficient descent advances the node's accepted trajectory.
3. **Promote**: The runtime copies a deliverable, approved result into the project.

The ledger records rejected observations as well as accepted checkpoints.
Rejected proposals do not become accepted progress and do not modify the project files through promotion.
Ledger checkpoints and promotion are not Git commits.

The scope of verification
-------------------------

A required check that cannot run blocks a hard pass.
Non-required stages can report advisory failures without changing the hard gate.
The default test policy measures the resulting implementation and tests together.
Historical regression and externally maintained acceptance evidence are explicit policies described in :doc:`../configuration`.

**A hard pass is evidence within the configured checks' coverage.** It cannot establish behavior that the checks do not cover.
Optional spectral and validator-dependence statistics are diagnostics, not acceptance inputs.

Relation to the stability papers
--------------------------------

Perspt's specifications use the *Stability is All You Need* paper series as a design framework for measured progress, bounded correction, and governed effects.
The implemented coding runtime uses the discrete measured gate above.
Continuous geometric claims require additional assumptions that the coding package does not declare.
The specifications preserve the design rationale; :doc:`srbn-architecture` describes the runtime mechanisms and their limits.
