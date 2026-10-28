.. _example-python-etl:

Example: Python ETL Pipeline
=============================

This tutorial demonstrates building a complete Python ETL (Extract, Transform, Load)
pipeline using Perspt's agent mode.

Task Description
----------------

We will ask the agent to create a data pipeline that:

- Reads CSV files with validation
- Transforms data using Pydantic models
- Handles missing values and edge cases
- Includes comprehensive pytest tests

Running the Agent
-----------------

Initialize a ``uv`` project first so ``pyproject.toml`` exists before the
session starts:

.. code-block:: bash

   export GEMINI_API_KEY="your-key"
   uv init --lib /tmp/etl-demo

   perspt agent --yes --allow-dependency-mutation -w /tmp/etl-demo \
     --actuator-model gemini::gemini-3.1-pro \
     --explorer-model gemini::gemini-3.5-flash \
     "Create a Python ETL pipeline package. It should:
      1. Read CSV files into Pydantic models for validation
      2. Transform records (clean nulls, normalize strings, compute derived fields)
      3. Write validated records to a new CSV
      4. Include a CLI entry point in src/main.py
      5. Use uv for dependency management with pandas and pydantic
      6. Include comprehensive pytest tests for each module"

Example output layout
---------------------

A successful run may produce a layout like this.
The exact files and test count depend on the generated implementation:

.. code-block:: text

   /tmp/etl-demo/
   +-- pyproject.toml          # [build-system] with uv_build backend
   +-- uv.lock
   +-- src/
   |   +-- etl_pipeline/
   |   |   +-- __init__.py
   |   |   +-- core.py         # Main pipeline logic
   |   |   +-- validator.py    # Pydantic models
   |   |   +-- transformer.py  # Data transformations
   |   +-- main.py             # CLI entry point
   +-- tests/
       +-- test_core.py
       +-- test_validator.py
       +-- test_transformer.py

Verification
------------

The engine verifies every checkpoint with real sensors: ``ty check`` (or
``python -m compileall`` / ``pyright`` fallback) and offline ``pytest`` measure
the energy of each candidate, and a checkpoint
is accepted on a hard pass or sufficient measured descent.
Only a hard-passing result can be delivered. When the run finishes, the terminal summary
reports the outcome, the session id, turns used, the ledger head hash,
and the promoted paths.

After completion, verify manually:

.. code-block:: bash

   cd /tmp/etl-demo
   uv run pytest -v

Key Observations
----------------

- Initializing with ``uv init --lib`` creates a ``src/`` layout with
  ``[build-system]`` in ``pyproject.toml``, and ``--allow-dependency-mutation``
  lets the agent run governed ``uv add`` commands for ``pandas``, ``pydantic``,
  and ``pytest``
- The dispatcher prevents nodes with conflicting write footprints from running concurrently
- ``ty check`` (with ``compileall`` / ``pyright`` fallback) checks syntax and types (``V_syn``)
- ``uv run --no-sync pytest`` runs the test suite offline inside the sandbox (``V_log``)
- Work is promoted only after the run's final checkpoint passes every
  hard predicate (a ``HardPass`` outcome)

See Also
--------

- :doc:`agent-mode` - Agent mode tutorial
- :doc:`example-rust-cli` - Rust project example
