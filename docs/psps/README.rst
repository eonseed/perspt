=====================================
Perspt Specification Proposals (PSPs)
=====================================

This directory contains the Perspt Specification Proposals (PSPs) documentation system, designed to manage and document significant changes to the Perspt terminal user interface application. The system follows the structure and approach of Python's PEPs repository, with a custom Sphinx extension and theme.

Features
========

* **Custom PSP Parser**: Parses PSP metadata (PSP number, title, author, status, etc.) into proper document headers
* **Professional Theme**: Clean, accessible design with light/dark theme toggle 
* **6-Digit PSP Numbering**: Supports PSP numbers from 000000 to 999999
* **Collapsible Table of Contents**: Each PSP document includes an expandable/collapsible ToC at the top
* **Field List Preambles**: PSP metadata is displayed as a professional field list similar to Python PEPs
* **Modern Build System**: Uses Sphinx with uv for fast dependency management

Overview
========

PSPs are design documents that provide information to the Perspt community and describe new features, processes, or design decisions for Perspt. This system is inspired by Python's PEP process but tailored for a terminal UI application focused on development workflow enhancement.

Quick Start
===========

Building the Documentation
---------------------------

1. Install dependencies:

   .. code-block:: bash

      cd docs/psps
      uv install
      # or
      pip install sphinx

2. Build the HTML documentation:

   .. code-block:: bash

      make html
      # or
      uv run make html

3. Open ``build/html/contents.html`` in your browser

Creating a New PSP
------------------

1. Copy the template:

   .. code-block:: bash

      cp source/psp-template.rst source/psp-XXXXXX-your-title.rst

2. Fill out the template with your proposal details (use 6-digit PSP numbers)

3. Add the new PSP to ``source/contents.rst``

4. Create a GitHub Issue for discussion

5. Submit a Pull Request with your PSP

6. A maintainer will assign an official PSP number (6 digits)

5. A maintainer will assign an official PSP number

Directory Structure
===================

.. code-block::

   docs/psps/
   ├── Makefile                     # Build configuration  
   ├── build.py                     # Custom build script
   ├── pyproject.toml              # Python dependencies
   ├── README.rst                  # This file
   ├── psp_sphinx_extensions/      # Custom Sphinx extension
   │   ├── __init__.py            # Extension entry point
   │   ├── psp_theme/             # Custom PSP theme
   │   │   ├── theme.conf         # Theme configuration
   │   │   ├── templates/         # HTML templates
   │   │   │   └── page.html      # Main page template
   │   │   └── static/           # CSS, JS, and assets
   │   │       ├── style.css     # Main stylesheet
   │   │       ├── mq.css        # Media queries
   │   │       ├── colour_scheme.js # Theme switching
   │   │       └── wrap_tables.js # Table enhancements
   │   └── psp_processor/         # PSP processing logic
   │       ├── html/             # HTML builders and translators
   │       └── parsing/          # PSP parsers and roles
   ├── source/
   │   ├── conf.py               # Sphinx configuration
   │   ├── contents.rst          # Main contents page (index)
   │   ├── psp-000000.rst       # PSP index (6-digit format)
   │   ├── psp-000001.rst       # PSP process definition
   │   ├── psp-000002.rst       # Example PSP
   │   ├── psp-template.rst     # Template for new PSPs
   │   └── psp-XXXXXX.rst       # Additional PSPs (6-digit format)
   └── build/                    # Generated documentation

Available Commands
==================

.. code-block:: bash

   # Build HTML documentation
   make html
   uv run make html

   # Build PDF documentation (requires LaTeX)
   make latexpdf
   uv run make latexpdf

   # Clean build directory
   make clean
   uv run make clean

   # Watch for changes and auto-rebuild (if sphinx-autobuild is installed)
   make livehtml
   uv run sphinx-autobuild source build/html

Architecture
============

The PSP system uses a custom Sphinx extension that closely follows the Python PEPs approach:

**Custom Theme**: A specialized theme optimized for PSP documents with proper styling, responsive design, and accessibility features.

**PSP Parser**: Custom parsing logic that understands PSP-specific metadata and cross-references.

**HTML Builders**: Specialized HTML generation for PSP documents with enhanced navigation and indexing.

**Role Support**: Custom roles like ``:psp:`NUM``` for cross-referencing PSPs within documents.

The extension is modular and can be easily extended with additional PSP-specific features.

PSP Types
=========

**Standards Track PSPs** describe new features or implementations for Perspt:

* New UI components or interaction patterns
* Changes to core functionality  
* New command-line options or configuration
* Performance improvements with user-visible changes

**Informational PSPs** provide information without proposing changes:

* Design philosophy documents
* User experience guidelines
* Best practices for Perspt usage
* Compatibility guides

**Process PSPs** describe process changes:

* Changes to the PSP process itself
* Development workflow modifications
* Release processes
* Community governance

PSP Workflow
============

1. **Idea Phase**: Discuss in GitHub Issues
2. **Draft Phase**: Create PSP document using template
3. **Discussion Phase**: Community review and feedback
4. **Decision Phase**: Maintainers accept/reject
5. **Implementation Phase**: Code the approved changes
6. **Final Phase**: Mark as complete

GitHub Integration
==================

The PSP process is designed to integrate with GitHub:

* **Issues** for discussions and proposals
* **Pull Requests** for PSP document changes  
* **Labels** for categorization and status tracking
* **Projects** for workflow management

Future enhancements may include GitHub Actions for automation.

Extending the System
====================

The PSP extension system is designed to be extensible:

**Adding New Features**: Modify the extension in ``psp_sphinx_extensions/`` to add new PSP-specific functionality.

**Theme Customization**: Update the theme files in ``psp_theme/`` to change the appearance and behavior.

**Custom Roles**: Add new reStructuredText roles in ``psp_processor/parsing/`` for enhanced markup.

**Build Customization**: Modify ``build.py`` or the Makefile to add custom build steps or validation.

Getting Help
============

* Read PSP 0001 for detailed process guidelines
* Check existing PSPs for examples
* Ask questions in GitHub Discussions
* Contact maintainers for guidance

Contributing
============

The PSP system itself can be improved through PSPs! If you have ideas for:

* Process improvements
* New extension features  
* Theme enhancements
* Better automation
* Documentation improvements

Please propose them through the standard PSP process.

Development Setup
=================

For developers working on the PSP system itself:

1. Install development dependencies:

   .. code-block:: bash

      cd docs/psps
      uv install --dev

2. Make changes to the extension in ``psp_sphinx_extensions/``

3. Test your changes:

   .. code-block:: bash

      make clean && make html

4. The extension is automatically loaded during the build process

Resources
=========

* `Python PEPs <https://peps.python.org/>`_ - Inspiration for this system
* `Sphinx Documentation <https://www.sphinx-doc.org/>`_ - Documentation framework
* `reStructuredText Guide <https://docutils.sourceforge.io/rst.html>`_ - Markup format
* `GitHub Actions <https://docs.github.com/en/actions>`_ - Automation platform

License
=======

This PSP system and all PSPs are placed in the public domain or under the CC0-1.0-Universal license, whichever is more permissive.
