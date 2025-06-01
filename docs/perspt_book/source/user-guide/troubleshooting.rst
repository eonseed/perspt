Troubleshooting
===============

This comprehensive troubleshooting guide helps you diagnose and resolve issues with Perspt's genai crate integration, provider connectivity, and advanced features.

Quick Diagnostics
------------------

Start with these diagnostic commands to check system status:

.. code-block:: bash

   # Check provider connectivity and model availability
   perspt --provider-type openai --list-models
   
   # Validate specific model
   perspt --provider-type anthropic --model claude-3-5-sonnet-20241022 --list-models
   
   # Test with minimal configuration
   perspt --api-key your-key --provider-type openai --model gpt-3.5-turbo

**Environment Variable Check**

.. code-block:: bash

   # Check if API keys are set
   echo $OPENAI_API_KEY
   echo $ANTHROPIC_API_KEY
   echo $GOOGLE_API_KEY
   
   # Verify genai crate can access providers
   export RUST_LOG=debug
   perspt --provider-type openai --list-models

Common Issues
-------------

GenAI Crate Integration Issues
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

**Provider Authentication Failures**

.. code-block:: text

   Error: Authentication failed for provider 'openai'
   Caused by: Invalid API key

**Solutions**:

1. **Verify API key format**:

   .. code-block:: bash

      # OpenAI keys start with 'sk-'
      echo $OPENAI_API_KEY | head -c 5  # Should show 'sk-'
      
      # Anthropic keys start with 'sk-ant-'
      echo $ANTHROPIC_API_KEY | head -c 7  # Should show 'sk-ant-'

2. **Test API key directly**:

   .. code-block:: bash

      # Test OpenAI API key
      curl -H "Authorization: Bearer $OPENAI_API_KEY" \
           https://api.openai.com/v1/models
      
      # Test Anthropic API key
      curl -H "x-api-key: $ANTHROPIC_API_KEY" \
           https://api.anthropic.com/v1/models

3. **Check API key permissions and billing**:
   - Ensure API key has model access permissions
   - Verify account has sufficient credits/billing set up
   - Check for rate limiting or usage quotas

**Model Validation Failures**

.. code-block:: text

   Error: Model 'gpt-4.1' not available for provider 'openai'
   Available models: gpt-3.5-turbo, gpt-4, gpt-4-turbo...

**Solutions**:

1. **Check model availability**:

   .. code-block:: bash

      # List all available models for provider
      perspt --provider-type openai --list-models
      
      # Search for specific model
      perspt --provider-type openai --list-models | grep gpt-4

2. **Use correct model names**:

   .. code-block:: bash

      # Correct model names (case-sensitive)
      perspt --provider-type openai --model gpt-4o-mini       # ✓ Correct
      perspt --provider-type openai --model GPT-4O-Mini       # ✗ Wrong case
      perspt --provider-type openai --model gpt4o-mini        # ✗ Missing hyphen

3. **Check provider-specific model access**:
   - Some models require special access (e.g., GPT-4, Claude Opus)
   - Verify your account tier supports the requested model
   - Check if model is in beta/preview status

**Streaming Connection Issues**

.. code-block:: text

   Error: Streaming connection interrupted
   Caused by: Connection reset by peer

**Solutions**:

1. **Network connectivity check**:

   .. code-block:: bash

      # Test basic connectivity
      ping api.openai.com
      ping api.anthropic.com
      
      # Check for proxy/firewall issues
      curl -I https://api.openai.com/v1/models

2. **Provider service status**:
   - Check OpenAI Status: https://status.openai.com
   - Check Anthropic Status: https://status.anthropic.com
   - Check Google AI Status: https://status.google.com

3. **Adjust streaming settings**:

   .. code-block:: json

      {
        "provider_type": "openai",
        "default_model": "gpt-4o-mini",
        "stream_timeout": 30,
        "retry_attempts": 3,
        "buffer_size": 1024
      }

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
        -d '{"model":"gpt-4o-mini","messages":[{"role":"user","content":"Hello"}]}' \\
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
     "model": "gpt-4o-mini"
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



