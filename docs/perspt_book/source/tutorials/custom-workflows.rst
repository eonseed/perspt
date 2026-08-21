.. _tutorial-custom-workflows:

Custom Workflows
================

Integrate Perspt into scripts, CI/CD pipelines, and automation.

Simple CLI for Scripting
------------------------

The ``simple-chat`` command provides a Unix-friendly interface:

.. code-block:: bash

   # Interactive
   perspt simple-chat

   # With session logging
   perspt simple-chat --log-file session.txt

Batch Agent Runs
----------------

Run multiple agent tasks from a script:

.. code-block:: bash

   #!/bin/bash
   set -e
   export GEMINI_API_KEY="your-key"

   perspt agent --yes --max-turns 8 -w /tmp/proj1 "Create a Python CSV parser"
   perspt agent --yes --max-turns 8 -w /tmp/proj2 "Create a Rust CLI calculator"

CI/CD Integration
-----------------

Use headless mode in CI/CD pipelines:

.. code-block:: yaml

   # GitHub Actions example
   name: Generate Boilerplate
   on:
     workflow_dispatch:
       inputs:
         task:
           description: 'Task description'
           required: true

   jobs:
     generate:
       runs-on: ubuntu-latest
       steps:
         - uses: actions/checkout@v4
         - name: Install Perspt
           run: cargo install perspt
         - name: Run Agent
           env:
             GEMINI_API_KEY: ${{ secrets.GEMINI_API_KEY }}
           run: |
             perspt agent --yes --max-turns 12 \
               -w ./generated "${{ inputs.task }}"
         - name: Commit Results
           run: |
             git add generated/
             git commit -m "Generated: ${{ inputs.task }}"


Post-Run Analysis
-----------------

After an agent run, use the management commands:

.. code-block:: bash

   # Session status
   perspt status

   # Audit a session from the ledger
   perspt replay <SESSION_ID>
   perspt prompts explain-session --db-path <PATH> <SESSION_ID>
   perspt context explain-turn --db-path <PATH> <SESSION_ID>

   # Ledger history
   perspt ledger --recent
   perspt ledger --stats

   # Resume incomplete sessions
   perspt resume --last
