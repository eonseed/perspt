.. _api-perspt-coding:

``perspt-coding``
=================

The coding domain package built on ``perspt-sdk``: it declares the coding
residual schema, supplies the coding energy model (weights, ``rho_gate``,
correction budget), and maps dominant residuals into coding correction
directions.

Modules
-------

.. list-table::
   :header-rows: 1
   :widths: 25 75

   * - Module
     - Description
   * - ``barrier``
     - ``OperationalSafetyBarrier`` - versioned operational safety channels;
       correctness evidence stays in ``V``, never duplicated into ``h``
   * - ``diag``
     - The structured verifier plane: adapters preserve as much source
       structure as each tool provides
   * - ``lang``
     - Language adapters (Rust, Python, TypeScript) as verifier suites
   * - ``prompts``
     - The coding prompt section library; ``branch_correct`` renders a typed
       ``CorrectionPacket`` for the model
   * - ``registry``
     - ``CodingAdapterRegistry``/``LanguageId`` - open language-adapter
       registry keyed by id, never enum dispatch
   * - ``runtime``
     - Generic runtime smoke-probe scheme for exercising built artifacts
   * - ``symbols``
     - Symbol extraction for the goal-presence sensor

Key Types
---------

**CodingDomain** - The ``AgentDomainPackage`` implementation:

.. code-block:: rust

   pub struct CodingDomain;

   impl AgentDomainPackage for CodingDomain {
       fn domain_id(&self) -> DomainId;                 // "coding"
       fn detect(&self, workspace: &WorkspaceSnapshot) -> DomainDetection;
       fn residual_schema(&self, scope: &DomainScope) -> ResidualSchema;
       fn energy_model(&self, scope: &DomainScope) -> EnergyModel;
       fn correction_directions(&self, residuals: &[ResidualEvent])
           -> Vec<CorrectionDirection>;
       // ... hard_gate_policy, verifier_suite, safety_barrier
   }

**LanguageId** - Open, stable language identifier:

.. code-block:: rust

   pub struct LanguageId(pub String);

   pub struct CodingAdapterRegistry {
       adapters: BTreeMap<LanguageId, Box<dyn LanguageAdapter>>,
   }

The coding domain operates on discrete verifier residuals (compiler, LSP,
AST, tests), so its analytic constants remain ``NotClaimed``; only the
measured discrete gate and the spectral ``mu`` apply.