Provider-Specific Troubleshooting
---------------------------------

OpenAI Provider Issues
~~~~~~~~~~~~~~~~~~~~~~

**Authentication and API Key Problems**

.. code-block:: text

   Error: Invalid API key for OpenAI
   Error: Rate limit exceeded for model gpt-4

**Solutions**:

1. **API Key Validation**:

   .. code-block:: bash

      # Verify OpenAI API key format (should start with 'sk-')
      echo $OPENAI_API_KEY | head -c 3  # Should show 'sk-'
      
      # Test API key with curl
      curl -H "Authorization: Bearer $OPENAI_API_KEY" \
           https://api.openai.com/v1/models

2. **Rate Limiting Management**:

   .. code-block:: bash

      # Use tier-appropriate models
      perspt --provider-type openai --model gpt-3.5-turbo  # Lower tier
      perspt --provider-type openai --model gpt-4o-mini    # Tier 1+
      perspt --provider-type openai --model gpt-4          # Tier 3+

3. **Quota and Billing Issues**:
   - Check OpenAI dashboard for usage limits
   - Verify payment method is valid
   - Monitor usage to avoid unexpected charges

**Model Access Issues**

.. code-block:: text

   Error: Model 'o1-preview' not available
   Error: Insufficient quota for GPT-4

**Solutions**:

1. **Model Tier Requirements**:

   .. code-block:: bash

      # Tier 1 models (widely available)
      perspt --provider-type openai --model gpt-3.5-turbo
      perspt --provider-type openai --model gpt-4o-mini
      
      # Tier 2+ models (higher usage requirements)
      perspt --provider-type openai --model gpt-4
      perspt --provider-type openai --model gpt-4-turbo
      
      # Special access models (invitation/waitlist)
      perspt --provider-type openai --model o1-preview
      perspt --provider-type openai --model o1-mini

2. **Reasoning Model Limitations**:
   - o1 models have special usage patterns
   - Higher latency expected for reasoning
   - May have stricter rate limits

Anthropic Provider Issues
~~~~~~~~~~~~~~~~~~~~~~~~~

**Claude Model Access**

.. code-block:: text

   Error: Model 'claude-3-opus-20240229' not available
   Error: Anthropic API key authentication failed

**Solutions**:

1. **API Key Format**:

   .. code-block:: bash

      # Anthropic keys start with 'sk-ant-'
      echo $ANTHROPIC_API_KEY | head -c 7  # Should show 'sk-ant-'
      
      # Test with curl
      curl -H "x-api-key: $ANTHROPIC_API_KEY" \
           -H "anthropic-version: 2023-06-01" \
           https://api.anthropic.com/v1/models

2. **Model Availability**:

   .. code-block:: bash

      # Generally available models
      perspt --provider-type anthropic --model claude-3-5-sonnet-20241022
      perspt --provider-type anthropic --model claude-3-5-haiku-20241022
      
      # Request access for Opus through Anthropic Console
      perspt --provider-type anthropic --model claude-3-opus-20240229

3. **Rate Limiting**:
   - Anthropic has strict rate limits for new accounts
   - Build up usage history for higher limits
   - Use Haiku model for testing and development

Google AI (Gemini) Provider Issues
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

**API Key and Setup Problems**

.. code-block:: text

   Error: Google AI API key not valid
   Error: Gemini model access denied

**Solutions**:

1. **API Key Configuration**:

   .. code-block:: bash

      # Get API key from Google AI Studio
      export GOOGLE_API_KEY="your-api-key"
      # Alternative environment variable
      export GEMINI_API_KEY="your-api-key"
      
      # Test API access
      curl -H "Content-Type: application/json" \
           "https://generativelanguage.googleapis.com/v1beta/models?key=$GOOGLE_API_KEY"

