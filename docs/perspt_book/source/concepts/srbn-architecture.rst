.. _srbn-architecture:

SRBN Architecture
=================

The **Stabilized Recursive Barrier Network (SRBN)** is Perspt's autonomous
coding engine, implementing the Flow Barrier Control (FBC) theory from PSP-000004.

Overview
--------

SRBN treats code generation as a **dynamical system** that must be stabilized.
The goal is to drive the system to a state where ``V(x) < ε`` (energy below threshold).

.. code-block:: text

   ┌─────────────┐
   │  PLANNING   │ ← Architect decomposes task into TaskPlan (JSON)
   └──────┬──────┘
          │
   ┌──────▼──────┐
   │   CODING    │ ← Actuator generates code for each sub-task
   └──────┬──────┘
          │
   ┌──────▼──────┐
   │  VERIFYING  │ ← LSP diagnostics compute V(x) energy
   └──────┬──────┘
          │
          ▼
      V(x) < ε? ───No──→ RETRY (with error feedback)
          │
         Yes
          │
   ┌──────▼──────┐
   │ COMMITTING  │ ← Merkle Ledger records changes
   └─────────────┘

Energy Model
------------

The total Lyapunov energy is:

.. math::

   V(x) = \alpha \cdot V_{syn} + \beta \cdot V_{str} + \gamma \cdot V_{log}

.. list-table:: Energy Components
   :widths: 15 50 15 20
   :header-rows: 1

   * - Symbol
     - Source
     - Weight
     - Calculation
   * - V_syn
     - LSP diagnostics
     - α = 1.0
     - errors × 1.0 + warnings × 0.1
   * - V_str
     - Structural analysis
     - β = 0.5
     - (placeholder)
   * - V_log
     - Test failures
     - γ = 2.0
     - Σ(criticality_weight)

**Stability Condition**: The system is stable when ``V(x) < 0.1`` (default ε).

Retry Policy
------------

Different error types have different retry limits per PSP-000004:

.. list-table::
   :widths: 30 20 50
   :header-rows: 1

   * - Error Type
     - Max Retries
     - Then
   * - Compilation/type errors
     - 3
     - Escalate to user
   * - Tool failures
     - 5
     - Escalate to user
   * - Review rejections
     - 3
     - Escalate to user

Workspace Crates
----------------

Perspt is organized as a Cargo workspace with specialized crates:

.. list-table::
   :widths: 25 75
   :header-rows: 1

   * - Crate
     - Purpose
   * - ``perspt-cli``
     - CLI entry point, argument parsing, mode routing
   * - ``perspt-core``
     - Configuration, LLM provider abstraction (genai crate)
   * - ``perspt-tui``
     - Terminal UI (Ratatui), Agent dashboard, Task tree
   * - ``perspt-agent``
     - SRBN orchestrator, tools, LSP client, test runner
   * - ``perspt-policy``
     - Security sandbox, file path validation
   * - ``perspt-sandbox``
     - Process isolation (future: WASM/containers)

Key Components
--------------

SRBNOrchestrator
~~~~~~~~~~~~~~~~

The main control loop in ``perspt-agent/src/orchestrator.rs``:

.. code-block:: rust

   impl SRBNOrchestrator {
       pub async fn execute_task(&mut self, task: &str) -> Result<()> {
           // 1. Sheafification: Get plan from Architect
           let plan = self.architect.plan(task).await?;
           
           // 2. For each node in plan
           for node in plan.nodes {
               // 3. Speculation: Generate code
               let code = self.actuator.generate(&node).await?;
               
               // 4. Verification: Compute V(x)
               let energy = self.stability_monitor.compute_energy()?;
               
               // 5. Convergence loop
               while energy > self.epsilon {
                   let code = self.actuator.fix_errors(&diagnostics).await?;
                   energy = self.stability_monitor.compute_energy()?;
               }
               
               // 6. Commit to ledger
               self.ledger.record(&node, &code)?;
           }
       }
   }

LspClient
~~~~~~~~~

Python type checking via ``ty`` in ``perspt-agent/src/lsp.rs``:

- Spawns ``ty server`` subprocess
- Sends ``textDocument/didOpen`` on file write
- Polls ``textDocument/diagnostic`` for errors
- Extracts error messages for LLM correction prompts

PythonTestRunner
~~~~~~~~~~~~~~~~

pytest execution in ``perspt-agent/src/test_runner.rs``:

- Creates ``pyproject.toml`` if missing
- Runs ``uv sync --dev`` to install pytest
- Executes ``uv run pytest --tb=short``
- Parses failures and computes V_log

Extending Perspt
----------------

Adding a New Tool
~~~~~~~~~~~~~~~~~

1. Add tool struct in ``perspt-agent/src/tools.rs``
2. Implement the tool's ``execute`` method
3. Register in ``AgentTools::new()``
4. Add to Architect's tool list in prompts

Adding a New LSP Server
~~~~~~~~~~~~~~~~~~~~~~~

1. Add language detection in ``LspClient::detect_language()``
2. Configure server spawn command
3. Map diagnostic codes to error severity

See Also
--------

- :doc:`/tutorials/agent-mode` - Hands-on Agent Mode tutorial
- :doc:`/howto/agent-options` - CLI options reference
- `PSP-000004 <https://github.com/eonseed/perspt/blob/master/docs/psps/source/psp-000004.rst>`_ - Full specification
