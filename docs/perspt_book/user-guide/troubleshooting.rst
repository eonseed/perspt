.. _user-guide-troubleshooting:

Troubleshooting
===============

API Key Issues
--------------

**Symptom:** "No API key found" error.

**Solutions:**

1. Check env var is exported: ``echo $GEMINI_API_KEY``
2. Verify spelling: ``ANTHROPIC_API_KEY`` (not ``ANTHROPIC_KEY``)
3. Store it in the config: ``perspt config --set api_key="your-key"``
4. Check config file: ``~/.config/perspt/config.toml``


Connection Errors
-----------------

**Symptom:** "Connection refused" or timeouts.

**Solutions:**

1. Check internet connectivity
2. For Ollama: ensure ``ollama serve`` is running
3. Check firewall/proxy settings
4. Try a different provider


Agent Mode Issues
-----------------

**Agent stuck in retry loop:**

1. Check tool prerequisites: ``which uv``, ``which cargo``, ``which node``
2. Check LSP is functioning: ``ty check .`` or ``cargo check``
3. Relax the descent gate: ``--rho-gate 0.3``
4. Raise the retry allowance: ``--rejection-budget 8``
5. Check ``perspt status`` for gate decisions and denials

**High energy despite clean code:**

1. Run tests manually: ``uv run pytest -v`` or ``cargo test``
2. Check for LSP diagnostics: ``ty check .``
3. Bound runaway turns: ``--max-turns 8 --max-calls-per-turn 4``
4. Verify contract compliance

**Plugin not detected:**

1. Ensure required binaries are installed in PATH
2. Check workspace has expected marker files (``Cargo.toml``, ``pyproject.toml``)
3. Run ``perspt status`` to see ledger counters, last energy, and gate state


TUI Rendering Issues
--------------------

**Symptom:** Garbled output, incorrect colors.

**Solutions:**

1. Ensure terminal supports 256 colors: ``echo $TERM``
2. Try a different terminal emulator
3. Fallback to simple CLI: ``perspt simple-chat``
4. Check for conflicting terminal multiplexer settings


Degraded Verification
---------------------

When tool binaries (``ty``, ``cargo``, ``pytest``) are missing, checkpoints
may be flagged as **degraded** in the review modal, with the reasons listed
alongside the flag. The agent keeps working, but with lower verification
confidence.

To restore full verification, install the required tools:

.. code-block:: bash

   # Python projects
   pip install ty pytest

   # Rust projects
   rustup component add rust-analyzer

   # Node.js projects
   npm install -g typescript


Session Recovery
----------------

If a session is interrupted:

.. code-block:: bash

   # Check what's in progress
   perspt status

   # Resume the last session (shows trust context first)
   perspt resume --last

   # Or abort and start fresh
   perspt abort

If the session store itself refuses to open because of a poisoned DuckDB
write-ahead log, back it up and quarantine it (the WAL is never deleted):

.. code-block:: bash

   perspt db repair --db-path <path-to-db> --discard-wal


Getting Help
------------

.. code-block:: bash

   perspt --help
   perspt agent --help
   perspt chat --help

For more details, see:

- :doc:`../reference/cli-reference` - Full CLI reference
- :doc:`../reference/troubleshooting` - Advanced troubleshooting
