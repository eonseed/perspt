.. _tutorial-agent-mode:

Agent Mode Tutorial
===================

**Ask Perspt to add a small feature, run the tests, and show the changes for review.** In this tutorial we add a function to a Rust library.
The task uses only the standard library, so no new dependencies are needed.

Prerequisites
-------------

- Perspt installed and a model provider configured; see :doc:`../quickstart`.
- Cargo and rustc installed, with the project's required tools available.
- Process isolation set up for agent mode; see :doc:`../installation`.

Agent mode is experimental.
A run may stop before finishing, and passing tests can still miss bugs.
We review both the code and the tests before accepting the result.

1. Create a small project
-------------------------

In a directory outside the Perspt source checkout:

.. code-block:: bash

   cargo new --lib parser-demo
   cd parser-demo
   cargo test

The generated library has a passing test.
This gives the agent a working starting point.

2. Describe the function and its tests
--------------------------------------

.. code-block:: bash

   perspt agent -w . \
     "Add a public parse_count function to src/lib.rs.
      It takes a &str and returns Result<u32, std::num::ParseIntError>.
      Trim whitespace before parsing. Reject empty input, negative numbers,
      and non-numeric text. Add tests for valid input, whitespace,
      empty input, negative numbers, and invalid text."

The prompt gives a name, a signature, and examples of expected behavior.
These details make the result easier to test and review.

3. Follow the build and test results
------------------------------------

The agent reads the project and edits a separate copy.
Perspt runs the Rust checks and sends any build or test errors back to the agent.
The agent can try corrections within the run's limits.

**Keep improvements. Reject regressions.** While the required checks still fail, Perspt can save an attempt that reduces its error score enough.
An attempt that fails this progress check is discarded in favor of the best saved version.
The original project files are updated only after the required checks pass and the result is approved.

To watch in a browser, add ``--dashboard`` to the command and open ``http://127.0.0.1:3000`` while the agent runs.
See :doc:`dashboard-monitoring` for the dashboard pages.

4. Review the diff
------------------

When a result is ready, the review window shows the edited files, diff, and check results.

- **y** approves applying the changes.
- **n** rejects the changes.
- **c** sends feedback so the agent can try a correction.
- **d** shows the full diff.

Check the implementation of ``parse_count`` and the assertions in its tests.
For example, a test for negative input should assert that parsing returns an error, rather than silently converting the value.

Approval copies the edited files into the project.
Perspt does not create a Git commit.

5. Inspect the result locally
-----------------------------

.. code-block:: bash

   cargo test
   git diff -- src/lib.rs

A successful result defines ``parse_count`` and tests the cases requested in the prompt.
The exact test names and organization depend on the implementation.
We can then commit the reviewed changes with Git.

If the agent cannot finish
--------------------------

A run that reaches its attempt limits stops and reports the remaining problems.
Inspect the compiler output, failing tests, and session history before trying again:

.. code-block:: bash

   perspt status
   perspt ledger --recent
   perspt replay <session-id>

Replay reads the recorded history without calling the model or rerunning the tools.
For missing tool errors, confirm that the project's compiler and tests run locally.
For interrupted work, ``perspt resume --last`` continues from saved progress after checking that the run is still authorized.

Run without the review prompt
-----------------------------

Once the workflow is familiar, ``--yes`` automatically approves applying a successful result:

.. code-block:: bash

   perspt agent --yes --output-summary result.json -w . \
     "Add a test for a number larger than u32::MAX to parse_count."

The required checks still run.
An incomplete or failed run exits with a nonzero status.
The JSON file records the outcome, session ID, and changed paths.
See :doc:`headless-mode` for scripts and CI examples.

Other languages and settings
----------------------------

The same command can work on Python and JavaScript/TypeScript projects when their required tools and dependencies are installed.
:doc:`../installation` lists the prerequisites.
For tasks that add dependencies, an explicit ``--allow-dependency-mutation`` grant is needed.

- :doc:`../user-guide/agent-mode`: Review controls, test policies, and session management.
- :doc:`../user-guide/advanced-features`: Model selection, attempt limits, and parallel tasks.
- :doc:`../configuration`: Require historical tests or a separately maintained acceptance suite.
- :doc:`../concepts/srbn-architecture`: How the error score and acceptance rules work.
