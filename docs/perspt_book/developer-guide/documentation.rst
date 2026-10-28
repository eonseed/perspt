.. _developer-guide-documentation:

Documentation
=============

**The website, book, and proposal catalog are published with Guidedog.** Python, Sphinx, pip, and a virtual environment are not part of this publication workflow.
The source format remains reStructuredText.

Where content lives
-------------------

.. list-table::
   :header-rows: 1
   :widths: 36 64

   * - Source
     - Purpose
   * - ``docs/site/``
     - Landing page, layout template, and landing-page CSS.
   * - ``docs/perspt_book/``
     - Tutorials, user guides, concepts, and references.
   * - ``docs/psps/``
     - Design proposals, lifecycle metadata, and generated registry.
   * - ``docs/shared/_static/``
     - Shared styles, local math rendering assets, and generated figures.
   * - ``docs/diagrams/``
     - Editable Typst figure sources.
   * - ``docs/perspt_book/_templates/``
     - Book layout and shared cover artwork.
   * - ``docs/conf.toml``
     - Website mounts for the landing page, book, PDF, and proposal catalog.

The ``sphinx.ext.graphviz`` and ``sphinx.ext.mathjax`` strings in configuration are compatibility identifiers interpreted by Guidedog.
They do not load Python extensions.
Python snippets, Starlark highlighting, Python-language agent examples, and benchmark fixtures are separate from the documentation publisher.

Build and preview
-----------------

.. code-block:: bash

   guidedog --version

   # HTML only, using prepared artwork when Typst is unavailable
   cargo run -p xtask -- docs --no-pdf

   # Website, book PDF, and proposal PDFs
   cargo run -p xtask -- docs

   # Recompile diagram and cover sources with the standalone Typst CLI
   cargo run -p xtask -- assets

   # Live local preview
   cargo run -p xtask -- serve

Guidedog includes the PDF and Graphviz engines.
The standalone Typst CLI regenerates the checked-in SVG figures and cover previews.
After changing a figure, regenerate its assets before reviewing the site.
CI publishes those prepared assets, so source and output must stay together.

Validate a change
-----------------

.. code-block:: bash

   guidedog dsc check --dir=docs/psps
   guidedog build html docs/perspt_book -W -n
   guidedog build html docs/psps -W -n
   cargo run -p xtask -- docs

We review the landing page and representative book pages at desktop and narrow widths in both themes.
We check the PDF cover, figures, captions, and code examples after layout changes.
The complete publication helper ``./generate-docs.sh`` also builds and stages Rustdoc.
CI pins the publisher version and checksum in ``.github/actions/guidedog/action.yml``.

Keep the documentation tied to behavior
---------------------------------------

- **Commands**: Check ``crates/perspt-cli/src/main.rs`` and each command implementation. Help text alone does not establish stdin or stdout behavior.
- **Scoring and gates**: Check ``crates/perspt-sdk/src/energy.rs`` and ``gate.rs``. An accepted descent is intermediate progress; a hard pass is the delivery success condition.
- **Verification**: Check ``crates/perspt-agent/src/measure.rs`` and ``crates/perspt-core/src/plugin/``. Required stages and advisory stages have different consequences.
- **Promotion and rollback**: Check the runtime and ledger commands. A ledger checkpoint is not a Git commit, and rollback is not arbitrary time travel.
- **Domain support**: Check registered plugins and domain packages. A skeleton contract is not a complete user workflow.
- **Proposal states**: Use the generated registry rather than copying lifecycle states into tutorials. A proposal can describe intended behavior beyond the implementation.

Use short, complete sentences and task-based examples.
We put the command and expected decision before the internal vocabulary.
We label conceptual plots as illustrations and tie empirical claims to recorded evidence.
The quadratic energy formula is continuous in residual magnitudes; the coding runtime measures discrete candidate states.