2. **Model Selection**:

   .. code-block:: bash

      # Recommended models
      perspt --provider-type google --model gemini-1.5-flash     # Fast, cost-effective
      perspt --provider-type google --model gemini-1.5-pro      # Balanced capability
      perspt --provider-type google --model gemini-1.5-pro-exp  # Experimental features

3. **Geographic Restrictions**:
   - Some Gemini models have geographic limitations
   - Check Google AI availability in your region
   - Use VPN if necessary and allowed by Google's terms

Groq Provider Issues
~~~~~~~~~~~~~~~~~~~~

**Service Availability**

.. code-block:: text

   Error: Groq service temporarily unavailable
   Error: Model inference timeout

**Solutions**:

1. **Service Reliability**:
   - Groq prioritizes speed over availability
   - Configure fallback providers for production use
   - Monitor Groq status page for outages

2. **Model Selection**:

   .. code-block:: bash

      # Fast inference models
      perspt --provider-type groq --model llama-3.1-8b-instant
      perspt --provider-type groq --model mixtral-8x7b-32768
      perspt --provider-type groq --model gemma-7b-it

3. **Timeout Configuration**:

   .. code-block:: json

      {
        "provider_type": "groq",
        "timeout": 30,
        "retry_attempts": 2,
        "fallback_provider": "openai"
      }

Cohere Provider Issues
~~~~~~~~~~~~~~~~~~~~~~

**API Integration Problems**

.. code-block:: text

   Error: Cohere API authentication failed
   Error: Model 'command-r-plus' not accessible

**Solutions**:

1. **API Key Setup**:

   .. code-block:: bash

      export COHERE_API_KEY="your-api-key"
      
      # Test API access
      curl -H "Authorization: Bearer $COHERE_API_KEY" \
           https://api.cohere.ai/v1/models

2. **Model Access**:

   .. code-block:: bash

      # Available Cohere models
      perspt --provider-type cohere --model command-r
      perspt --provider-type cohere --model command-r-plus
      perspt --provider-type cohere --model command-light

XAI (Grok) Provider Issues
~~~~~~~~~~~~~~~~~~~~~~~~~~

**Grok Model Access**

.. code-block:: text

   Error: XAI API key invalid
   Error: Grok model not available

**Solutions**:

1. **API Configuration**:

   .. code-block:: bash

      export XAI_API_KEY="your-api-key"
      
      # Check available models
      perspt --provider-type xai --list-models

2. **Model Selection**:

   .. code-block:: bash

      # Available Grok models
      perspt --provider-type xai --model grok-beta

Ollama (Local) Provider Issues
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

**Service Connection Problems**

.. code-block:: text

   Error: Could not connect to Ollama server
   Error: Model not found in Ollama

**Solutions**:

1. **Ollama Service Management**:

   .. code-block:: bash

      # Check if Ollama is running
      curl http://localhost:11434/api/tags
      
      # Start Ollama service
      ollama serve
      
      # Start as background service (macOS)
      brew services start ollama

2. **Model Management**:

   .. code-block:: bash

      # List installed models
      ollama list
      
      # Install popular models
      ollama pull llama3.2:8b
      ollama pull mistral:7b
      ollama pull codellama:7b
      
      # Remove unused models to save space
      ollama rm unused-model

3. **Resource Optimization**:

   .. code-block:: bash

      # Check system resources
      htop
      nvidia-smi  # For GPU users
      
      # Use smaller models for limited resources
      ollama pull llama3.2:3b      # 3B parameters
      ollama pull phi3:mini        # Microsoft Phi-3 Mini

4. **Configuration Tuning**:

   .. code-block:: json

      {
        "provider_type": "ollama",
        "base_url": "http://localhost:11434",
        "options": {
          "num_gpu": 1,           # Number of GPU layers
          "num_thread": 8,        # CPU threads
          "num_ctx": 4096,        # Context window
          "temperature": 0.7,
          "top_p": 0.9
        }
      }

Performance Optimization
------------------------

