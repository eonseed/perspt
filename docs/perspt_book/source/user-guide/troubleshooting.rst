Troubleshooting
===============

This comprehensive troubleshooting guide helps you diagnose and resolve common issues with Perspt.

Quick Diagnostics
------------------

Start with these basic diagnostic commands:

.. code-block:: text

   > /status          # Check current configuration and connectivity
   > /validate-config # Validate configuration file
   > /test-connection # Test provider connectivity
   > /version         # Check Perspt version

Common Issues
-------------

Installation Problems
~~~~~~~~~~~~~~~~~~~~~

**Binary Not Found**

.. code-block:: bash

   perspt: command not found

**Solutions**:

1. Check if Perspt is in your PATH:

   .. code-block:: bash

      echo $PATH
      which perspt

2. Add Perspt to PATH:

   .. code-block:: bash

      # Add to ~/.bashrc, ~/.zshrc, or ~/.config/fish/config.fish
      export PATH="$PATH:/path/to/perspt/binary"

3. Reinstall using package manager:

   .. code-block:: bash

      # Using Homebrew
      brew uninstall perspt
      brew install perspt

**Permission Denied**

.. code-block:: bash

   -bash: ./perspt: Permission denied

**Solution**:

.. code-block:: bash

   chmod +x /path/to/perspt

**Missing Dependencies**

For builds from source:

.. code-block:: bash

   # macOS
   xcode-select --install
   
   # Ubuntu/Debian
   sudo apt update
   sudo apt install build-essential pkg-config libssl-dev
   
   # Fedora/RHEL
   sudo dnf groupinstall "Development Tools"
   sudo dnf install pkg-config openssl-devel

Configuration Issues
~~~~~~~~~~~~~~~~~~~~

**Invalid Configuration File**

.. code-block:: text

   Error: Failed to parse configuration file

**Common causes and solutions**:

1. **JSON Syntax Errors**:

   .. code-block:: bash

      # Validate JSON syntax
      cat config.json | python -m json.tool

   Common syntax errors:

   .. code-block:: json

      {
        "provider": "openai",  // ❌ Comments not allowed in JSON
        "api_key": "sk-...",   // ❌ Trailing comma
      }

   Correct format:

   .. code-block:: json

      {
        "provider": "openai",
        "api_key": "sk-..."
      }

2. **Missing Required Fields**:

   .. code-block:: json

      {
        "provider": "openai"
        // ❌ Missing api_key
      }

   **Solution**: Ensure all required fields are present:

   .. code-block:: json

      {
        "provider": "openai",
        "api_key": "your-api-key",
        "model": "gpt-4"
      }

**Configuration File Not Found**

.. code-block:: text

   Error: Configuration file not found at ~/.config/perspt/config.json

**Solutions**:

1. Create the configuration directory:

   .. code-block:: bash

      mkdir -p ~/.config/perspt

2. Create a basic configuration file:

   .. code-block:: bash

      cat > ~/.config/perspt/config.json << EOF
      {
        "provider": "openai",
        "api_key": "your-api-key",
        "model": "gpt-4"
      }
      EOF

3. Specify a custom configuration path:

   .. code-block:: bash

      perspt --config /path/to/your/config.json

API Connection Issues
~~~~~~~~~~~~~~~~~~~~~

**Invalid API Key**

.. code-block:: text

   Error: Authentication failed - Invalid API key

**Solutions**:

1. **Verify API key format**:

   .. code-block:: bash

      # OpenAI keys start with 'sk-'
      # Anthropic keys start with 'sk-ant-'
      # Check your provider's documentation

2. **Test API key manually**:

   .. code-block:: bash

      # OpenAI
      curl -H "Authorization: Bearer YOUR_API_KEY" \\
           https://api.openai.com/v1/models

      # Anthropic
      curl -H "x-api-key: YOUR_API_KEY" \\
           -H "anthropic-version: 2023-06-01" \\
           https://api.anthropic.com/v1/messages

3. **Check API key permissions**:
   - Ensure the key has necessary permissions
   - Check if the key is associated with the correct organization
   - Verify the key hasn't expired

**Network Connectivity Issues**

.. code-block:: text

   Error: Failed to connect to API endpoint

**Solutions**:

1. **Check internet connectivity**:

   .. code-block:: bash

      ping google.com
      curl -I https://api.openai.com

2. **Verify firewall/proxy settings**:

   .. code-block:: bash

      # Check if behind corporate firewall
      echo $HTTP_PROXY
      echo $HTTPS_PROXY

3. **Test with different endpoints**:

   .. code-block:: bash

      # Try different base URLs
      curl https://api.openai.com/v1/models
      curl https://api.anthropic.com/v1/models

4. **Configure proxy if needed**:

   .. code-block:: json

      {
        "provider": "openai",
        "proxy": {
          "http": "http://proxy.company.com:8080",
          "https": "https://proxy.company.com:8080"
        }
      }

