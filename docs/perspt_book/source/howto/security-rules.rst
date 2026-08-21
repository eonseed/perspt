.. _howto-security-rules:

Security and Policy Rules
=========================

Perspt provides multiple layers of security for agent mode.

Starlark Policies (perspt-policy)
---------------------------------

The ``perspt-policy`` crate evaluates Starlark scripts against every
command line the agent proposes. Policies can:

- **Deny shell commands** - Block dangerous commands (``rm -rf``, etc.)
- **Require confirmation** - Gate risky commands behind a user prompt

Each ``.star`` file in the rules directory (``rules/`` under the config
directory) must define ``evaluate``, a function that receives the full
command line and returns ``"allow"``, ``"prompt"``, or ``"deny"`` (a bool
is also accepted). The builtins are ``matches_pattern(command, pattern)``
(substring match) and ``log_policy(message)``:

.. code-block:: python

   # Example Starlark policy
   def evaluate(command):
       for pattern in ["rm -rf", "sudo", "chmod 777"]:
           if matches_pattern(command, pattern):
               log_policy("blocked: " + command)
               return "deny"
       if matches_pattern(command, "git push"):
           return "prompt"
       return "allow"

When several files are loaded, the strictest decision wins, and a policy
that fails to evaluate denies the command.


Sandbox Isolation (perspt-sandbox)
----------------------------------

The ``perspt-sandbox`` crate provides filesystem and process isolation for
agent-executed commands:

- **Filesystem scoping** - Commands run in a restricted view of the filesystem
- **Process limits** - Timeout, memory, and CPU constraints
- **Network control** - Optional network access restriction

Configuration:

.. code-block:: bash

   # Agent with sandbox enabled
   perspt agent -w ./project "Task"

   # The sandbox restricts commands to the working directory


Authority Model
---------------

PSP-9 authority is capability-based:

- Live capabilities are minted from grants at session start and are bound
  to the session's authority epoch
- ``perspt abort`` revokes the epoch durably: in-flight promotions and
  stale resume intents are refused, and the workspace is untouched
- ``--allow-dependency-mutation`` is the explicit grant for governed
  dependency mutation (``cargo add``, ``uv add``, ``npm install``)
- ``--persistent-grants`` persists signed grant intent only, never live
  capabilities: resume must mint fresh ones


Footprint Scheduling
--------------------

The multi-node dispatcher schedules by write footprints:

- Each node declares the files it writes (a node that declares none holds
  an opaque whole-workspace footprint)
- Nodes with conflicting footprints never run concurrently
- Promotion is single-flight: it runs only in the dispatcher's completion
  arm

This prevents conflicting edits and provides a clear audit trail.


Review Modal
------------

In interactive mode (without ``--yes``), every node's changes must be manually
approved. The review modal shows:

- Full diff of all changes
- Verification results (syntax, build, test, lint), the measured energy,
  and a degraded flag with reasons when any sensor was skipped
- Options to reject, correct, or edit

For security-sensitive projects, always use interactive mode.


Merkle Ledger
-------------

Every committed change is recorded in a content-addressed Merkle tree stored in
DuckDB. This provides:

- **Tamper detection** - Hash chain integrity
- **Full auditability** - Every node's input/output is recorded
- **Rollback capability** - Restore to any point in the session

.. code-block:: bash

   perspt ledger --recent
   perspt ledger --stats


Best Practices
--------------

1. **Use interactive mode for production code** - Always review diffs
2. **Replay sessions** - ``perspt replay <SESSION_ID>`` is a deterministic,
   credential-free audit replay
3. **Use workspace directories** - ``-w <dir>`` scopes agent writes
4. **Label delayed audit samples** - ``perspt audit`` lists pending samples
   and records safe/unsafe labels
5. **Inspect prompt and context provenance** - ``perspt prompts
   explain-session`` and ``perspt context explain-turn`` show what a session
   compiled and compacted
6. **Review ledger after headless runs** - ``perspt ledger --recent``
