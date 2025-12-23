perspt-sandbox API
==================

Process isolation for safe command execution.

Overview
--------

``perspt-sandbox`` provides controlled execution of shell commands with resource limits and isolation.

.. graphviz::
   :align: center
   :caption: Sandbox Execution Flow

   digraph sandbox {
       rankdir=LR;
       node [shape=box, style="rounded,filled", fontname="Helvetica", fontsize=11];
       edge [fontname="Helvetica", fontsize=10];
       
       cmd [label="Command", fillcolor="#E3F2FD"];
       
       subgraph cluster_sandbox {
           label="Sandbox Environment";
           style=dashed;
           color="#E53935";
           
           limits [label="Resource\nLimits", fillcolor="#FFECB3"];
           exec [label="Process\nExecution", fillcolor="#FFF3E0"];
           capture [label="Output\nCapture", fillcolor="#E8F5E9"];
       }
       
       result [label="Result", fillcolor="#C8E6C9"];
       
       cmd -> limits;
       limits -> exec;
       exec -> capture;
       capture -> result;
   }

SandboxedCommand
----------------

Safe command execution with limits.

.. code-block:: rust
   :caption: SandboxedCommand structure

   pub struct SandboxedCommand {
       command: String,
       args: Vec<String>,
       working_dir: PathBuf,
       timeout: Duration,
       max_output: usize,
   }

   pub struct CommandResult {
       pub exit_code: i32,
       pub stdout: String,
       pub stderr: String,
       pub duration: Duration,
       pub truncated: bool,
   }

   impl SandboxedCommand {
       /// Create a new sandboxed command
       pub fn new(command: &str) -> Self

       /// Set working directory
       pub fn working_dir(mut self, path: PathBuf) -> Self

       /// Set execution timeout
       pub fn timeout(mut self, duration: Duration) -> Self

       /// Set maximum output size
       pub fn max_output(mut self, bytes: usize) -> Self

       /// Add command arguments
       pub fn args<I, S>(mut self, args: I) -> Self
       where
           I: IntoIterator<Item = S>,
           S: AsRef<str>

       /// Execute the command
       pub async fn execute(self) -> Result<CommandResult>
   }

Default Limits
--------------

.. list-table::
   :header-rows: 1
   :widths: 30 25 45

   * - Resource
     - Default Limit
     - Purpose
   * - Timeout
     - 60 seconds
     - Prevent hanging processes
   * - Output Size
     - 1 MB
     - Prevent memory exhaustion
   * - Process Count
     - 10
     - Limit fork bombs
   * - File Descriptors
     - 256
     - Prevent resource exhaustion

Security Features
-----------------

.. admonition:: Isolation Mechanisms
   :class: warning

   - **Working Directory Restriction**: Commands run in specified workspace only
   - **Environment Sanitization**: Only safe environment variables passed
   - **Output Truncation**: Large outputs are truncated with warning
   - **Timeout Enforcement**: Processes killed after timeout

Usage Example
-------------

.. code-block:: rust
   :caption: Using SandboxedCommand

   use perspt_sandbox::SandboxedCommand;
   use std::time::Duration;

   #[tokio::main]
   async fn main() -> Result<()> {
       let result = SandboxedCommand::new("pytest")
           .args(["tests/", "-v"])
           .working_dir("/path/to/project".into())
           .timeout(Duration::from_secs(120))
           .max_output(2 * 1024 * 1024)  // 2MB
           .execute()
           .await?;

       println!("Exit code: {}", result.exit_code);
       println!("Duration: {:?}", result.duration);
       
       if result.truncated {
           println!("Warning: Output was truncated");
       }
       
       println!("{}", result.stdout);

       Ok(())
   }

Integration with Agent
----------------------

The agent uses ``SandboxedCommand`` for all shell operations:

.. code-block:: rust
   :caption: Agent tool integration

   impl AgentTools {
       async fn execute_shell(&self, cmd: &str) -> Result<ToolResult> {
           // First, check policy
           let decision = self.policy_engine.evaluate(cmd);
           if decision.action == Action::Deny {
               return Err(anyhow!("Denied: {}", decision.reason));
           }

           // Execute in sandbox
           let result = SandboxedCommand::new("sh")
               .args(["-c", cmd])
               .working_dir(self.workspace.clone())
               .timeout(Duration::from_secs(60))
               .execute()
               .await?;

           Ok(ToolResult::Success(result.stdout))
       }
   }

Source Code
-----------

:file:`crates/perspt-sandbox/src/command.rs`: SandboxedCommand (5KB)
