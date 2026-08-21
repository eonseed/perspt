.. _tutorial-headless-mode:

Headless Mode
=============

Run Perspt's experimental SRBN agent without interactive prompts. This is designed
for CI/CD pipelines, batch code generation, and automated workflows.

Overview
--------

In headless mode (``--yes`` flag), the agent auto-approves all changes, skipping
the interactive review modal. Combined with ``--workdir``, it enables fully
autonomous project generation.

.. code-block:: bash

   perspt agent --yes -w /tmp/output "Create a Python ETL pipeline"

When to Use Headless Mode
-------------------------

- **CI/CD pipelines** - Generate boilerplate or scaffold projects in automation
- **Batch processing** - Run multiple agent tasks in sequence from a script
- **Rapid prototyping** - Skip review when iterating quickly
- **Testing the agent** - Validate agent behavior without manual intervention

When NOT to use headless mode:

- **Production codebases** - Always review changes before committing
- **Security-sensitive projects** - Manual review catches policy violations
- **Learning** - Interactive mode teaches you how SRBN works


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
2. Plan the work graph — a single node by default; with
   ``--max-parallel-nodes`` above 1, one governed architect turn may
   decompose the task
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
   * - ``--max-turns <N>``
     - Maximum model turns per node (default 12)
   * - ``--max-calls-per-turn <N>``
     - Maximum model-issued and nested tool calls per turn (default 8)
   * - ``--rejection-budget <N>``
     - Shared non-descending and recovery budget (default 4)
   * - ``--output-summary <FILE>``
     - Write the terminal session summary as JSON

.. admonition:: Headless requirements
   :class: note

   A run without a terminal (CI, piped output) requires ``--yes``:
   promotion approval cannot be prompted without a terminal, so the run
   fails fast instead of silently escalating. ``--max-parallel-nodes``
   above 1 also requires ``--yes``. A run that ends in escalation or
   incomplete work exits with a nonzero status, so CI never reads a
   stopped node as success.


Reading Structured Progress
---------------------------

In headless mode, Perspt prints its startup banner (task, workspace,
detected domain), then streams ledger narration lines and per-node status
changes as the run progresses. When the run finishes it prints the
outcome, session id, turns used, ledger head hash, and any promoted
paths.

For a machine-readable record, pass ``--output-summary``:

.. code-block:: bash

   perspt agent --yes -w /tmp/out --output-summary summary.json \
     "Build a CLI tool in Rust that converts CSV to JSON"

The JSON summary contains the session id, node id, outcome, turns used,
ledger head, and promoted paths.


Checking Session Status
-----------------------

After a headless run, inspect the results:

.. code-block:: bash

   # Session status
   perspt status

   # Recent ledger entries
   perspt ledger --recent

   # Ledger statistics
   perspt ledger --stats

   # Deterministic, credential-free replay of a session
   perspt replay <SESSION_ID>

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
     perspt agent --yes --max-turns 8 -w "$dir" "${tasks[$i]}"
     echo "=== Project $i complete ==="
   done


Safety Recommendations
----------------------

1. **Always set loop bounds** - ``--max-turns`` and ``--rejection-budget``
   bound runaway sessions
2. **Use disposable directories** - Point ``-w`` to a fresh directory
3. **Review after generation** - Inspect the output before using it in production
4. **Audit with the ledger** - ``perspt replay <SESSION_ID>`` reconstructs
   what the agent did, no logging flag required
5. **Check the exit status** - A nonzero exit means escalation or
   incomplete work, not verified success


See Also
--------

- :doc:`agent-mode` - Interactive agent mode tutorial
- :doc:`../concepts/srbn-architecture` - SRBN technical details
- :doc:`../howto/agent-options` - Full agent CLI reference
