===========================
PSP System Maintainer Guide
===========================

This guide describes how maintainers manage the Perspt Specification Proposals (PSPs) catalog using ``guidedog dsc``.

Catalog Operations
==================

All PSP operations run through ``guidedog dsc`` configured by ``docs/psps/discussion.toml``:

.. code-block:: bash

   # List all PSPs and their current states
   guidedog dsc list --dir=docs/psps

   # Check metadata headers, section structure, and registry consistency
   guidedog dsc check --dir=docs/psps

   # Refresh generated blocks in registry.rst and bundle.rst
   guidedog dsc index --dir=docs/psps

   # Promote a draft (XXXXX) to the next 6-digit number
   guidedog dsc promote <slug> --dir=docs/psps

   # Transition a record's state
   guidedog dsc state <number> --to=<state> --reason="<reason>" --dir=docs/psps

   # Build HTML and Typst PDF outputs
   guidedog dsc build --dir=docs/psps

Quality Gate
============

Before merging any PSP change, verify:

1. ``guidedog dsc check --dir=docs/psps`` passes with zero errors.
2. ``guidedog build html docs/psps -W -n`` passes with zero warnings or broken links.
3. ``guidedog dsc build --dir=docs/psps`` builds all HTML pages and Typst PDFs cleanly.