**Rate Limiting**

.. code-block:: text

   Error: Rate limit exceeded

**Solutions**:

1. **Wait and retry**:
   - Most rate limits reset within minutes
   - Implement exponential backoff

2. **Check rate limits**:

   .. code-block:: bash

      # Check OpenAI rate limits
      curl -H "Authorization: Bearer YOUR_API_KEY" \\
           https://api.openai.com/v1/usage

3. **Optimize requests**:

   .. code-block:: json

      {
        "rate_limiting": {
          "requests_per_minute": 50,
          "delay_between_requests": 1.2,
          "max_retries": 3
        }
      }

4. **Upgrade API plan**:
   - Consider higher-tier plans for increased limits
   - Contact provider support for enterprise limits

Model and Response Issues
~~~~~~~~~~~~~~~~~~~~~~~~~

**Model Not Available**

.. code-block:: text

   Error: Model 'gpt-5' not found

**Solutions**:

1. **Check available models**:

   .. code-block:: text

      > /list-models

2. **Verify model name spelling**:

   .. code-block:: json

      {
        "model": "gpt-4-turbo",  // ✓ Correct
        "model": "gpt-4-turob"   // ❌ Typo
      }

3. **Check provider model availability**:
   - Some models may be region-specific
   - Newer models might not be available to all users

**Slow Responses**

**Causes and solutions**:

1. **Large context windows**:

   .. code-block:: json

      {
        "max_tokens": 1000,        // ✓ Reasonable
        "conversation_history_limit": 20  // ✓ Limit history
      }

2. **Network latency**:

   .. code-block:: bash

      # Test latency to provider
      ping api.openai.com

3. **Provider server load**:
   - Check provider status pages
   - Try different models or regions

**Unexpected Responses**

.. code-block:: text

   AI responses seem off-topic or inappropriate

**Solutions**:

1. **Review system prompt**:

   .. code-block:: json

      {
        "system_prompt": "You are a helpful assistant..."  // Clear instructions
      }

2. **Adjust model parameters**:

   .. code-block:: json

      {
        "temperature": 0.3,     // Lower for more focused responses
        "top_p": 0.8,          // Reduce randomness
        "frequency_penalty": 0.2  // Reduce repetition
      }

3. **Clear conversation history**:

   .. code-block:: text

      > /clear

Local Model Issues
~~~~~~~~~~~~~~~~~~

**Ollama Connection Failed**

.. code-block:: text

   Error: Failed to connect to Ollama at localhost:11434

**Solutions**:

1. **Check if Ollama is running**:

   .. code-block:: bash

      # Start Ollama
      ollama serve

      # Check if running
      curl http://localhost:11434/api/tags

2. **Verify model is installed**:

   .. code-block:: bash

      ollama list
      ollama pull llama2:7b  # Install if missing

3. **Check port configuration**:

   .. code-block:: json

      {
        "provider": "ollama",
        "base_url": "http://localhost:11434"  // Correct port
      }

**Insufficient Memory/GPU**

.. code-block:: text

   Error: Out of memory when loading model

**Solutions**:

1. **Use smaller models**:

   .. code-block:: bash

      # Instead of 13B model, use 7B
      ollama pull llama2:7b
      ollama pull mistral:7b

2. **Adjust GPU layers**:

   .. code-block:: json

      {
        "provider": "ollama",
        "options": {
          "num_gpu": 0,     // Use CPU only
          "num_thread": 4   // Limit CPU threads
        }
      }

3. **Monitor system resources**:

   .. code-block:: bash

      # Check memory usage
      htop
      nvidia-smi  # For GPU usage

Platform-Specific Issues
-------------------------

macOS Issues
~~~~~~~~~~~~

**Gatekeeper Blocking Execution**

.. code-block:: text

   "perspt" cannot be opened because it is from an unidentified developer

**Solution**:

.. code-block:: bash

   sudo xattr -rd com.apple.quarantine /path/to/perspt

**Homebrew Installation Issues**

.. code-block:: bash

   # Update Homebrew
   brew update
   brew upgrade

   # Clear caches
   brew cleanup

   # Reinstall if needed
   brew uninstall perspt
   brew install perspt

Linux Issues
~~~~~~~~~~~~

**Missing Shared Libraries**

.. code-block:: text

   error while loading shared libraries: libssl.so.1.1

**Solutions**:

.. code-block:: bash

   # Ubuntu/Debian
   sudo apt update
   sudo apt install libssl1.1 libssl-dev

   # Fedora/RHEL
   sudo dnf install openssl-libs openssl-devel

   # Check library dependencies
   ldd /path/to/perspt

**Permission Issues**

.. code-block:: bash

   # Make executable
   chmod +x perspt

   # Install system-wide
   sudo cp perspt /usr/local/bin/

Windows Issues
~~~~~~~~~~~~~~

