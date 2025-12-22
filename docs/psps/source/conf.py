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
    "sphinx.ext.extlinks",
    "sphinx.ext.intersphinx", 
    "sphinx.ext.githubpages",
    "sphinx.ext.graphviz",
    "sphinx.ext.imgconverter",
    "sphinx.ext.mathjax",
    "sphinx_math_dollar",
]

# Try to add PSP extensions if available
try:
    import psp_sphinx_extensions
    extensions.insert(0, "psp_sphinx_extensions")
except ImportError:
    print("Warning: psp_sphinx_extensions not available, continuing without it")
    pass

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
# HTML output settings
# html_math_renderer = "maths_to_html"  # Maths rendering

# Theme settings
_PSE_PATH = _ROOT / "psp_sphinx_extensions"
if _PSE_PATH.exists():
    html_theme_path = [os.fspath(_PSE_PATH)]
    html_theme = "psp_theme"  # The actual theme directory (child of html_theme_path)
else:
    # Fallback to default theme if PSP theme not available
    html_theme = "sphinx_rtd_theme"
    html_theme_path = []
html_use_index = False  # Disable index (we use PSP 0)
html_style = ""  # must be defined here or in theme.conf, but is unused
html_copy_source = False  # Prevent unneeded source copying - we link direct to GitHub
html_show_sourcelink = False  # We use custom source links
html_permalinks = False  # handled in custom transforms
html_baseurl = "https://perspt.dev/psps/"  # to create the CNAME file
html_search_language = None  # Disable search completely
gettext_auto_build = False  # speed-ups

# Theme template relative paths from `confdir`
templates_path = []
_PSE_PATH = _ROOT / "psp_sphinx_extensions"
if _PSE_PATH.exists():
    _template_dir = _PSE_PATH / "psp_theme" / "templates"
    if _template_dir.exists():
        templates_path = [os.fspath(_template_dir)]

# -- Options for LaTeX output ------------------------------------------------

latex_engine = "lualatex"


def get_psp_metadata(filename):
    """Extract metadata from PSP .rst file."""
    metadata = {"PSP": "XXXXXX", "Title": "Untitled PSP", "Author": "Unknown"}
    try:
        with open(filename, "r", encoding="utf-8") as f:
            for _ in range(20):  # Check first 20 lines
                line = f.readline()
                if not line:
                    break
                for key in metadata:
                    if line.startswith(f"{key}:"):
                        val = line.split(":", 1)[1].strip()
                        # Escape special LaTeX characters
                        val = val.replace("&", "\\&")
                        metadata[key] = val
    except Exception:
        pass
    return metadata


# Find all PSP documents and create a separate PDF for each
latex_documents = []
source_dir = Path(__file__).resolve().parent
for psp_path in sorted(source_dir.glob("psp-??????.rst")):
    meta = get_psp_metadata(psp_path)
    # entry: (startdocname, targetname, title, author, theme)
    item = (
        psp_path.stem,
        f"{psp_path.stem}.tex",
        f"PSP {meta['PSP']}: {meta['Title']}",
        meta["Author"],
        "howto",  # 'howto' style is better for single documents than 'manual'
    )
    latex_documents.append(item)

latex_elements = {
    "papersize": "letterpaper",
    "pointsize": "11pt",
    "geometry": "\\usepackage[margin=1in, headheight=14pt]{geometry}",
    "preamble": r"""
\usepackage{fancyhdr}
\usepackage{fontspec}
\usepackage{graphicx}

% Use a monospaced font if available, fallback to courier
\setmonofont{Courier New}[
    BoldFont={Courier New Bold},
    ItalicFont={Courier New Italic},
    BoldItalicFont={Courier New Bold Italic}
]

% Force monospaced throughout
\renewcommand{\familydefault}{\ttdefault}

\fancypagestyle{normal}{
    \fancyhf{}
    \fancyhead[L]{Perspt Specification Proposal}
    \fancyhead[R]{Internet-Draft}
    \fancyfoot[C]{\thepage}
    \renewcommand{\headrulewidth}{0.4pt}
    \renewcommand{\footrulewidth}{0pt}
}
\pagestyle{normal}

% Simplify titles
\usepackage{titling}
\pretitle{\begin{center}\huge\bfseries}
\posttitle{\end{center}\vspace{1em}}
\preauthor{\begin{center}\large}
\postauthor{\end{center}}
\predate{\begin{center}\large}
\postdate{\end{center}}

\setcounter{secnumdepth}{-1}
""",
    "maketitle": r"\maketitle",
    "figure_align": "htbp",
}
