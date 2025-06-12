"""PSP role for cross-references."""

from docutils import nodes
from sphinx.roles import XRefRole


class PSPRole(XRefRole):
    """Role for PSP cross-references."""
    
    def __init__(self):
        super().__init__(warn_dangling=True)
    
    def process_link(self, env, refnode, has_explicit_title, title, target):
        """Process PSP links."""
        # Convert PSP numbers to proper format (6 digits)
        if not has_explicit_title:
            if target.isdigit():
                psp_num = int(target)
                title = f"PSP {psp_num}"
                target = f"psp-{psp_num:06d}"
            elif target.startswith('0') and target.isdigit():
                # Handle zero-padded numbers
                psp_num = int(target)
                title = f"PSP {psp_num}"
                target = f"psp-{psp_num:06d}"
        
        return title, target
