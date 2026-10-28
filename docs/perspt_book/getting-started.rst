.. _getting-started:

Getting Started
===============

**We can start with chat and add agent workflows when the project is ready.** Follow :doc:`quickstart` for installation and a first conversation.
This page gives a short path through the next tasks.

Choose a project task
---------------------

Start in a project whose tests already run locally.
A small task with an observable result is easiest to review:

.. code-block:: bash

   perspt agent -w ./my-project \
     "Add parser tests for empty input and malformed headers. Preserve the public API."

Perspt explores the project, edits a candidate copy, and measures the detected checks.
The default run uses one work node.
A lower error score can keep an intermediate checkpoint, while delivery requires a hard pass and promotion approval.
The run can stop with errors remaining when a limit is reached.

Review the result
-----------------

Inspect the diff and required-check results in the review modal.
Approve promotion to copy the result into the project, or send correction feedback.
Promotion does not create a Git commit.

**We still assess the tests and the intended behavior.** A passing suite establishes only what it covers.
The default test policy lets tests evolve with implementation.
:doc:`configuration` explains additional historical and external acceptance evidence.

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

``--yes`` approves promotion automatically within the run's granted authority.
It is required without a terminal.
Any outcome other than a hard pass exits with a nonzero status.
See :doc:`tutorials/headless-mode` for automation examples.

Inspect or continue a session
-----------------------------

.. code-block:: bash

   perspt status
   perspt ledger --recent
   perspt replay <session-id>
   perspt resume --last

Replay audits recorded events without calling the model or rerunning checks.
Resume reconstructs recorded progress and checks authority before continuing.
The command ``perspt ledger --rollback <session-prefix>`` undoes that session's newest completed promotion and refuses to overwrite files changed since promotion.

Read further
------------

- :doc:`tutorials/agent-mode`: A complete example with review and verification.
- :doc:`user-guide/basic-usage`: Chat commands and keyboard controls.
- :doc:`installation`: Project tools and platform isolation requirements.
- :doc:`api/index`: Reuse scoring and gate contracts in Rust.
