.. _api-perspt-sdk:

``perspt-sdk``
==============

The domain-neutral SRBN control plane (PSP-8): residual evidence, the
canonical quadratic energy, the measured acceptance gate, capabilities,
scheduling, and the replay ledger. Domain packages such as ``perspt-coding``
provide task-specific residual construction, weights, and correction
directions on top.

Modules
-------

.. list-table::
   :header-rows: 1
   :widths: 25 75

   * - Module
     - Description
   * - ``residual``
     - ``ResidualEvent`` - the measured reason a state is unsafe or incomplete
   * - ``energy``
     - ``score_candidate``/``EnergyModel`` - the canonical quadratic residual
       energy ``V(x) = sum_e w_e ||r_e||^2``
   * - ``gate``
     - Measured acceptance gate and finite-decision bound
   * - ``kernel``
     - Adapter over the published ``srbn`` kernel crate
   * - ``domain``
     - ``AgentDomainPackage`` - the domain-package contract
   * - ``capability``
     - Capability-constrained admissibility kernel: proposals, never
       unmediated effects
   * - ``admissibility``
     - The five-clause admissibility kernel (PSP-9)
   * - ``command``
     - Typed command IR and governance tiers (no implicit ``sh -c``)
   * - ``scheduler``
     - Dependency-aware parallel ready-queue scheduler
   * - ``workgraph``
     - Mutable, revisioned work graph
   * - ``ledger``
     - Append-only, Merkle-chained event stream with audit replay
   * - ``checkpoint``
     - Context checkpoints: compaction as recorded projection, never deletion
   * - ``conformal``
     - Calibrated risk budgets via conformal risk control
   * - ``recovery``
     - The recovery lattice for classified failures
   * - ``routing``
     - Phase-aware model routing (``ModelRoute``, ``ModelTier``)
   * - ``model``
     - Provider-neutral model plane identity and contract types (pure data)
   * - ``prompt``
     - The model-conditioned prompt plane: typed sections, composition,
       digested programs (PSP-10)
   * - ``search``
     - The bounded search plane: forests, branches, witnesses, limits
       (PSP-10)
   * - ``toolset``
     - The typed tool catalog with effect/risk/footprint contracts
   * - ``independence``
     - Measured validator dependence, never assumed independence
   * - ``spectral``
     - Spectral energy-slope constant ``mu`` from the verification graph
   * - ``stability``
     - Analytic stability claims (``NotClaimed`` for the coding domain)
   * - ``certificate``
     - ``ResidualCertificate`` - the honest stop on exhaustion
   * - ``goal``
     - Goal-presence sensor: the verifier that refuses false stability
   * - ``observability``
     - Read-only dashboard/TUI projections over the event ledger
   * - ``benchmark``
     - Mechanism-check benchmark harness and metrics
   * - ``canon``
     - Domain-tagged canonical byte encoding for replayable digests
   * - ``scope``
     - Glob-like scope patterns shared by capabilities and grant policies
   * - ``error``
     - ``SdkError``/``Result`` - fail-closed numerical contracts

The crate re-exports the published ``srbn``, ``srbn_ledger``, and
``srbn_serde`` kernel crates so consumers depend on one SRBN source.

Key Types
---------

**evaluate_gate** - The measured acceptance gate:

.. code-block:: rust

   pub fn evaluate_gate(
       hard_pass: bool,
       candidate_v: f64,
       best_accepted_v: f64,
       rho_gate: f64,
   ) -> Result<GateDecision>;

   pub fn finite_decision_bound(
       baseline_energy: f64,
       rho_gate: f64,
       rejection_budget: u32,
   ) -> Result<u64>;

**EnergyScore** - The result of scoring a candidate:

.. code-block:: rust

   pub struct EnergyScore {
       pub total: f64,                        // Total energy V
       pub components: EnergyComponents,      // Derived rollups
       pub dominant: Vec<ResidualEventRef>,   // Dominant residuals first
       pub hard_violations: Vec<ResidualClass>,
   }