**PowerShell Execution Policy**

.. code-block:: powershell

   # Check current policy
   Get-ExecutionPolicy

   # Set policy to allow local scripts
   Set-ExecutionPolicy -ExecutionPolicy RemoteSigned -Scope CurrentUser

**Windows Defender False Positive**

1. Add Perspt to Windows Defender exclusions
2. Download from official sources only
3. Verify file hashes if available

Advanced Troubleshooting
------------------------

Debug Mode
~~~~~~~~~~

Enable detailed logging:

.. code-block:: json

   {
     "debug": {
       "enabled": true,
       "log_level": "trace",
       "log_file": "~/.config/perspt/debug.log"
     }
   }

Run with verbose output:

.. code-block:: bash

   perspt --verbose --debug

Log Analysis
~~~~~~~~~~~~

Check log files for detailed error information:

.. code-block:: bash

   # View recent logs
   tail -f ~/.config/perspt/perspt.log

   # Search for specific errors
   grep -i "error" ~/.config/perspt/perspt.log

   # Analyze API calls
   grep -i "api" ~/.config/perspt/debug.log

Network Debugging
~~~~~~~~~~~~~~~~~

**Capture network traffic**:

.. code-block:: bash

   # Using tcpdump (Linux/macOS)
   sudo tcpdump -i any -n host api.openai.com

   # Using netstat
   netstat -an | grep :443

**Test with curl**:

.. code-block:: bash

   # Test OpenAI API
   curl -v -H "Authorization: Bearer YOUR_API_KEY" \\
        -H "Content-Type: application/json" \\
        -d '{"model":"gpt-3.5-turbo","messages":[{"role":"user","content":"Hello"}]}' \\
        https://api.openai.com/v1/chat/completions

Configuration Debugging
~~~~~~~~~~~~~~~~~~~~~~~

**Validate configuration**:

.. code-block:: bash

   # Check JSON syntax
   python -c "import json; print(json.load(open('config.json')))"

   # Validate with Perspt
   perspt --validate-config

**Test minimal configuration**:

.. code-block:: json

   {
     "provider": "openai",
     "api_key": "your-key",
     "model": "gpt-3.5-turbo"
   }

Performance Debugging
~~~~~~~~~~~~~~~~~~~~~

**Monitor resource usage**:

.. code-block:: bash

   # Monitor CPU and memory
   top -p $(pgrep perspt)

   # Monitor disk I/O
   iotop -p $(pgrep perspt)

**Profile network usage**:

.. code-block:: bash

   # Monitor bandwidth usage
   netlimit -p $(pgrep perspt)

Recovery Procedures
-------------------

Reset Configuration
~~~~~~~~~~~~~~~~~~~

1. **Backup current configuration**:

   .. code-block:: bash

      cp ~/.config/perspt/config.json ~/.config/perspt/config.json.backup

2. **Reset to defaults**:

   .. code-block:: bash

      rm ~/.config/perspt/config.json
      perspt --create-config

3. **Restore from backup if needed**:

   .. code-block:: bash

      cp ~/.config/perspt/config.json.backup ~/.config/perspt/config.json

Clear Cache and Data
~~~~~~~~~~~~~~~~~~~~

.. code-block:: bash

   # Clear conversation history
   rm -rf ~/.config/perspt/history/

   # Clear cache
   rm -rf ~/.config/perspt/cache/

   # Clear temporary files
   rm -rf /tmp/perspt*

Complete Reinstallation
~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: bash

   # Remove all Perspt data
   rm -rf ~/.config/perspt/
   rm -rf ~/.local/share/perspt/

   # Uninstall and reinstall
   # (method depends on installation method)

Getting Help
------------

Community Support
~~~~~~~~~~~~~~~~~

- **GitHub Issues**: Report bugs and feature requests
- **Discussions**: Ask questions and share tips
- **Discord/Slack**: Real-time community support

Reporting Issues
~~~~~~~~~~~~~~~

When reporting issues, include:

1. **System information**:

   .. code-block:: bash

      perspt --version
      uname -a  # or systeminfo on Windows

2. **Configuration** (sanitized):

   .. code-block:: json

      {
        "provider": "openai",
        "model": "gpt-4",
        "api_key": "sk-***redacted***"
      }

3. **Error messages** (full text)
4. **Steps to reproduce**
5. **Expected vs actual behavior**

Professional Support
~~~~~~~~~~~~~~~~~~~~

For enterprise users:

- **Priority support tickets**
- **Direct communication channels**
- **Custom configuration assistance**
- **Integration consulting**

Next Steps
----------

If you're still experiencing issues:

- :doc:`../configuration` - Review complete configuration options
- :doc:`providers` - Check provider-specific troubleshooting
- :doc:`../developer-guide/index` - Development and debugging guides
- :doc:`../api/index` - API reference for programmatic troubleshooting
