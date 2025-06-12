"""PSP HTML translator."""

from sphinx.writers.html import HTMLTranslator


class PSPTranslator(HTMLTranslator):
    """HTML translator for PSP documents."""
    
    def __init__(self, document, builder):
        """Initialize PSP translator."""
        super().__init__(document, builder)
        
    # Add PSP-specific translation methods here if needed
    # For now, use standard HTML translation
