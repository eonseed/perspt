.. _psp-process:

PSP: Perspt Specification Proposals
===================================

PSP (Perspt Specification Proposals) is the process by which Perspt designs and implements 
new features in a structured, reviewable manner.

Overview
--------

PSPs are design documents that describe proposed features, architectural changes, or 
process improvements for Perspt. They are inspired by PEPs (Python Enhancement Proposals) 
and RFCs (Request for Comments).

.. graphviz::
   :align: center
   :caption: PSP Lifecycle

   digraph psp {
       rankdir=LR;
       node [shape=box, style="rounded,filled", fontname="Helvetica", fontsize=10];
       
       draft [label="Draft", fillcolor="#E3F2FD"];
       review [label="Under Review", fillcolor="#FFF3E0"];
       accepted [label="Accepted", fillcolor="#E8F5E9"];
       implemented [label="Implemented", fillcolor="#C8E6C9"];
       rejected [label="Rejected", fillcolor="#FFCDD2"];
       
       draft -> review;
       review -> accepted;
       review -> rejected;
       accepted -> implemented;
   }

PSP Format
----------

Each PSP follows a standard format in reStructuredText:

.. code-block:: rst

   :PSP: 000N
   :Title: Feature Name
   :Author: Name <email@example.com>
   :Status: Draft | Under Review | Accepted | Implemented | Rejected
   :Created: YYYY-MM-DD
   :Updated: YYYY-MM-DD

   Abstract
   --------
   One-paragraph summary.

   Motivation
   ----------
   Why is this needed?

   Specification
   -------------
   Technical details.

   Implementation
   --------------
   How will it be implemented?

   Rationale
   ---------
   Why this approach?

   Reference Implementation
   ------------------------
   Links to code.

Current PSPs
------------

.. list-table::
   :header-rows: 1
   :widths: 10 40 20 30

   * - PSP
     - Title
     - Status
     - Description
   * - PSP-0001
     - PSP Process
     - Implemented
     - This document
   * - PSP-0002
     - Multi-Provider Architecture
     - Implemented
     - GenAI-based unified provider
   * - PSP-0003
     - Configuration System
     - Implemented
     - JSON config with env vars
   * - PSP-0004
     - SRBN Agent Mode
     - Implemented
     - Stabilized Recursive Barrier Network

PSP-0004: SRBN Agent Mode
-------------------------

The most significant PSP, introducing autonomous coding capabilities.

**Key Components**:

1. **Orchestrator** — SRBN control loop
2. **Lyapunov Energy** — Stability measurement (V_syn, V_str, V_log)
3. **Model Tiers** — Architect, Actuator, Verifier, Speculator
4. **Retry Policy** — Bounded retries with escalation
5. **Merkle Ledger** — Change tracking with rollback

**State Machine**:

.. graphviz::
   :align: center

   digraph psp4 {
       rankdir=TB;
       node [shape=box, style="rounded,filled", fontname="Helvetica", fontsize=10];
       
       idle [label="Idle", fillcolor="#E3F2FD"];
       sheaf [label="Sheafifying", fillcolor="#FFF3E0"];
       spec [label="Speculating", fillcolor="#E8F5E9"];
       verify [label="Verifying", fillcolor="#F3E5F5"];
       review [label="Awaiting Review", fillcolor="#FFECB3"];
       commit [label="Committing", fillcolor="#C8E6C9"];
       failed [label="Failed", fillcolor="#FFCDD2"];
       
       idle -> sheaf [label="task received"];
       sheaf -> spec [label="plan ready"];
       sheaf -> failed [label="sheaf fail (3x)"];
       spec -> verify [label="code generated"];
       verify -> spec [label="V(x) > ε"];
       verify -> review [label="manual approval"];
       review -> commit [label="approved"];
       review -> spec [label="rejected, retry"];
       review -> failed [label="rejected (3x)"];
       verify -> commit [label="V(x) ≤ ε"];
       commit -> idle [label="done"];
   }

Creating a PSP
--------------

1. **Assign Number**: Get the next PSP number from the maintainers
2. **Create File**: ``docs/psps/source/psp-00000N.rst``
3. **Write Draft**: Follow the template format
4. **Submit PR**: Open a pull request with the PSP
5. **Review**: Gather feedback and iterate
6. **Accept/Reject**: Maintainers decide on the proposal

PSP Repository
--------------

All PSPs are stored in the ``docs/psps/`` directory:

.. code-block:: text

   docs/psps/
   ├── source/
   │   ├── psp-000001.rst  # PSP Process
   │   ├── psp-000002.rst  # Multi-Provider
   │   ├── psp-000003.rst  # Configuration
   │   └── psp-000004.rst  # SRBN Agent Mode
   └── Makefile

See Also
--------

- `PSP Repository <https://github.com/eonseed/perspt/tree/master/docs/psps>`_
- :doc:`srbn-architecture` - Technical details of SRBN
- :doc:`../developer-guide/contributing` - How to contribute
