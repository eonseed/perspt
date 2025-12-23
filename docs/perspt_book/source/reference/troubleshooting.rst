.. _reference-troubleshooting:

Troubleshooting
===============

Common issues and solutions.

API Key Issues
--------------

**Error**: ``API key not found``

.. code-block:: bash

   # Check if key is set
   echo $OPENAI_API_KEY

   # Set the key
   export OPENAI_API_KEY="sk-..."

**Error**: ``Invalid API key``

- Verify the key is correct (no extra spaces)
- Check if the key has been revoked
- Regenerate from provider dashboard

Provider Connection
-------------------

**Error**: ``Connection refused``

.. code-block:: bash

   # Check internet connectivity
   curl https://api.openai.com

   # For Ollama, ensure it's running
   ollama serve

**Error**: ``Rate limit exceeded``

- Wait and retry
- Reduce request frequency
- Consider using a different provider

Model Issues
------------

**Error**: ``Model not found``

.. code-block:: bash

   # List available models
   perspt --list-models
   
   # For Ollama
   ollama list

**Error**: ``Context length exceeded``

- Use a model with longer context
- Reduce conversation history
- Clear chat and start fresh

Agent Mode Issues
-----------------

**Agent stuck in retry loop**

1. Check LSP is working:

   .. code-block:: bash

      ty check file.py

2. Lower stability threshold:

   .. code-block:: bash

      perspt agent --stability-threshold 0.5 "task"

3. Check for unsolvable errors in code

**High energy despite clean code**

1. Run tests manually:

   .. code-block:: bash

      pytest -v

2. Check LSP diagnostics
3. Adjust energy weights:

   .. code-block:: bash

      perspt agent --energy-weights "0.5,0.5,1.0" "task"

**Agent aborted unexpectedly**

.. code-block:: bash

   # Check session status
   perspt status

   # Resume if possible
   perspt resume

TUI Issues
----------

**Terminal rendering problems**

- Ensure terminal supports 256 colors
- Try a different terminal emulator
- Check ``$TERM`` environment variable

**Keyboard shortcuts not working**

- Check terminal keybinding conflicts
- Try raw mode: ``perspt --raw``

Configuration Issues
--------------------

**Config file not found**

.. code-block:: bash

   # Create config directory
   mkdir -p ~/.perspt

   # Initialize config
   perspt config --edit

**Invalid config format**

Check TOML syntax:

.. code-block:: bash

   # Validate TOML
   python -c "import tomllib; tomllib.load(open('~/.perspt/config.toml', 'rb'))"

Build Issues
------------

**Compilation errors**

.. code-block:: bash

   # Update Rust
   rustup update

   # Clean and rebuild
   cargo clean
   cargo build --release

**Missing dependencies**

.. code-block:: bash

   # macOS
   brew install openssl pkg-config

   # Ubuntu/Debian
   sudo apt install libssl-dev pkg-config

Performance Issues
------------------

**Slow response times**

- Try Groq for faster inference
- Use smaller models (e.g., ``gemini-3-flash``)
- Check network latency

**High memory usage**

- Reduce conversation history
- Use streaming mode
- Restart application periodically

Debug Mode
----------

Enable verbose logging:

.. code-block:: bash

   perspt -v chat
   perspt -v agent "task"

   # Maximum verbosity
   RUST_LOG=debug perspt chat

Getting Help
------------

1. **Check documentation**: This book
2. **GitHub Issues**: `github.com/eonseed/perspt/issues <https://github.com/eonseed/perspt/issues>`_
3. **Logs**: Check ``~/.perspt/logs/``

See Also
--------

- :doc:`cli-reference` - Command reference
- :doc:`../howto/configuration` - Configuration guide
