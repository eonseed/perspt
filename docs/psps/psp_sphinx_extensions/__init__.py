"""Sphinx extensions for Perspt Specification Proposals (PSPs)"""

from __future__ import annotations

import html
from pathlib import Path
from typing import TYPE_CHECKING, Any

from docutils.writers.html5_polyglot import HTMLTranslator
from sphinx import environment

from psp_sphinx_extensions.psp_processor.html import (
    psp_html_builder,
    psp_html_translator,
)
from psp_sphinx_extensions.psp_processor.parsing import (
    psp_parser,
    psp_role,
)

if TYPE_CHECKING:
    from sphinx.application import Sphinx


def _update_config_for_builder(app: Sphinx) -> None:
    """Update configuration for the builder."""
    app.env.document_ids = {}
    app.env.settings["builder"] = app.builder.name
    if app.builder.name == "dirhtml":
        app.env.settings["psp_url"] = "psp-{:0>4}/"

    app.connect("build-finished", _post_build)


def _post_build(app: Sphinx, exception: Exception | None) -> None:
    """Post-build tasks."""
    if exception is not None:
        return

    # Create index file
    from pathlib import Path
    if "internal_builder" not in app.tags:
        build_directory = Path(app.outdir)
        builder_name = app.builder.name
        if builder_name == "dirhtml":
            psp_zero_file = build_directory / "psp-0000" / "index.html"
            index_file = build_directory / "index.html"
        else:
            psp_zero_file = build_directory / "psp-0000.html"
            index_file = build_directory / "index.html"

        if psp_zero_file.exists():
            index_file.write_text(psp_zero_file.read_text(encoding="utf-8"), encoding="utf-8")


def set_description(
    app: Sphinx, pagename: str, templatename: str, context: dict[str, Any], doctree
) -> None:
    """Set page description for PSP pages."""
    if not pagename.startswith("psp-"):
        return

    # For now, use a generic description
    context["description"] = "Perspt Specification Proposals (PSPs)"


def setup(app: Sphinx) -> dict[str, bool]:
    """Initialize Sphinx extension."""

    environment.default_settings["psp_url"] = "psp-{:0>4}.html"
    # environment.default_settings["halt_level"] = 2  # Commented out to allow warnings

    # Register plugin logic
    # app.add_builder(psp_html_builder.FileBuilder, override=True)
    # app.add_builder(psp_html_builder.DirectoryBuilder, override=True)

    app.add_source_parser(psp_parser.PSPParser)  # Add PSP transforms

    # app.set_translator("html", psp_html_translator.PSPTranslator)  # Docutils Node Visitor overrides (html builder)
    # app.set_translator("dirhtml", psp_html_translator.PSPTranslator)  # Docutils Node Visitor overrides (dirhtml builder)

    app.add_role("psp", psp_role.PSPRole(), override=True)  # Transform PSP references to links

    # Register event callbacks
    # app.connect("builder-inited", _update_config_for_builder)  # Update configuration values for builder used
    app.connect('html-page-context', set_description)

    # Mathematics rendering
    inline_maths = HTMLTranslator.visit_math, None
    block_maths = HTMLTranslator.visit_math_block, None
    app.add_html_math_renderer("maths_to_html", inline_maths, block_maths)  # Render maths to HTML

    # Parallel safety
    return {"parallel_read_safe": True, "parallel_write_safe": True}
