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

.. code-block:: bash

   export GEMINI_API_KEY="your-key"

   perspt agent --yes -w /tmp/etl-demo \
     --architect-model gemini-pro-latest \
     --actuator-model gemini-3.1-flash-lite-preview \
     "Create a Python ETL pipeline package. It should:
      1. Read CSV files into Pydantic models for validation
      2. Transform records (clean nulls, normalize strings, compute derived fields)
      3. Write validated records to a new CSV
      4. Include a CLI entry point in src/main.py
      5. Use uv for dependency management with pandas and pydantic
      6. Include comprehensive pytest tests for each module"

Expected Output
---------------

The agent produces a project with ``src/`` layout:

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

The SRBN engine verifies each node:

.. code-block:: text

   [node-1] Initialize Python package (V_boot=0.00)
   [node-2] Create validator module (V_syn=0.00, V_log=0.00, V=0.00)
   [node-3] Create transformer module (V_syn=0.00, V_log=0.00, V=0.00)
   [node-4] Create core pipeline (V_syn=0.00, V_log=0.00, V=0.00)
   [node-5] Create CLI entry point (V_syn=0.00, V=0.00)
   [node-6] Write pytest tests (V_log=0.00, V=0.00)
   [node-7] Integration wiring (V_sheaf=0.00)

After completion, verify manually:

.. code-block:: bash

   cd /tmp/etl-demo
   uv run pytest -v
   # Expected: 15-20 tests passing

Key Observations
----------------

- The agent uses ``uv init --lib`` to create proper ``src/`` layout with
  ``[build-system]`` in ``pyproject.toml``
- Ownership closure ensures no two nodes write to the same file
- The ``ty`` LSP server provides real-time type checking (V_syn)
- pytest provides test verification (V_log)
- All 7 SRBN nodes converge at V(x) = 0.00

See Also
--------

- :doc:`agent-mode` — Agent mode tutorial
- :doc:`example-rust-cli` — Rust project example
