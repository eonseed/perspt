.. _reference-troubleshooting:

Advanced Troubleshooting
========================

Diagnostic Commands
-------------------

.. code-block:: bash

   # Session status with per-node details
   perspt status

   # Deterministic, credential-free audit replay of a session
   perspt replay <SESSION_ID>

   # Ledger integrity check
   perspt ledger --stats

   # Enable verbose logging
   RUST_LOG=debug perspt simple-chat 2>debug.log


Common Error Patterns
---------------------

Missing API key
~~~~~~~~~~~~~~~

When no provider is configured, Perspt detects one from environment keys in
a fixed order: Vertex AI project settings first, then ``GEMINI_API_KEY``,
``OPENAI_API_KEY``, ``ANTHROPIC_API_KEY``, ``GROQ_API_KEY``,
``COHERE_API_KEY``, ``XAI_API_KEY``, and ``DEEPSEEK_API_KEY``, falling back
to a local Ollama setup when none are present. Set the provider environment
variable or store a key with ``perspt config --set api_key=<KEY>``.


``Unsupported provider``
~~~~~~~~~~~~~~~~~~~~~~~~

The configured ``provider`` is not a recognized adapter. Supported values:
``openai``, ``anthropic``, ``gemini`` (or ``google``), ``vertex``,
``groq``, ``cohere``, ``ollama``, ``xai``, ``deepseek``.


Provider failover
~~~~~~~~~~~~~~~~~

When a model route fails, the agent fails over to the next configured
fallback route. The failover is charged to the shared recovery pool, so
repeated provider failures can exhaust the session's rejection budget.


Poisoned DuckDB WAL
~~~~~~~~~~~~~~~~~~~

If the local DuckDB store fails to open because of a poisoned WAL, run
``perspt db repair --db-path <PATH> [--discard-wal]``. The repair makes
durable backups first and quarantines the WAL; the WAL is never deleted.


Agent-Specific Issues
---------------------

**Footprint conflict:**

Two nodes declared overlapping file footprints. The dispatcher's scheduler
never runs conflicting nodes concurrently — the later node waits for the
conflicting node to finish. If this persists, simplify the task description.

**Global integration failure:**

Two nodes may pass separately and fail together. Node winners enter a
content-addressed staging root instead of the user workspace, and one
global verifier gate runs the full domain suite and the immutable test
oracle on the combined state. Only a hard-passing integration root is
promoted, atomically; on failure the prior staging root is restored and the
user workspace is left byte-identical.


**Degraded verification:**

When some checks could not run, the review modal marks a node's stability
metrics as degraded and lists the reasons. Address the listed reasons to
restore full verification.


Terminal Restoration
--------------------

If Perspt crashes and leaves the terminal in raw mode:

.. code-block:: bash

   reset
   # or
   stty sane

Perspt restores the terminal (raw mode off, leave alternate screen) only on
the normal exit path; after a crash, ``reset`` will fix it.


Performance
-----------

**Slow response streaming:**

1. Check network latency to the provider
2. Try a faster model (e.g., ``gemini-3.5-flash``)
3. Use Groq for the fastest inference

**High memory usage in agent mode:**

1. Large DAGs with many nodes consume more memory
2. Use ``--max-turns`` and ``--max-calls-per-turn`` to bound per-node work
3. Use ``--rejection-budget`` to bound recovery attempts
