.. _tutorial-headless-mode:

Headless Mode
=============

Run Perspt's experimental SRBN agent without interactive prompts. This is designed
for CI/CD pipelines, batch code generation, and automated workflows.

Overview
--------

In headless mode (``--yes`` flag), the agent auto-approves all changes, skipping
the interactive review modal. Combined with ``--workdir`` and ``--defer-tests``,
it enables fully autonomous project generation.

.. code-block:: bash

   perspt agent --yes -w /tmp/output "Create a Python ETL pipeline"

When to Use Headless Mode
-------------------------

- **CI/CD pipelines** — Generate boilerplate or scaffold projects in automation
- **Batch processing** — Run multiple agent tasks in sequence from a script
- **Rapid prototyping** — Skip review when iterating quickly
- **Testing the agent** — Validate agent behavior without manual intervention

When NOT to use headless mode:

- **Production codebases** — Always review changes before committing
- **Security-sensitive projects** — Manual review catches policy violations
- **Learning** — Interactive mode teaches you how SRBN works


Basic Headless Run
------------------

.. code-block:: bash

   export GEMINI_API_KEY="your-key"

   # Create a project autonomously
   perspt agent --yes -w /tmp/my-project \
     "Create a Python data validation library using Pydantic.
      Include src layout, pyproject.toml, and pytest tests."

The agent will:

1. Detect language plugins
2. Plan the task DAG
3. Execute all nodes, auto-approving each
4. Run verification (LSP + tests) on each node
5. Commit stable nodes to the ledger
6. Print a summary


Key Flags
---------

.. list-table::
   :header-rows: 1
   :widths: 30 70

   * - Flag
     - Description
   * - ``--yes`` / ``-y``
     - Auto-approve all changes (headless mode)
   * - ``-w, --workdir <DIR>``
     - Working directory for the project
   * - ``--defer-tests``
     - Skip V_log during node coding; only run tests at sheaf validation
   * - ``--max-cost <USD>``
     - Safety limit on total LLM spend
   * - ``--max-steps <N>``
     - Safety limit on total iterations
   * - ``--log-llm``
     - Log all LLM requests to DuckDB for post-analysis
   * - ``--single-file``
     - Force single-file mode (no DAG planning)
   * - ``--output-plan <FILE>``
     - Export the task plan as JSON before execution


Deferred Tests
--------------

By default, the agent runs tests (V_log) during each node's verification. With
``--defer-tests``, V_log is set to 0.0 during node coding and tests only run at
the final sheaf validation stage. This speeds up iteration:

.. code-block:: bash

   perspt agent --yes --defer-tests -w /tmp/fast \
     "Build a CLI tool in Rust that converts CSV to JSON"

.. admonition:: Trade-off
   :class: warning

   Deferred tests mean individual nodes may converge with untested code. The sheaf
   validation stage catches integration issues, but per-node test failures are
   discovered later.


Reading Structured Progress
---------------------------

In headless mode, Perspt emits structured progress to stderr:

.. code-block:: text

   [detect] Workspace: greenfield
   [detect] Plugin: python (LSP: ty, tests: pytest, init: uv init --lib)
   [plan] TaskPlan: 5 nodes, 1 plugin
   [node-1] State: Generating -> Verifying -> Stable (V=0.00)
   [node-2] State: Generating -> Verifying -> Stable (V=0.00)
   [node-3] State: Generating -> Verifying -> Retry (V=2.50)
   [node-3] State: Generating -> Verifying -> Stable (V=0.00)
   [sheaf] 3 validators, 0 failures, V_sheaf=0.00
   [done] 5/5 nodes completed, session abc123


Checking Session Status
-----------------------

After a headless run, inspect the results:

.. code-block:: bash

   # Session status
   perspt status

   # Recent ledger entries
   perspt ledger --recent

   # LLM usage statistics (if --log-llm was used)
   perspt logs --stats

   # Resume a failed session
   perspt resume --last


Scripting Multiple Tasks
------------------------

Run multiple agent tasks from a shell script:

.. code-block:: bash

   #!/bin/bash
   set -e
   export GEMINI_API_KEY="your-key"

   tasks=(
     "Create a Python CSV parser library"
     "Create a Rust JSON validator CLI"
     "Create a Python REST API with FastAPI"
   )

   for i in "${!tasks[@]}"; do
     dir="/tmp/project-$i"
     mkdir -p "$dir"
     perspt agent --yes --max-cost 2.0 -w "$dir" "${tasks[$i]}"
     echo "=== Project $i complete ==="
   done


Safety Recommendations
----------------------

1. **Always set cost limits** — ``--max-cost 5.0`` prevents runaway spending
2. **Use disposable directories** — Point ``-w`` to a fresh directory
3. **Review after generation** — Inspect the output before using it in production
4. **Use --log-llm** — Enables post-run analysis of what the agent did
5. **Set --max-steps** — Bounds the total number of retries across all nodes


See Also
--------

- :doc:`agent-mode` — Interactive agent mode tutorial
- :doc:`../concepts/srbn-architecture` — SRBN technical details
- :doc:`../howto/agent-options` — Full agent CLI reference