Response Time Optimization
~~~~~~~~~~~~~~~~~~~~~~~~~~

**Model Selection for Speed**

.. code-block:: bash

   # Fastest models by provider
   perspt --provider-type groq --model llama-3.1-8b-instant     # Groq (fastest)
   perspt --provider-type openai --model gpt-4o-mini            # OpenAI (fast)
   perspt --provider-type google --model gemini-1.5-flash       # Google (fast)
   perspt --provider-type anthropic --model claude-3-5-haiku-20241022  # Anthropic (fast)

**Configuration Tuning**

.. code-block:: json

   {
     "performance": {
       "max_tokens": 1000,           # Limit response length
       "stream": true,               # Enable streaming
       "timeout": 15,                # Shorter timeout
       "parallel_requests": 2,       # Multiple requests
       "cache_responses": true       # Cache similar queries
     }
   }

Memory and Resource Management
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

**System Resource Monitoring**

.. code-block:: bash

   # Monitor CPU and memory usage
   top -p $(pgrep perspt)
   
   # Monitor network usage
   iftop -i any -f "host api.openai.com"
   
   # Check disk usage for logs and cache
   du -sh ~/.config/perspt/

**Resource Optimization**

.. code-block:: json

   {
     "resource_limits": {
       "max_history_size": 50,       # Limit conversation history
       "cache_size_mb": 100,         # Limit cache size
       "log_rotation_size": "10MB",  # Rotate logs
       "cleanup_interval": "24h"     # Regular cleanup
     }
   }

Network Performance
~~~~~~~~~~~~~~~~~~~

**Connection Optimization**

.. code-block:: json

   {
     "network": {
       "keep_alive": true,           # Reuse connections
       "connection_pool_size": 5,    # Pool connections
       "dns_cache": true,            # Cache DNS lookups
       "compression": true           # Enable compression
     }
   }

**Regional Configuration**

.. code-block:: json

   {
     "provider_endpoints": {
       "openai": "https://api.openai.com",           # US
       "anthropic": "https://api.anthropic.com",     # US
       "google": "https://generativelanguage.googleapis.com"  # Global
     }
   }

Advanced Recovery Procedures
---------------------------

Complete System Reset
~~~~~~~~~~~~~~~~~~~~~

**Full Configuration Reset**

.. code-block:: bash

   # Backup current configuration
   cp -r ~/.config/perspt ~/.config/perspt.backup.$(date +%Y%m%d)
   
   # Remove all Perspt data
   rm -rf ~/.config/perspt/
   rm -rf ~/.local/share/perspt/
   rm -rf ~/.cache/perspt/
   
   # Clear temporary files
   rm -rf /tmp/perspt*
   
   # Recreate default configuration
   perspt --create-default-config

**Selective Reset Options**

