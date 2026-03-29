.. _example-rust-cli:

Example: Rust CLI Tool
======================

Build a Rust command-line tool using Perspt's agent mode.

Task Description
----------------

We will ask the agent to create a CLI tool that converts CSV files to JSON.

Running the Agent
-----------------

.. code-block:: bash

   export GEMINI_API_KEY="your-key"

   perspt agent --yes -w /tmp/csv2json \
     --architect-model gemini-pro-latest \
     --actuator-model gemini-3.1-flash-lite-preview \
     "Create a Rust CLI tool called csv2json that:
      1. Reads a CSV file from a path argument
      2. Converts each row to a JSON object
      3. Outputs JSON array to stdout or a file (--output flag)
      4. Handles errors gracefully with anyhow
      5. Uses clap for argument parsing
      6. Includes unit tests and integration tests"

Expected Output
---------------

.. code-block:: text

   /tmp/csv2json/
   +-- Cargo.toml
   +-- src/
   |   +-- main.rs        # Entry point, clap args
   |   +-- lib.rs          # Core conversion logic
   |   +-- converter.rs    # CSV -> JSON conversion
   +-- tests/
       +-- integration.rs  # Integration tests

Verification
------------

The agent uses the ``rust`` plugin:

- **LSP**: ``rust-analyzer`` for V_syn
- **Tests**: ``cargo test`` for V_log
- **Init**: ``cargo init`` for V_boot

.. code-block:: bash

   cd /tmp/csv2json
   cargo test
   cargo run -- --help

Key Observations
----------------

- The ``rust`` plugin selects ``rust-analyzer`` as the LSP server
- ``cargo init`` bootstraps the project structure (V_boot)
- ``cargo test`` runs both unit and integration tests (V_log)
- Ownership closure assigns each source file to exactly one node

See Also
--------

- :doc:`agent-mode` — Agent mode tutorial
- :doc:`example-python-etl` — Python project example
