.. _srbn-architecture:

SRBN Architecture
=================

The **Stabilized Recursive Barrier Network (SRBN)** is Perspt's core innovation for 
autonomous coding with mathematically guaranteed stability.

Overview
--------

SRBN ensures that AI-generated code converges to a stable state before being committed,
using concepts from control theory (Lyapunov stability) and software verification (LSP, tests).

.. graphviz::
   :align: center
   :caption: SRBN Architecture

   digraph srbn {
       rankdir=TB;
       compound=true;
       node [shape=box, style="rounded,filled", fontname="Helvetica", fontsize=10];
       
       subgraph cluster_models {
           label="Model Tiers";
           style=dashed;
           arch [label="Architect\n(Deep Reasoning)", fillcolor="#E8F5E9"];
           act [label="Actuator\n(Code Generation)", fillcolor="#E3F2FD"];
           ver [label="Verifier\n(Stability Check)", fillcolor="#F3E5F5"];
           spec [label="Speculator\n(Fast Lookahead)", fillcolor="#FFF3E0"];
       }
       
       subgraph cluster_barriers {
           label="Stability Barriers";
           style=dashed;
           lsp [label="LSP\n(V_syn)", fillcolor="#FFECB3"];
           tests [label="Tests\n(V_log)", fillcolor="#FFECB3"];
           struct [label="Structure\n(V_str)", fillcolor="#FFECB3"];
       }
       
       subgraph cluster_output {
           label="Output";
           style=dashed;
           ledger [label="Merkle Ledger", fillcolor="#C8E6C9"];
       }
       
       arch -> act;
       act -> lsp;
       act -> tests;
       act -> struct;
       lsp -> ver;
       tests -> ver;
       struct -> ver;
       ver -> act [label="retry", style=dashed];
       ver -> ledger [label="stable"];
   }

The Control Loop
----------------

The SRBN control loop executes 5 phases for each task:

.. list-table::
   :header-rows: 1
   :widths: 5 20 75

   * - #
     - Phase
     - Description
   * - 1
     - **Sheafification**
     - Architect model decomposes task into a JSON ``TaskPlan`` with dependency graph
   * - 2
     - **Speculation**
     - Actuator model generates code for each node with tool calls (write_file, etc.)
   * - 3
     - **Verification**
     - Compute Lyapunov Energy V(x) from LSP diagnostics, structure, and tests
   * - 4
     - **Convergence**
     - If V(x) > ε, retry with error feedback; otherwise proceed
   * - 5
     - **Commit**
     - Record changes in Merkle Ledger with cryptographic integrity

Lyapunov Energy
---------------

The stability of generated code is measured using a Lyapunov energy function:

.. admonition:: Energy Formula
   :class: important

   **V(x) = α·V_syn + β·V_str + γ·V_log**

   Default weights: α = 1.0, β = 0.5, γ = 2.0

Components
~~~~~~~~~~

.. list-table::
   :header-rows: 1
   :widths: 15 25 60

   * - Component
     - Source
     - Description
   * - **V_syn**
     - LSP Diagnostics
     - Count of errors and warnings from ``ty`` (Python type checker)
   * - **V_str**
     - Structural Analysis
     - Code complexity, dead code, pattern violations
   * - **V_log**
     - Test Failures
     - Weighted sum of pytest failures (critical tests have higher weight)

Convergence Criterion
~~~~~~~~~~~~~~~~~~~~~

The system is considered stable when:

.. math::

   V(x) \leq \varepsilon

Default: ε = 0.1

Model Tiers
-----------

SRBN uses multiple specialized models:

.. list-table::
   :header-rows: 1
   :widths: 20 30 50

   * - Tier
     - Purpose
     - Recommended Model
   * - **Architect**
     - Deep reasoning, task decomposition
     - GPT-5.2, Claude Opus 4.5
   * - **Actuator**
     - Code generation, tool calls
     - Claude Opus 4.5, GPT-5.2
   * - **Verifier**
     - Stability analysis
     - Gemini 3 Pro
   * - **Speculator**
     - Fast lookahead, branch prediction
     - Gemini 3 Flash, Groq Llama

