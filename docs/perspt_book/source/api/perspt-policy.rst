.. _api-perspt-policy:

``perspt-policy``
=================

Starlark-based policy engine for agent action governance.

Core Types
----------

.. code-block:: rust

   pub struct PolicyEngine {
       policies: Vec<FrozenModule>,
       policy_dir: PathBuf,
   }

   pub enum PolicyDecision {
       Allow,
       Prompt(String),
       Deny(String),
   }

   pub struct SanitizeResult {
       pub parts: Vec<String>,
       pub warnings: Vec<String>,
       pub rejected: bool,
       pub rejection_reason: Option<String>,
   }

Functions
---------

.. list-table::
   :header-rows: 1
   :widths: 40 60

   * - Function
     - Description
   * - ``PolicyEngine::new()``
     - Create engine instance
   * - ``PolicyEngine::load_policies()``
     - Load all .star files from policy directory
   * - ``sanitize_command(cmd)``
     - Parse, validate, and filter a shell command
   * - ``validate_workspace_bound(cmd, dir)``
     - Ensure command stays within working directory
   * - ``is_safe_for_auto_exec(cmd)``
     - Whitelist check for auto-approval in balanced mode
