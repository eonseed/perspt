.. _api-perspt-sandbox:

``perspt-sandbox``
==================

Process isolation for agent-executed commands.

Core Types
----------

.. code-block:: rust

   pub struct CommandResult {
       pub stdout: String,
       pub stderr: String,
       pub exit_code: Option<i32>,
       pub timed_out: bool,
       pub duration: Duration,
   }

   pub trait SandboxedCommand: Send + Sync {
       fn execute(&self) -> Result<CommandResult>;
       fn display(&self) -> String;
       fn is_read_only(&self) -> bool;
   }

   pub struct BasicSandbox {
       program: String,
       args: Vec<String>,
       working_dir: Option<PathBuf>,
       timeout: Duration,
   }

Usage
-----

.. code-block:: rust

   let sandbox = BasicSandbox::new("cargo", vec!["test"])
       .with_working_dir("/path/to/project")
       .with_timeout(Duration::from_secs(60));

   let result = sandbox.execute()?;
   assert!(result.exit_code == Some(0));

``BasicSandbox`` can also be created from a command string:

.. code-block:: rust

   let sandbox = BasicSandbox::from_command_string("uv run pytest -v")?;
