.. _howto-security-rules:

Security and Policy Rules
=========================

Perspt provides multiple layers of security for agent mode.

Starlark Policies (perspt-policy)
---------------------------------

The ``perspt-policy`` crate evaluates Starlark scripts to enforce rules on
agent actions. Policies can:

- **Allow/deny file operations** — Prevent writes to sensitive paths
- **Restrict shell commands** — Block dangerous commands (``rm -rf``, etc.)
- **Enforce naming conventions** — Require files match patterns
- **Limit resource usage** — Cap file sizes, directory depth

.. code-block:: python

   # Example Starlark policy
   def check_file_write(path, content):
       if path.startswith("/etc/") or path.startswith("/usr/"):
           return deny("Cannot write to system directories")
       if len(content) > 1_000_000:
           return deny("File too large (>1MB)")
       return allow()

   def check_command(cmd):
       forbidden = ["rm -rf", "sudo", "chmod 777"]
       for f in forbidden:
           if f in cmd:
               return deny("Forbidden command: " + f)
       return allow()


Sandbox Isolation (perspt-sandbox)
----------------------------------

The ``perspt-sandbox`` crate provides filesystem and process isolation for
agent-executed commands:

- **Filesystem scoping** — Commands run in a restricted view of the filesystem
- **Process limits** — Timeout, memory, and CPU constraints
- **Network control** — Optional network access restriction

Configuration:

.. code-block:: bash

   # Agent with sandbox enabled
   perspt agent -w ./project "Task"

   # The sandbox restricts commands to the working directory


Ownership Closure
-----------------

The PSP-5 ownership closure rule ensures:

- Each file is owned by exactly one DAG node
- No two nodes can write to the same file
- Violations trigger a re-plan by the Architect

This prevents conflicting edits and provides a clear audit trail.


Review Modal
------------

In interactive mode (without ``--yes``), every node's changes must be manually
approved. The review modal shows:

- Full diff of all changes
- Verification results (V_syn, V_str, V_log, V_boot, V_sheaf)
- Options to reject, correct, or edit

For security-sensitive projects, always use interactive mode.


Merkle Ledger
-------------

Every committed change is recorded in a content-addressed Merkle tree stored in
DuckDB. This provides:

- **Tamper detection** — Hash chain integrity
- **Full auditability** — Every node's input/output is recorded
- **Rollback capability** — Restore to any point in the session

.. code-block:: bash

   perspt ledger --recent
   perspt ledger --stats


Best Practices
--------------

1. **Use interactive mode for production code** — Always review diffs
2. **Set cost limits** — ``--max-cost`` prevents runaway spending
3. **Use workspace directories** — ``-w <dir>`` scopes agent writes
4. **Enable LLM logging** — ``--log-llm`` for post-run auditing
5. **Review ledger after headless runs** — ``perspt ledger --recent``
