.. _reference-troubleshooting:

Advanced Troubleshooting
========================

Diagnostic Commands
-------------------

.. code-block:: bash

   # Session status with per-node details
   perspt status

   # LLM call log browser
   perspt logs --tui

   # Usage statistics
   perspt logs --stats

   # Ledger integrity check
   perspt ledger --stats

   # Enable verbose logging
   RUST_LOG=debug perspt simple-chat 2>debug.log


Common Error Patterns
---------------------

``ErrorType::ApiKeyMissing``
~~~~~~~~~~~~~~~~~~~~~~~~~~~~

No API key found for the selected provider. Set the environment variable or
pass ``--api-key``.


``ErrorType::ModelNotFound``
~~~~~~~~~~~~~~~~~~~~~~~~~~~~

The model identifier is not recognized by the provider. Use ``perspt --list-models``
to see available models.


``ErrorType::ConnectionFailed``
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Cannot reach the provider endpoint. Check:

- Internet connectivity
- Ollama service status (``ollama serve``)
- Firewall/proxy settings


``ErrorType::RateLimitExceeded``
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Provider rate limit hit. Wait and retry, or switch providers.


``ErrorType::BudgetExceeded``
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

LLM spend exceeded ``--max-cost`` limit. Increase the limit or reduce task scope.


Agent-Specific Issues
---------------------

**Ownership conflict:**

Two nodes attempted to write the same file. The Architect replans the DAG to
resolve the conflict. If this persists, simplify the task description.

**Sheaf validation failure:**

Cross-node contracts are incompatible. Common causes:

1. Inconsistent function signatures across modules
2. Missing imports between files
3. Type mismatches at module boundaries

The agent automatically retries with feedback from the sheaf validator.


**Degraded verification mode:**

Missing tool binaries cause fallback to heuristic verification:

.. list-table::
   :header-rows: 1
   :widths: 20 40 40

   * - Component
     - Full Mode
     - Degraded Mode
   * - V_syn
     - LSP diagnostics (ty, rust-analyzer)
     - Regex pattern matching
   * - V_log
     - Test runner (pytest, cargo test)
     - Exit code stubs
   * - V_boot
     - Init commands (uv init, cargo init)
     - Skipped

Install the required tools to restore full verification.


Terminal Restoration
--------------------

If Perspt crashes and leaves the terminal in raw mode:

.. code-block:: bash

   reset
   # or
   stty sane

Perspt installs a panic hook that attempts to restore the terminal automatically
(raw mode off, leave alternate screen). If this fails, ``reset`` will fix it.


Performance
-----------

**Slow response streaming:**

1. Check network latency to the provider
2. Try a faster model (e.g., ``gemini-3.1-flash-lite-preview``)
3. Use Groq for the fastest inference

**High memory usage in agent mode:**

1. Large DAGs with many nodes consume more memory
2. Use ``--single-file`` for simple tasks
3. Use ``--max-steps`` to bound total iterations
