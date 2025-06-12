"""PSP-specific parser for Sphinx."""

import re
from docutils import nodes
from docutils.parsers.rst import Parser
from sphinx.parsers import RSTParser


class PSPParser(RSTParser):
    """Parser for PSP documents."""
    
    supported = ('psp',)
    
    def __init__(self):
        """Initialize the PSP parser."""
        super().__init__()
        
    def parse(self, inputstring, document):
        """Parse PSP document with preamble formatting."""
        # Process PSP preamble first
        processed_input = self._process_psp_preamble(inputstring)
        # Then use standard RST parsing
        super().parse(processed_input, document)
    
    def _process_psp_preamble(self, text):
        """Process PSP preamble to create proper formatting like Python PEPs."""
        lines = text.split('\n')
        preamble_lines = []
        content_lines = []
        
        # Extract preamble (lines at the start until first blank line)
        in_preamble = True
        preamble_end = 0
        
        for i, line in enumerate(lines):
            if in_preamble:
                if line.strip() == '':
                    # First blank line marks end of preamble
                    in_preamble = False
                    preamble_end = i
                    break
                elif ':' in line and not line.startswith(' '):
                    # This is a preamble field
                    preamble_lines.append(line)
                else:
                    # Not a preamble format, no preamble to process
                    return text
        
        if not preamble_lines:
            return text
            
        # Get content after preamble
        content_lines = lines[preamble_end:]
        
        # Create formatted preamble using raw HTML table for proper two-column layout
        formatted_preamble = []
        formatted_preamble.append('.. raw:: html')
        formatted_preamble.append('')
        formatted_preamble.append('   <div class="psp-preamble-table">')
        formatted_preamble.append('   <table class="psp-preamble">')
        
        # Process each preamble field into table row format
        for line in preamble_lines:
            if ':' in line and line.strip():
                field, value = line.split(':', 1)
                field = field.strip()
                value = value.strip()
                
                # Create table row with proper two-column structure
                formatted_preamble.append(f'   <tr><td class="psp-field">{field}:</td><td class="psp-value">{value}</td></tr>')
        
        formatted_preamble.append('   </table>')
        formatted_preamble.append('   </div>')
        formatted_preamble.append('')
        
        # Combine formatted preamble with content
        result = '\n'.join(formatted_preamble + content_lines)
        return result
