.. _developer-guide-extending:

Extending Perspt
================

How to add new capabilities to Perspt.

Adding a New CLI Command
------------------------

1. **Create command file**: ``crates/perspt-cli/src/commands/mycommand.rs``

   .. code-block:: rust

      use anyhow::Result;

      pub async fn run(arg: String) -> Result<()> {
          println!("Running mycommand with: {}", arg);
          Ok(())
      }

2. **Register in mod.rs**: ``crates/perspt-cli/src/commands/mod.rs``

   .. code-block:: rust

      pub mod mycommand;

3. **Add to CLI enum**: ``crates/perspt-cli/src/main.rs``

   .. code-block:: rust

      #[derive(Subcommand)]
      enum Commands {
          // ... existing commands
          
          /// My new command
          Mycommand {
              /// Argument
              arg: String,
          },
      }

4. **Add match arm**:

   .. code-block:: rust

      Some(Commands::Mycommand { arg }) => commands::mycommand::run(arg).await,

Adding a New Agent Tool
-----------------------

1. **Define tool in tools.rs**: ``crates/perspt-agent/src/tools.rs``

   .. code-block:: rust

      pub fn available_tools() -> Vec<ToolDefinition> {
          vec![
              // ... existing tools
              ToolDefinition {
                  name: "my_tool".to_string(),
                  description: "Does something useful".to_string(),
                  parameters: json!({
                      "type": "object",
                      "properties": {
                          "input": { "type": "string", "description": "The input" }
                      },
                      "required": ["input"]
                  }),
              },
          ]
      }

2. **Implement execution**:

   .. code-block:: rust

      pub async fn execute(&self, call: &ToolCall) -> Result<ToolResult> {
          match call.name.as_str() {
              // ... existing tools
              "my_tool" => {
                  let input = call.arguments["input"].as_str().unwrap();
                  Ok(ToolResult::Success(format!("Processed: {}", input)))
              }
              _ => Err(anyhow!("Unknown tool: {}", call.name)),
          }
      }

Adding a Custom Provider
------------------------

The ``genai`` crate handles providers. To add custom support:

1. **Set environment variable** for new provider
2. **Use provider-specific model names**

For custom API endpoints, modify ``perspt-core/src/llm_provider.rs``.

Adding TUI Components
---------------------

1. **Create widget in perspt-tui**: ``crates/perspt-tui/src/my_widget.rs``

   .. code-block:: rust

      use ratatui::{prelude::*, widgets::*};

      pub struct MyWidget {
          data: String,
      }

      impl MyWidget {
          pub fn new(data: String) -> Self {
              Self { data }
          }

          pub fn render(&self, frame: &mut Frame, area: Rect) {
              let block = Block::default().title("My Widget").borders(Borders::ALL);
              let paragraph = Paragraph::new(self.data.clone()).block(block);
              frame.render_widget(paragraph, area);
          }
      }

2. **Register in lib.rs**:

   .. code-block:: rust

      pub mod my_widget;
      pub use my_widget::MyWidget;

Adding Policy Rules
-------------------

Extend the Starlark policy engine in ``crates/perspt-policy/src/engine.rs``:

.. code-block:: rust

   pub fn add_custom_rule(&mut self, pattern: &str, action: Action) {
       self.rules.push(Rule {
           pattern: pattern.to_string(),
           action,
           reason: None,
       });
   }

Testing Extensions
------------------

.. code-block:: bash

   # Test specific crate
   cargo test -p perspt-agent

   # Run all tests
   cargo test --all

   # With coverage
   cargo tarpaulin

Documentation
-------------

Update docs when extending:

1. Update API docs in ``docs/perspt_book/source/api/``
2. Add usage examples to relevant tutorials
3. Rebuild: ``cd docs/perspt_book && make html``

See Also
--------

- :doc:`architecture` - Crate design
- :doc:`testing` - Testing guide
- :doc:`../api/index` - API reference
