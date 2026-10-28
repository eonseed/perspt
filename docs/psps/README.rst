=====================================
Perspt Specification Proposals (PSPs)
=====================================

This directory contains the Perspt Specification Proposals (PSPs) catalog, managed in pure Rust using ``guidedog dsc`` and rendered to HTML and Typst PDF.

Quick Start
===========

.. code-block:: bash

   # Validate all PSP records and index alignment
   guidedog dsc check --dir=docs/psps

   # Regenerate registry.rst and bundle.rst
   guidedog dsc index --dir=docs/psps

   # Build HTML catalog and per-record Typst PDFs
   guidedog dsc build --dir=docs/psps

   # Create a new draft PSP
   guidedog dsc new "Proposal Title" --labels=architecture --dir=docs/psps

Maintainer decisions
====================

New proposals use ``XXXXX`` and the ``unassigned`` authoring state.
Maintainers assign a number with ``guidedog dsc promote``; the numbered proposal remains ``draft``.
Opening discussion is a separate ``guidedog dsc state`` decision.
Merging a pull request never changes a proposal's number or state automatically.
The complete lifecycle is configured in ``discussion.toml`` and explained in ``workflow.rst``.

Directory Structure
===================

.. code-block:: text

   docs/psps/
   ├── conf.toml                 # Guidedog HTML & PDF configuration
   ├── discussion.toml           # Catalog prefix, numbering, and state machine
   ├── index.rst                 # PSP catalog landing page
   ├── workflow.rst              # PSP process and CLI workflow
   ├── registry.rst              # Auto-generated PSP registry table
   ├── bundle.rst                # Auto-generated reading order toctree
   ├── template/
   │   └── rfc-template.rst      # Template for new PSP drafts
   ├── _templates/
   │   ├── discussion.typ        # Typst layout for individual PSP PDFs
   │   └── theme.typ             # Shared Typst palette and typography
   └── records/
       └── 000001-...rst         # Numbered PSP specification records
