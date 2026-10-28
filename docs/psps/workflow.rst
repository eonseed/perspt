PSP Process and Workflow
========================

Perspt Specification Proposals (PSPs) provide a structured, reproducible mechanism for proposing significant changes, collecting architectural review, and maintaining a permanent historical record of Perspt's evolution. Routine bug fixes and minor internal refactors go through normal GitHub pull requests without a PSP.

Catalog Management with Guidedog
--------------------------------

The PSP catalog is managed natively in Rust using ``guidedog dsc`` and configured by ``docs/psps/discussion.toml`` (prefix ``psp``, 6-digit zero-padded numbering starting at ``000001``).

.. code-block:: sh

   # Inspect effective PSP catalog configuration
   guidedog dsc config --dir=docs/psps

   # Create a new draft PSP from template/rfc-template.rst
   guidedog dsc new "Capability Delegation Lease Protocol" --labels=sdk,capabilities --dir=docs/psps

   # List all PSPs and their lifecycle states
   guidedog dsc list --dir=docs/psps

   # Validate all PSP headers, state transitions, and section invariants
   guidedog dsc check --dir=docs/psps

   # Regenerate registry.rst and bundle.rst indices
   guidedog dsc index --dir=docs/psps

   # Maintainer decision: assign a number and keep the proposal in draft
   guidedog dsc promote capability-delegation-lease-protocol --dir=docs/psps

   # Separate maintainer decision: open community discussion
   guidedog dsc state <number> --to=discussion --reason="Ready for review" --dir=docs/psps

   # Transition a PSP state with an explicit audit reason
   guidedog dsc state <number> --to=final --reason="All conformance gates W-AF passed" --evidence="cargo test --workspace" --dir=docs/psps

   # Build the HTML catalog and per-record Typst PDFs
   guidedog dsc build --dir=docs/psps

Maintainers control the lifecycle
---------------------------------

New proposals are unnumbered authoring drafts in state ``unassigned``.
After review, a maintainer uses ``promote`` to assign a number; the proposal becomes a numbered ``draft``.
Opening discussion, accepting a design, and marking an implementation final are separate decisions made with ``state``.
We submit the resulting metadata and registry changes through reviewed pull requests.
Merging a draft does not promote it, and GitHub Actions never changes lifecycle metadata.

When to Write a PSP
-------------------

Create a PSP when proposing:

* **SRBN Kernel & SDK Contracts:** Changes to Lyapunov energy equations, residual schemas, acceptance gates, admissibility predicates, or replay invariants.
* **Domain Packages & Verifier Suites:** New domain packages (such as coding or research) or changes to language-plugin verification and correction contracts.
* **CLI, TUI & Dashboard Surfaces:** Significant user-interface workflows, command-line flags, or observability projections.
* **Process & Governance:** Updates to engineering rules (such as PSP-1/2/3 complexity bounds), testing gates, or the PSP workflow itself.

PSP Lifecycle States
--------------------

Each record in ``docs/psps/records/`` carries a ``:psp-state:`` field governed by the state machine in ``discussion.toml``:

.. list-table:: PSP Lifecycle States
   :header-rows: 1
   :widths: 18 47 35

   * - State
     - Meaning
     - Allowed Next States
   * - ``unassigned``
     - Unnumbered authoring draft awaiting a maintainer-assigned number.
     - ``draft`` through maintainer ``promote``, or ``withdrawn``
   * - ``draft``
     - Numbered proposal under authoring and review.
     - ``discussion``, ``withdrawn``
   * - ``discussion``
     - Numbered and open for community and maintainer architectural review.
     - ``accepted``, ``active``, ``rejected``, ``withdrawn``
   * - ``accepted``
     - Maintainers have approved the specification for implementation.
     - ``final``, ``active``, ``withdrawn``
   * - ``active``
     - Living process or foundational runtime specification currently in force.
     - ``final``, ``withdrawn``
   * - ``final``
     - Implementation complete, verified by conformance tests, and merged.
     - none (terminal)
   * - ``rejected``
     - Proposal not adopted; retained with rationale for historical context.
     - none (terminal)
   * - ``withdrawn``
     - Superseded or withdrawn by the author; retained with pointer to replacement.
     - none (terminal)

PSP Types
---------

Every record declares its ``:psp-type:`` from the allowed set in ``discussion.toml``:

* ``process`` — Development workflow, governance, and meta-process rules (:doc:`records/000001-psp-purpose-and-guidelines`).
* ``feature`` — New user-facing capabilities or subsystems (:doc:`records/000003-simple-cli-mode`, :doc:`records/000006-real-time-web-dashboard`).
* ``enhancement`` — Architectural evolutions of existing Perspt subsystems (:doc:`records/000005-srbn-multi-file-coding-ux`, :doc:`records/000007-robust-typed-correction-loops`, :doc:`records/000008-srbn-agent-sdk-work-graph`, :doc:`records/000009-governed-agentic-platform`, :doc:`records/000010-bounded-search-trajectories`, :doc:`records/000011-domain-neutral-runtime-inversion`).
* ``ui-ux`` — Terminal user interface and interaction design (:doc:`records/000002-multi-line-input-support`).
* ``standards-track`` / ``api-change`` / ``informational`` — Formal protocol, API, or reference specifications.

Record Structure
----------------

Every PSP record starts from ``template/rfc-template.rst`` and begins with a leading reStructuredText field list containing:

* ``:psp-number:`` — 6-digit zero-padded number (or ``XXXXX`` prior to promotion)
* ``:psp-title:`` — Full title matching the top-level document heading
* ``:psp-state:`` — Current lifecycle state
* ``:psp-type:`` — Proposal type
* ``:psp-authors:`` — JSON array of author names
* ``:psp-created:`` — Creation date (``YYYY-MM-DD``)
* ``:psp-updated:`` — Last updated date (``YYYY-MM-DD``, ``>= created``)
* ``:psp-discussion:`` — Discussion URL or local review note
* ``:psp-labels:`` — JSON array of lowercase hyphenated topic labels

Every record includes top-level ``Abstract`` and ``References`` sections so that both HTML and Typst PDF outputs preserve self-contained scholarly provenance.
