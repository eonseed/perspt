.. _getting-started:

Getting Started
===============

**Start with a conversation, then try the coding agent on a project.** Follow :doc:`quickstart` for installation and a first conversation.
This page gives a short path through the next tasks.

Choose a project task
---------------------

Start in a project whose tests already run locally.
A small task with an observable result is easiest to review:

.. code-block:: bash

   perspt agent -w ./my-project \
     "Add parser tests for empty input and malformed headers. Preserve the public API."

Perspt reads the code, edits a separate project copy, and runs the required checks.
Build errors and failing tests guide further corrections.
The agent asks for review before applying a successful result.
If it runs out of attempts, it stops and reports what remains to be fixed.

Review the result
-----------------

Inspect the diff and the build and test results in the review window.
Approve the edits to copy them into the project, or ask the agent for a correction.
Perspt does not create a Git commit.

**We still assess the tests and the intended behavior.** Passing tests can miss bugs or leave requirements untested.
The default test policy lets tests evolve with implementation.
:doc:`configuration` explains how to require existing tests or separately maintained acceptance tests.

Watch a run
-----------

.. code-block:: bash

   perspt agent --dashboard -w ./my-project "Fix the failing parser test"

Open ``http://127.0.0.1:3000`` while the agent runs.
The embedded dashboard shares the agent's session store and stops when the agent exits.
For completed sessions, use ``perspt dashboard``.
See :doc:`user-guide/dashboard` for details.

Automate after a successful trial
---------------------------------

.. code-block:: bash

   perspt agent --yes --max-turns 12 --rejection-budget 4 \
     --output-summary summary.json \
     -w ./my-project "Add parser tests for empty input"

``--yes`` automatically approves applying a successful result.
It is required without a terminal.
An incomplete or failed run exits with a nonzero status.
See :doc:`tutorials/headless-mode` for automation examples.

Inspect or continue a session
-----------------------------

.. code-block:: bash

   perspt status
   perspt ledger --recent
   perspt replay <session-id>
   perspt resume --last

Replay reads the saved session history without calling the model or rerunning tests.
Resume restores saved progress and checks that the run is still authorized.
``perspt ledger --rollback <session-prefix>`` undoes the latest set of edits applied by that session.
It refuses to overwrite files changed afterward.

Read further
------------

- :doc:`tutorials/agent-mode`: A complete example with review and verification.
- :doc:`user-guide/basic-usage`: Chat commands and keyboard controls.
- :doc:`installation`: Project tools and platform isolation requirements.
- :doc:`api/index`: Use Perspt's libraries from a Rust application.
