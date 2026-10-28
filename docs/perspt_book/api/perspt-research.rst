.. _api-perspt-research:

``perspt-research``
===================

The research domain package (skeleton): the second-domain readiness check
proving the ``perspt-sdk`` contracts admit another domain without forking the
scheduler, admissibility kernel, residual model, replay ledger, or dashboard
event model.

Core Type
---------

**ResearchDomain** - The ``AgentDomainPackage`` implementation:

.. code-block:: rust

   pub struct ResearchDomain;

   impl AgentDomainPackage for ResearchDomain {
       fn domain_id(&self) -> DomainId;   // "research"
       // detect, residual_schema, energy_model, correction_directions,
       // hard_gate_policy, verifier_suite, safety_barrier
   }

Residual Mapping
----------------

Research reuses the SDK residual classes:

.. list-table::
   :header-rows: 1
   :widths: 35 65

   * - Residual Class
     - Research Meaning
   * - ``TestFailure``
     - Unsupported / contradicted claim
   * - ``InterfaceMismatch``
     - Source mismatch
   * - ``ImportGraph``
     - Citation gap (missing source link)
   * - ``ContextDrift``
     - Stale evidence
   * - ``SheafInconsistency``
     - Cross-source contradiction

Detection looks for bibliography markers (``references.bib``, ``refs.bib``,
``bibliography.bib``, ``sources.md``). The verifier suite is a single
``citation-check`` stage; the safety barrier is ``NotClaimed``. Verifier
suites for source discovery, citation provenance, and claim extraction are
out of scope for the skeleton — the point is contract conformance.