.. code-block:: bash

   # Reset only configuration
   rm ~/.config/perspt/config.json
   perspt --setup
   
   # Clear only cache
   rm -rf ~/.config/perspt/cache/
   
   # Clear only conversation history
   rm -rf ~/.config/perspt/history/
   
   # Reset only logs
   rm ~/.config/perspt/*.log

Emergency Fallback Procedures
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

**Provider Fallback Chain**

.. code-block:: json

   {
     "fallback_chain": [
       {
         "provider_type": "openai",
         "model": "gpt-4o-mini",
         "on_failure": "next"
       },
       {
         "provider_type": "anthropic", 
         "model": "claude-3-5-haiku-20241022",
         "on_failure": "next"
       },
       {
         "provider_type": "ollama",
         "model": "llama3.2:8b",
         "on_failure": "fail"
       }
     ]
   }

**Manual Override Mode**

.. code-block:: bash

   # Force specific provider regardless of config
   perspt --force-provider openai --force-model gpt-3.5-turbo
   
   # Use minimal configuration
   perspt --no-config --api-key sk-... --provider-type openai
   
   # Debug mode with maximum verbosity
   perspt --debug --verbose --log-level trace

Data Recovery
~~~~~~~~~~~~~

**Conversation History Recovery**

.. code-block:: bash

   # Check for backup files
   ls ~/.config/perspt/history/*.backup
   
   # Restore from backup
   cp ~/.config/perspt/history/conversation.backup \
      ~/.config/perspt/history/conversation.json
   
   # Export conversations before reset
   perspt --export-history ~/perspt-backup.json

**Configuration Recovery**

.. code-block:: bash

   # Restore from automatic backup
   cp ~/.config/perspt/config.json.backup ~/.config/perspt/config.json
   
   # Recreate from environment variables
   perspt --config-from-env
   
   # Interactive configuration rebuild
   perspt --reconfigure

Version Migration Issues
~~~~~~~~~~~~~~~~~~~~~~~

**Upgrading from allms to genai**

.. code-block:: bash

   # Backup old configuration
   cp ~/.config/perspt/config.json ~/.config/perspt/config.allms.backup
   
   # Run migration script
   perspt --migrate-config
   
   # Manual migration if needed
   perspt --validate-config --fix-issues

**Downgrade Procedures**

.. code-block:: bash

   # Install specific version
   cargo install perspt --version 0.2.0
   
   # Use version-specific configuration
   cp ~/.config/perspt/config.v0.2.0.json ~/.config/perspt/config.json

Emergency Contact and Support
-----------------------------

Critical Issue Escalation
~~~~~~~~~~~~~~~~~~~~~~~~~

For production-critical issues:

1. **Immediate Workarounds**:
   - Switch to backup providers
   - Use local models (Ollama) for offline capability
   - Enable debug logging for detailed diagnosis

2. **Community Support Channels**:
   - GitHub Issues: https://github.com/eonseed/perspt/issues
   - Discord Community: [Link to Discord]
   - Reddit: r/perspt

3. **Enterprise Support**:
   - Priority ticket system
   - Direct developer contact
   - Custom configuration assistance

Issue Documentation Template
~~~~~~~~~~~~~~~~~~~~~~~~~~~~

When reporting issues, include this information:

.. code-block:: text

   **Environment Information:**
   - OS: [macOS 14.1 / Ubuntu 22.04 / Windows 11]
   - Perspt Version: [perspt --version]
   - Installation Method: [cargo / brew / binary]
   
   **Configuration:**
   - Provider: [openai / anthropic / google / etc.]
   - Model: [gpt-4o-mini / claude-3-5-sonnet / etc.]
   - Config file: [attach sanitized config.json]
   
   **Error Details:**
   - Full error message: [exact text]
   - Error code: [if available]
   - Stack trace: [if available]
   
   **Reproduction Steps:**
   1. [Step 1]
   2. [Step 2]
   3. [Error occurs]
   
   **Expected vs Actual Behavior:**
   - Expected: [what should happen]
   - Actual: [what actually happens]
   
   **Additional Context:**
   - Network environment: [corporate / home / proxy]
   - Recent changes: [configuration / system updates]
   - Workarounds attempted: [list what you've tried]

Recovery Verification
~~~~~~~~~~~~~~~~~~~~

After resolving issues, verify system health:

.. code-block:: bash

   # Test basic functionality
   perspt --provider-type openai --model gpt-3.5-turbo --test-connection
   
   # Verify configuration
   perspt --validate-config
   
   # Test streaming
   echo "Hello" | perspt --provider-type openai --model gpt-4o-mini --stream
   
   # Check all providers
   for provider in openai anthropic google groq; do
     echo "Testing $provider..."
     perspt --provider-type $provider --list-models
   done

Related Documentation
--------------------

For additional help:

- :doc:`providers` - Provider-specific configuration and features
- :doc:`advanced-features` - Advanced usage patterns and optimization
- :doc:`../configuration` - Complete configuration reference
- :doc:`../developer-guide/index` - Development and API documentation
- :doc:`../api/index` - API reference and integration guides
