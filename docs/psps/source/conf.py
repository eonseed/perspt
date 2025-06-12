# Configuration file for building PSPs using Sphinx.

import os
from pathlib import Path
import sys

_ROOT = Path(__file__).resolve().parent.parent
sys.path.append(os.fspath(_ROOT))

# -- Project information -----------------------------------------------------

project = "PSPs"
master_doc = "contents"

# -- General configuration ---------------------------------------------------

# Add any Sphinx extension module names here, as strings.
extensions = [
    "psp_sphinx_extensions",
    "sphinx.ext.extlinks",
    "sphinx.ext.intersphinx",
    "sphinx.ext.githubpages",
]

# The file extensions of source files. Sphinx uses these suffixes as sources.
source_suffix = {
    ".rst": "psp",
}

# List of patterns (relative to source dir) to include when looking for source files.
include_patterns = [
    # Required for Sphinx
    "contents.rst",
    # PSP files (6-digit format)
    "psp-??????.rst",
    # PSP ancillary files
    "psp-??????/*.rst",
]
# And to ignore when looking for source files.
exclude_patterns = [
    # PSP Template
    "psp-template.rst",
]

# Warn on missing references
nitpicky = True

# Intersphinx configuration
intersphinx_mapping = {
    "python": ("https://docs.python.org/3/", None),
    "sphinx": ("https://www.sphinx-doc.org/", None),
}
intersphinx_disabled_reftypes = []

# sphinx.ext.extlinks
# This config is a dictionary of external sites,
# mapping unique short aliases to a base URL and a prefix.
extlinks = {
    "psp": ("psp-%s.html", "PSP %s"),
    "github-issue": ("https://github.com/perspt/perspt/issues/%s", "issue %s"),
    "github-pr": ("https://github.com/perspt/perspt/pull/%s", "PR %s"),
}

# -- Options for HTML output -------------------------------------------------

_PSE_PATH = _ROOT / "psp_sphinx_extensions"

# HTML output settings
html_math_renderer = "maths_to_html"  # Maths rendering

# Theme settings
html_theme_path = [os.fspath(_PSE_PATH)]
html_theme = "psp_theme"  # The actual theme directory (child of html_theme_path)
html_use_index = False  # Disable index (we use PSP 0)
html_style = ""  # must be defined here or in theme.conf, but is unused
html_copy_source = False  # Prevent unneeded source copying - we link direct to GitHub
html_show_sourcelink = False  # We use custom source links
html_permalinks = False  # handled in custom transforms
html_baseurl = "https://perspt.dev/psps/"  # to create the CNAME file
html_search_language = None  # Disable search completely
gettext_auto_build = False  # speed-ups

# Theme template relative paths from `confdir`
templates_path = [os.fspath(_PSE_PATH / "psp_theme" / "templates")]
