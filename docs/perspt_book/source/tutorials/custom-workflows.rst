.. _tutorial-custom-workflows:

Custom Workflows
================

Build automated pipelines with Perspt.

Overview
--------

Perspt can be integrated into automated workflows for:

- CI/CD code generation
- Batch processing
- Scripted interactions
- Test automation

Scripting with Agent Mode
-------------------------

Use agent mode in scripts:

.. code-block:: bash

   #!/bin/bash
   # generate_tests.sh

   for file in src/*.py; do
       perspt agent -y -w . "Add unit tests for $file"
   done

Batch Code Generation
---------------------

Process a list of tasks:

.. code-block:: bash

   #!/bin/bash
   # batch_tasks.sh

   TASKS=(
       "Add type hints to utils.py"
       "Create docstrings for api.py"
       "Add error handling to db.py"
   )

   for task in "${TASKS[@]}"; do
       echo "Processing: $task"
       perspt agent -y --max-cost 1.0 "$task"
   done

CI/CD Integration
-----------------

GitHub Actions example:

.. code-block:: yaml

   # .github/workflows/code-review.yml
   name: AI Code Review
   on: [pull_request]
   
   jobs:
     review:
       runs-on: ubuntu-latest
       steps:
         - uses: actions/checkout@v4
         
         - name: Install Perspt
           run: cargo install perspt
         
         - name: Run AI Review
           env:
             OPENAI_API_KEY: ${{ secrets.OPENAI_API_KEY }}
           run: |
             perspt agent -y "Review code changes and suggest improvements"

Programmatic API
----------------

Use perspt-agent crate directly:

.. code-block:: rust

   use perspt_agent::{SRBNOrchestrator, OrchestratorOptions};
   use perspt_core::GenAIProvider;
   use std::sync::Arc;

   #[tokio::main]
   async fn main() -> anyhow::Result<()> {
       let provider = Arc::new(GenAIProvider::new()?);
       
       let options = OrchestratorOptions {
           architect_model: Some("gpt-5.2".to_string()),
           actuator_model: Some("claude-opus-4.5".to_string()),
           ..Default::default()
       };
       
       let mut orchestrator = SRBNOrchestrator::new(
           provider,
           ".".into(),
           options,
       ).await?;
       
       let result = orchestrator.execute("Add unit tests").await?;
       println!("Result: {:?}", result);
       
       Ok(())
   }

Ledger Automation
-----------------

Automate rollbacks on failure:

.. code-block:: bash

   #!/bin/bash
   # safe_agent.sh

   # Store current state
   BEFORE=$(perspt ledger --recent | head -1 | cut -d' ' -f1)

   # Run agent
   perspt agent -y "$1"

   # Run tests
   if ! python -m pytest; then
       echo "Tests failed, rolling back..."
       perspt ledger --rollback "$BEFORE"
       exit 1
   fi

   echo "Success!"

Policy Automation
-----------------

Create project-specific rules:

.. code-block:: bash

   # Initialize with rules
   perspt init --rules

   # Edit .perspt/rules.star
   cat > .perspt/rules.star << 'EOF'
   # Allow read operations
   allow("cat *")
   allow("ls *")

   # Prompt for writes
   prompt("rm *", reason="File deletion")

   # Deny dangerous
   deny("rm -rf *")
   EOF

See Also
--------

- :doc:`agent-mode` - Agent fundamentals
- :doc:`../howto/configuration` - Project config
- :doc:`../api/perspt-agent` - Programmatic API
