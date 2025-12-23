perspt-policy API
=================

Security policy engine for command approval and sanitization.

Overview
--------

``perspt-policy`` provides security controls for agent operations:

.. graphviz::
   :align: center
   :caption: Policy Engine Flow

   digraph policy {
       rankdir=LR;
       node [shape=box, style="rounded,filled", fontname="Helvetica", fontsize=11];
       edge [fontname="Helvetica", fontsize=10];
       
       cmd [label="Command\nRequest", fillcolor="#E3F2FD"];
       sanitize [label="Sanitizer\n━━━━━━━━━\nClean Input", fillcolor="#FFF3E0"];
       engine [label="PolicyEngine\n━━━━━━━━━\nEvaluate Rules", fillcolor="#E8F5E9"];
       
       allow [label="✓ Allow", shape=ellipse, fillcolor="#C8E6C9"];
       deny [label="✗ Deny", shape=ellipse, fillcolor="#FFCDD2"];
       
       cmd -> sanitize;
       sanitize -> engine;
       engine -> allow [label="pass"];
       engine -> deny [label="fail"];
   }

PolicyEngine
------------

Starlark-based policy evaluation engine.

.. code-block:: rust
   :caption: PolicyEngine structure

   pub struct PolicyEngine {
       rules: Vec<Rule>,
       default_action: Action,
   }

   pub enum Action {
       Allow,
       Deny,
       Prompt,
   }

   pub struct Rule {
       pub pattern: String,
       pub action: Action,
       pub reason: Option<String>,
   }

   impl PolicyEngine {
       /// Create engine with default rules
       pub fn new() -> Self

       /// Load rules from Starlark file
       pub fn load_rules(path: &Path) -> Result<Self>

       /// Evaluate a command against rules
       pub fn evaluate(&self, command: &str) -> PolicyDecision
   }

   pub struct PolicyDecision {
       pub action: Action,
       pub matched_rule: Option<Rule>,
       pub reason: String,
   }

Default Rules
~~~~~~~~~~~~~

The engine includes built-in safety rules:

.. list-table::
   :header-rows: 1
   :widths: 30 15 55

   * - Pattern
     - Action
     - Reason
   * - ``rm -rf /``
     - Deny
     - Destructive root deletion
   * - ``rm -rf ~``
     - Deny
     - Home directory deletion
   * - ``chmod 777``
     - Prompt
     - Insecure permissions
   * - ``curl | bash``
     - Deny
     - Remote code execution
   * - ``sudo *``
     - Prompt
     - Privilege escalation

Sanitizer
---------

Command input sanitization and validation.

.. code-block:: rust
   :caption: Sanitizer structure

   pub struct Sanitizer {
       // Sanitization rules
   }

   impl Sanitizer {
       pub fn new() -> Self

       /// Sanitize a command string
       pub fn sanitize(&self, command: &str) -> Result<String>

       /// Check for path traversal attempts
       pub fn check_path_traversal(&self, path: &str) -> bool

       /// Check for command injection
       pub fn check_injection(&self, input: &str) -> bool
   }

Security Checks
~~~~~~~~~~~~~~~

.. list-table::
   :header-rows: 1
   :widths: 25 75

   * - Check
     - Description
   * - Path Traversal
     - Detects ``../`` patterns escaping workspace
   * - Command Injection
     - Detects ``;``, ``|``, ``&&``, ``$()`` in untrusted input
   * - Null Bytes
     - Removes null bytes that can truncate strings
   * - Shell Metacharacters
     - Escapes or rejects dangerous characters

Custom Rules
------------

Create custom Starlark rules in ``.perspt/rules.star``:

.. code-block:: python
   :caption: Example rules.star

   # Allow read operations
   allow("cat *")
   allow("head *")
   allow("tail *")

   # Prompt for writes
   prompt("rm *", reason="File deletion")
   prompt("mv *", reason="File move/rename")

   # Deny dangerous operations
   deny("rm -rf *", reason="Recursive force delete")
   deny("chmod -R *", reason="Recursive permission change")

Usage Example
-------------

.. code-block:: rust
   :caption: Using PolicyEngine

   use perspt_policy::{PolicyEngine, Sanitizer, Action};

   fn check_command(cmd: &str) -> Result<()> {
       let sanitizer = Sanitizer::new();
       let engine = PolicyEngine::new();

       // Sanitize input
       let clean_cmd = sanitizer.sanitize(cmd)?;

       // Evaluate policy
       let decision = engine.evaluate(&clean_cmd);

       match decision.action {
           Action::Allow => execute(clean_cmd),
           Action::Prompt => {
               if user_approves(&decision.reason) {
                   execute(clean_cmd)
               }
           }
           Action::Deny => {
               eprintln!("Denied: {}", decision.reason);
           }
       }
   }

Source Code
-----------

:file:`crates/perspt-policy/src/engine.rs`: Policy engine (7KB)
:file:`crates/perspt-policy/src/sanitize.rs`: Sanitizer (5KB)
