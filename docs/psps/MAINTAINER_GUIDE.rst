PSP Maintainer Guide
====================

**Maintainers assign PSP numbers and decide when a proposal changes state.** GitHub Actions validates the catalog, builds the documentation, and reports stale discussions.
It never promotes a proposal or changes its state after a merge.

Create an unnumbered proposal
-----------------------------

.. code-block:: bash

   guidedog dsc new "Proposal title" --labels=architecture --dir=docs/psps

The new file is ``records/XXXXX-proposal-title.rst`` with state ``unassigned``.
We can review and merge the draft without assigning a number or opening discussion.
Set its ``:psp-type:`` metadata to one of the types allowed by ``discussion.toml``.

Assign a number
---------------

After maintainer review, run:

.. code-block:: bash

   guidedog dsc promote proposal-title --dir=docs/psps

This assigns the next six-digit number, renames the file, refreshes the registry, and sets the state to ``draft``.
It does not open discussion.
We include these changes in a reviewed pull request.
Existing numbered drafts keep their number and use ``state`` for later decisions.

Open discussion
---------------

When the proposal is ready for community review:

.. code-block:: bash

   guidedog dsc state <number> --to=discussion \
     --reason="Ready for community review" --dir=docs/psps

Record the discussion issue in the proposal and submit the state change for review.
After discussion, maintainers can accept or reject the proposal, adopt a living specification as ``active``, or withdraw it.
The allowed transitions come from ``discussion.toml``.

Record a decision
-----------------

.. code-block:: bash

   guidedog dsc state <number> --to=accepted \
     --reason="Maintainers approved the design after review" --dir=docs/psps

   # After the implementation is merged and verified
   guidedog dsc state <number> --to=final \
     --reason="Implementation complete" \
     --evidence="Link to the implementation and passing checks" --dir=docs/psps

The ``final`` state requires evidence.
Use ``--dry-run`` to inspect a proposed change without writing it.
No lifecycle decision follows automatically from a pull request being merged.

Validate before submitting
--------------------------

.. code-block:: bash

   guidedog dsc index --dir=docs/psps
   guidedog dsc check --dir=docs/psps
   guidedog build html docs/psps -W -n
   guidedog dsc build --dir=docs/psps

Review the record, registry, and reading-order changes together.
The weekly reminder workflow only lists discussions unchanged for 60 days; it does not close or advance them.
