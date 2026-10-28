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

   pub enum FilesystemAccess {
       ReadOnly,
       WorkspaceWrite,
   }

   pub enum IsolationMode {
       Required,
       BestEffort,
   }

   pub struct ProcessPolicy {
       pub workspace_root: PathBuf,
       pub filesystem: FilesystemAccess,
       pub allow_network: bool,
       pub timeout: Duration,
       pub output_limit: usize,
       pub isolation: IsolationMode,
       pub environment: BTreeMap<String, String>,
   }

   pub struct ProcessSandbox { /* OS sandbox runner (bwrap on Linux, sandbox-exec on macOS) */ }

   pub struct BasicSandbox {
       program: String,
       args: Vec<String>,
       working_dir: Option<String>,
       timeout: Option<Duration>,
       // ...
   }

Usage
-----

For OS-isolated child execution (used by the governed runtime and verifier),
construct a ``ProcessSandbox`` with a ``ProcessPolicy``:

.. code-block:: rust

   let policy = ProcessPolicy::inspection("/path/to/project");
   let sandbox = ProcessSandbox::new("cargo", vec!["test".into()], policy)?;
   let result = sandbox.execute()?;
   assert!(result.success());

For direct process execution with active timeout and output limits:

.. code-block:: rust

   let sandbox = BasicSandbox::new("cargo".into(), vec!["test".into()])
       .with_working_dir("/path/to/project".into())
       .with_timeout(Duration::from_secs(60));

   let result = sandbox.execute()?;
   assert!(result.exit_code == Some(0));
