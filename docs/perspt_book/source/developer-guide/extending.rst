.. _developer-guide-extending:

Extending Perspt
================

Adding a Language Plugin
------------------------

Language plugins implement the ``LanguagePlugin`` trait from ``perspt-core``.

1. Create a new module in ``crates/perspt-core/src/plugin/``:

   .. code-block:: rust

      pub struct GoPlugin;

      impl LanguagePlugin for GoPlugin {
          fn name(&self) -> &str { "go" }
          fn extensions(&self) -> &[&str] { &["go"] }
          fn key_files(&self) -> &[&str] { &["go.mod", "go.sum"] }

          fn detect(&self, path: &Path) -> bool {
              path.join("go.mod").exists()
          }

          fn get_init_action(&self, opts: &InitOptions) -> ProjectAction {
              ProjectAction::ExecCommand {
                  command: format!("go mod init {}", opts.name),
                  description: "Initialize Go module".into(),
              }
          }

          fn test_command(&self) -> Option<String> {
              Some("go test ./...".into())
          }

          fn syntax_check_command(&self) -> Option<String> {
              Some("go vet ./...".into())
          }

          fn verifier_profile(&self) -> VerifierProfile {
              // Define capabilities for each verifier stage
              // with primary and fallback commands
          }
      }

2. Register in the ``PluginRegistry``
3. Add LSP config for ``gopls``

Adding an Agent Tool
--------------------

Tools are defined in ``crates/perspt-agent/src/tools.rs``:

.. code-block:: rust

   impl AgentTools {
       pub fn get_available_tools(&self) -> Vec<ToolDefinition> {
           vec![
               // ... existing tools ...
               ToolDefinition {
                   name: "my_tool".into(),
                   description: "Description".into(),
                   parameters: vec![
                       ToolParameter {
                           name: "arg1".into(),
                           description: "First argument".into(),
                           required: true,
                       },
                   ],
               },
           ]
       }

       async fn execute_my_tool(&self, args: &HashMap<String, String>)
           -> ToolResult
       {
           // Implementation
       }
   }


Adding a Sheaf Validator
-------------------------

Sheaf validators check cross-node contracts. Add a new variant to
``SheafValidatorClass`` in ``perspt-core/src/types.rs``:

.. code-block:: rust

   pub enum SheafValidatorClass {
       ExportImportConsistency,
       DependencyGraphConsistency,
       // ... existing variants ...
       MyNewValidator,     // Add new variant
   }

Then implement the validation logic in the orchestrator's sheaf validation phase.


Adding an LLM Provider
-----------------------

The ``genai`` adapter in ``perspt-core/src/llm_provider.rs`` handles providers.
To add a new provider:

1. Add the adapter kind to ``str_to_adapter_kind()``
2. Map the env var in ``new_with_config()``
3. Add default model fallbacks in ``perspt-cli/src/main.rs``
4. Update the auto-detection priority in config

.. code-block:: rust

   fn str_to_adapter_kind(provider: &str) -> AdapterKind {
       match provider {
           "openai" => AdapterKind::OpenAI,
           "anthropic" => AdapterKind::Anthropic,
           // ... existing providers ...
           "newprovider" => AdapterKind::NewProvider,
           _ => AdapterKind::OpenAI,
       }
   }


Adding a Starlark Policy
--------------------------

Create a ``.star`` file in the policy directory (``~/.config/perspt/policies/``):

.. code-block:: python

   # custom_policy.star

   def check_file_write(path, content):
       # Called before any file write.
       if path.endswith(".env"):
           return deny("Cannot write .env files")
       return allow()

   def check_command(cmd):
       # Called before any command execution.
       if "curl" in cmd and "http://" in cmd:
           return prompt("Insecure HTTP request: " + cmd)
       return allow()

The ``PolicyEngine`` loads all ``.star`` files from the policy directory automatically.