Configure model tiers via CLI:

.. code-block:: bash

   perspt agent \
     --architect-model gpt-5.2 \
     --actuator-model claude-opus-4.5 \
     --verifier-model gemini-3-pro \
     --speculator-model gemini-3-flash \
     "Build a REST API"

Retry Policy
------------

SRBN implements bounded retries per PSP-0004:

.. list-table::
   :header-rows: 1
   :widths: 30 20 50

   * - Error Type
     - Max Retries
     - Escalation
   * - Compilation errors (LSP)
     - 3
     - Escalate to user with context
   * - Tool failures (file ops)
     - 5
     - Escalate with error logs
   * - Review rejections
     - 3
     - Escalate with diff summary

TaskPlan Structure
------------------

The Architect generates a JSON TaskPlan:

.. code-block:: json

   {
     "nodes": [
       {
         "id": 1,
         "description": "Create Calculator class",
         "type": "create",
         "files": ["calculator.py"],
         "dependencies": []
       },
       {
         "id": 2,
         "description": "Add arithmetic methods",
         "type": "modify",
         "files": ["calculator.py"],
         "dependencies": [1]
       },
       {
         "id": 3,
         "description": "Write unit tests",
         "type": "create",
         "files": ["test_calculator.py"],
         "dependencies": [2]
       }
     ]
   }

Merkle Ledger
-------------

All changes are recorded in a Merkle tree for:

- **Integrity** — Cryptographic verification of change history
- **Rollback** — Revert to any previous state
- **Audit** — Complete trail of AI-generated changes

.. code-block:: bash

   # View recent commits
   perspt ledger --recent

   # Rollback to commit
   perspt ledger --rollback abc123

   # Statistics
   perspt ledger --stats

Provisional Branches
--------------------

When SRBN speculates on child nodes before the parent is fully committed, it uses
**provisional branches** to isolate speculative work from the committed ledger state.

.. admonition:: Key Invariant
   :class: important

   Provisional work is **never** merged into the global ledger until the parent node
   meets the stability threshold. If the parent fails, all dependent branches are flushed.

Branch Lifecycle
~~~~~~~~~~~~~~~~

Each provisional branch tracks state through four stages:

.. list-table::
   :header-rows: 1
   :widths: 20 80

   * - State
     - Description
   * - **Active**
     - Branch is open, speculative work is in progress
   * - **Sealed**
     - Parent interface is sealed; children may proceed
   * - **Merged**
     - Parent committed; branch work is merged into the global ledger
   * - **Flushed**
     - Parent failed; branch work is discarded and may be replayed later

Interface Seal Prerequisites
~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Downstream speculation requires that the parent node's public interface be sealed and
hashed. This prevents children from coding against unstable signatures:

1. Parent node reaches **Commit** phase
2. If the node is an **Interface** class, its structural digest is sealed
3. ``InterfaceSealed`` event is emitted with sealed paths and hash
4. Blocked dependents are released and may begin speculation
5. Sealed digests are injected into child restriction maps for compilation

Flush Cascade
~~~~~~~~~~~~~

When a parent node's verification fails:

1. The parent's provisional branch is flushed
2. ``collect_descendants`` walks the DAG to find all transitive children
3. Each descendant branch is flushed recursively
4. ``BranchFlushed`` event is emitted with the reason and affected branch IDs
5. Surviving branch state may be replayed after the parent is repaired

Sandboxed Verification
~~~~~~~~~~~~~~~~~~~~~~

Provisional branches execute verification against isolated sandbox workspaces:

- ``create_sandbox()`` creates a temporary directory for the branch
- ``copy_to_sandbox()`` copies workspace files into the sandbox
- ``cleanup_sandbox()`` removes a single sandbox after branch completion
- ``cleanup_session_sandboxes()`` removes all session sandboxes at shutdown

See Also
--------

- :doc:`psp-process` - The PSP-0004 specification
- :doc:`../api/perspt-agent` - API documentation
- :doc:`../tutorials/agent-mode` - Tutorial walkthrough
