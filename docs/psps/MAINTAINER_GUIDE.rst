========================
PSP System Maintainer Guide
========================

This guide provides detailed information for maintainers of the Perspt Specification Proposals (PSPs) documentation system.

System Architecture
===================

The PSP system follows the Python PEPs approach with a custom Sphinx extension that provides:

**Custom Extension** (``psp_sphinx_extensions/``)
  - PSP-specific parsing and cross-referencing
  - Custom HTML builders and translators  
  - Specialized roles like ``:psp:`NUM```

**Custom Theme** (``psp_theme/``)
  - Responsive design optimized for PSP documents
  - Accessibility features and proper contrast
  - JavaScript enhancements for tables and color schemes

**Build System**
  - Custom ``build.py`` script for specialized processing
  - Makefile integration with standard Sphinx commands
  - Support for both HTML and PDF output

Directory Structure
==================

.. code-block::

   psp_sphinx_extensions/
   ├── __init__.py                  # Main extension entry point
   ├── psp_theme/                   # Custom theme
   │   ├── theme.conf              # Theme configuration
   │   ├── templates/              # Jinja2 templates
   │   │   └── page.html          # Main page template
   │   └── static/                # CSS, JS, assets
   │       ├── style.css          # Main stylesheet
   │       ├── mq.css             # Media queries
   │       ├── colour_scheme.js   # Theme switching
   │       └── wrap_tables.js     # Table enhancements
   └── psp_processor/              # Core processing logic
       ├── __init__.py
       ├── html/                   # HTML generation
       │   ├── __init__.py
       │   ├── psp_html_builder.py # Custom HTML builder
       │   └── psp_html_translator.py # HTML translator
       └── parsing/                # Parsing and roles
           ├── __init__.py
           ├── psp_parser.py       # PSP document parser
           └── psp_role.py         # Cross-reference roles

Adding New PSPs
==============

1. **Create the PSP file:**

   .. code-block:: bash

      cp source/psp-template.rst source/psp-XXXX-title.rst

2. **Fill out the PSP metadata:**
   - PSP number (coordinate with other maintainers)
   - Title, Author, Status, Type, Created date
   - Discussion-To URL (GitHub issue)

3. **Add to contents:**
   Edit ``source/contents.rst`` to include the new PSP in the appropriate section.

4. **Build and test:**

   .. code-block:: bash

      make clean && make html

5. **Validate formatting:**
   Check for any Sphinx warnings and ensure proper RST formatting.

Customizing the Extension
========================

**Adding New Roles:**

1. Create a new role file in ``psp_processor/parsing/``
2. Register it in ``psp_sphinx_extensions/__init__.py``
3. Update the theme CSS if needed for styling

**Modifying the Theme:**

1. Edit templates in ``psp_theme/templates/``
2. Update stylesheets in ``psp_theme/static/``
3. Test across different browsers and screen sizes
4. Ensure accessibility compliance

**Custom Builders:**

1. Extend ``psp_html_builder.py`` for new output formats
2. Create corresponding translators in ``html/``
3. Register new builders in the extension ``__init__.py``

Build Process
=============

**Standard Build:**

.. code-block:: bash

   make html        # HTML output
   make latexpdf    # PDF output (requires LaTeX)
   make clean       # Clean build directory

**Development Build:**

.. code-block:: bash

   make livehtml    # Auto-rebuild on changes (requires sphinx-autobuild)

**Custom Build Script:**

The ``build.py`` script provides additional processing:

.. code-block:: bash

   python build.py  # Run custom build logic

Troubleshooting
==============

**Common Issues:**

1. **Extension not loading:**
   - Check ``conf.py`` extensions list
   - Verify ``psp_sphinx_extensions`` is in Python path
   - Check for syntax errors in extension files

2. **Theme not applying:**
   - Verify ``html_theme = 'psp_theme'`` in ``conf.py``
   - Check theme.conf syntax
   - Ensure templates are valid Jinja2

3. **Build failures:**
   - Check RST syntax in source files
   - Verify all cross-references are valid
   - Check for missing dependencies

4. **CSS/JS not loading:**
   - Check static file paths in theme
   - Verify browser cache is cleared
   - Check for JavaScript errors in console

**Debugging:**

Enable verbose Sphinx output:

.. code-block:: bash

   sphinx-build -v -b html source build/html

Check extension loading:

.. code-block:: bash

   python -c "import psp_sphinx_extensions; print('Extension loads successfully')"

Maintenance Tasks
================

**Regular Maintenance:**

1. **Update dependencies:** Keep Sphinx and related packages current
2. **Check links:** Validate external URLs in PSPs
3. **Review formatting:** Ensure consistent RST formatting across PSPs
4. **Test builds:** Verify HTML and PDF generation works correctly

**Theme Updates:**

1. **Accessibility:** Regular accessibility audits
2. **Browser compatibility:** Test with current browser versions
3. **Mobile responsiveness:** Verify mobile display
4. **Performance:** Optimize CSS and JavaScript

**Extension Updates:**

1. **Sphinx compatibility:** Test with new Sphinx versions
2. **Python compatibility:** Ensure compatibility with supported Python versions
3. **Feature additions:** Implement new PSP-specific features as needed

Version Management
=================

The extension version is managed in ``psp_sphinx_extensions/__init__.py``:

.. code-block:: python

   __version__ = '1.0.0'

Update the version when making significant changes:

- **Major (X.0.0):** Breaking changes to extension API
- **Minor (0.X.0):** New features, theme updates
- **Patch (0.0.X):** Bug fixes, minor improvements

Release Process
==============

1. **Test thoroughly:**
   - Build all PSPs without warnings
   - Test HTML and PDF output
   - Verify cross-references work
   - Check theme rendering

2. **Update documentation:**
   - Update README.rst with any changes
   - Document new features or changes
   - Update this maintainer guide if needed

3. **Version and tag:**
   - Update extension version
   - Create git tag for release
   - Update changelog/release notes

Quality Assurance
=================

**Code Quality:**
- Follow PEP 8 for Python code
- Use type hints where appropriate
- Add docstrings to all functions/classes
- Write tests for new functionality

**Documentation Quality:**
- Ensure all PSPs follow the template format
- Check for consistent terminology
- Verify all code examples work
- Maintain up-to-date cross-references

**Build Quality:**
- Zero Sphinx warnings (except expected ones)
- Valid HTML output
- Proper PDF generation
- Consistent styling across browsers

Support
=======

For questions about maintaining the PSP system:

1. Review this guide and the Python PEPs repository
2. Check the Sphinx documentation for extension development
3. Test changes thoroughly in a local environment
4. Document any new procedures or discoveries

Contact the development team for guidance on complex changes or architectural decisions.
