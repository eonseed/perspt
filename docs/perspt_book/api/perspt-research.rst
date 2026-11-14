.. _api-perspt-research:

``perspt-research``
===================

The research domain package: a working non-coding domain proving the
``perspt-sdk`` contracts execute without forking the scheduler,
admissibility kernel, residual model, replay ledger, or dashboard event model.

Core Type
---------

**ResearchDomain** - The ``AgentDomainPackage`` implementation:

.. code-block:: rust

   pub struct ResearchDomain;

   impl AgentDomainPackage for ResearchDomain {
       fn domain_id(&self) -> DomainId;   // "research"
       // detect, residual_schema, energy_model, correction_directions,
       // correction_packet, evaluate_workspace, hard_gate_policy,
       // verifier_suite, safety_barrier
   }

Residual Mapping
----------------

Research reuses the SDK residual classes:

.. list-table::
   :header-rows: 1
   :widths: 35 65

   * - Residual Class
     - Research Meaning
   * - ``Syntax``
     - Malformed BibTeX entries or unbalanced braces
   * - ``TestFailure``
     - Unsupported / contradicted claim
   * - ``InterfaceMismatch``
     - Source mismatch
   * - ``ImportGraph``
     - Unresolved citation key (missing BibTeX or source entry)
   * - ``SymbolMismatch``
     - Mismatched citation label
   * - ``ContextDrift``
     - Stale evidence
   * - ``SheafInconsistency``
     - Cross-source contradiction

Detection looks for bibliography markers (``references.bib``, ``refs.bib``,
``bibliography.bib``, ``sources.md``, ``references.md``). Its ``evaluate_workspace``
verifier executes the ``citation-check`` stage over ``.bib``, source-index, and
Markdown/RST/LaTeX prose files, while ``correction_packet`` folds unresolved
citations and syntax errors into targeted correction prompts. The safety barrier
is ``NotClaimed``.
