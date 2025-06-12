"""PSP HTML builders."""

from docutils import nodes
from docutils.frontend import OptionParser
from sphinx.builders.html import StandaloneHTMLBuilder
from sphinx.writers.html import HTMLWriter
from sphinx.builders.dirhtml import DirectoryHTMLBuilder


class FileBuilder(StandaloneHTMLBuilder):
    """File-based HTML builder for PSPs."""
    
    copysource = False  # Prevent unneeded source copying - we link direct to GitHub
    search = False  # Disable search

    def __init__(self, app, env=None):
        super().__init__(app, env)
        # Initialize docsettings
        option_parser = OptionParser(components=(HTMLWriter,))
        self.docsettings = option_parser.get_default_values()

    def prepare_writing(self, _doc_names: set[str]) -> None:
        """Prepare writing."""
        # Call parent method to ensure proper initialization
        super().prepare_writing(_doc_names)

    def get_doc_context(self, docname: str, body: str, _metatags: str) -> dict:
        """Collect items for the template context of a page."""
        # Try to get the table of contents
        try:
            toc = self.render_partial(self.env.get_toc_for(docname, self))["fragment"]
        except AttributeError:
            # Fallback for newer Sphinx versions
            try:
                toc = self.render_partial(self.env.get_toctree_for(docname, self, collapse=False))["fragment"]
            except:
                toc = ""
        
        # Get the document title properly
        title = ""
        if docname in self.env.titles:
            title = self.env.titles[docname].astext()
        
        # Get the base context from the parent
        ctx = super().get_doc_context(docname, body, _metatags)
        
        # Update with our custom values
        ctx.update({
            "docname": docname,
            "body": body,
            "title": title,
            "toc": toc,
            "pagename": docname,
            "description": "Perspt Specification Proposals (PSPs)",
        })
        
        return ctx

    def handle_page(self, pagename: str, addctx: dict, templatename: str = "page.html", outfilename: str = None, event_arg=None) -> None:
        """Handle a page with our custom context."""
        # Use our custom template name
        return super().handle_page(pagename, addctx, templatename)

    def write_doc(self, docname: str, doctree: nodes.document) -> None:
        """Write a document."""
        super().write_doc(docname, doctree)
        
        # If we're writing the PSP index (psp-000000), also create index.html as a redirect
        if docname == "psp-000000":
            self._write_index_redirect()
    
    def _write_index_redirect(self):
        """Create index.html that redirects to the PSP index."""
        redirect_content = """<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8" />
    <meta http-equiv="refresh" content="0; url=psp-000000.html" />
    <title>Perspt Specification Proposals</title>
</head>
<body>
    <p>Redirecting to <a href="psp-000000.html">PSP Index</a>...</p>
</body>
</html>"""
        
        index_path = self.outdir / "index.html"
        with open(index_path, "w", encoding="utf-8") as f:
            f.write(redirect_content)


class DirectoryBuilder(FileBuilder):
    """Directory-based HTML builder for PSPs."""
    
    # sync all overwritten things from DirectoryHTMLBuilder
    name = DirectoryHTMLBuilder.name
    get_target_uri = DirectoryHTMLBuilder.get_target_uri
    get_outfilename = DirectoryHTMLBuilder.get_outfilename
