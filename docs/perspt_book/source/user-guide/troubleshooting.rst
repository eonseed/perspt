.. _user-guide-troubleshooting:

Troubleshooting
===============

API Key Issues
--------------

**Symptom:** "No API key found" error.

**Solutions:**

1. Check env var is exported: ``echo $GEMINI_API_KEY``
2. Verify spelling: ``ANTHROPIC_API_KEY`` (not ``ANTHROPIC_KEY``)
3. Pass explicitly: ``perspt chat --api-key "your-key"``
4. Check config file: ``~/.config/perspt/config.json``


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
3. Lower stability threshold: ``--stability-threshold 0.5``
4. Use ``--defer-tests`` to skip V_log during coding
5. Check ``perspt status`` for escalation details

**High energy despite clean code:**

1. Run tests manually: ``uv run pytest -v`` or ``cargo test``
2. Check for LSP diagnostics: ``ty check .``
3. Adjust energy weights: ``--energy-weights "0.5,0.5,1.0"``
4. Verify contract compliance

**Plugin not detected:**

1. Ensure required binaries are installed in PATH
2. Check workspace has expected marker files (``Cargo.toml``, ``pyproject.toml``)
3. Run ``perspt status`` to see active plugins


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

When tool binaries (``ty``, ``cargo``, ``pytest``) are missing, the agent enters
**degraded verification mode**:

- V_syn is estimated via heuristic pattern matching (regex-based)
- V_log uses ``exit 0`` stubs
- V_boot skips missing commands

This allows the agent to function, but with lower verification confidence.

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


Getting Help
------------

.. code-block:: bash

   perspt --help
   perspt agent --help
   perspt chat --help

For more details, see:

- :doc:`../reference/cli-reference` — Full CLI reference
- :doc:`../reference/troubleshooting` — Advanced troubleshooting
