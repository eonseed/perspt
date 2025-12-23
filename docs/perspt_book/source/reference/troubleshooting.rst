.. _troubleshooting:

Troubleshooting
===============

Common issues and solutions.

No Provider Detected
--------------------

**Error**: ``❌ No LLM provider configured!``

**Fix**: Set an API key:

.. code-block:: bash

   export OPENAI_API_KEY="sk-..."
   perspt

API Key Invalid
---------------

**Error**: ``Authentication failed``

**Fix**:

1. Verify key is correct
2. Check key has credits/quota
3. Test with curl:

   .. code-block:: bash

      curl -H "Authorization: Bearer $OPENAI_API_KEY" \
           https://api.openai.com/v1/models

Model Not Found
---------------

**Error**: ``Model not available``

**Fix**: List available models:

.. code-block:: bash

   perspt --provider-type openai --list-models

Connection Timeout
------------------

**Fix**:

1. Check internet connection
2. Provider status page
3. Try different model

Ollama Not Running
------------------

**Error**: ``Connection refused``

**Fix**:

.. code-block:: bash

   ollama serve
   curl http://localhost:11434/api/tags

Terminal Display Issues
-----------------------

**Fix**:

- Use modern terminal (iTerm2, Alacritty, Windows Terminal)
- Check UTF-8 support
- Resize terminal: ``echo $COLUMNS x $LINES``

Agent Mode: LSP Errors
----------------------

**Error**: ``ty not found``

**Fix**: Install ty (Python type checker):

.. code-block:: bash

   pip install ty

Agent Mode: Test Failures
-------------------------

**Error**: ``pytest not found``

**Fix**: Install uv and pytest:

.. code-block:: bash

   pip install uv
   uv sync --dev
