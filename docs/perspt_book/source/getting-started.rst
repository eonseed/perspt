Getting Started
===============

Welcome to Perspt! This guide will get you up and running with your first AI conversation in just a few minutes.

Prerequisites
-------------

Before installing Perspt, ensure you have the following:

System Requirements
~~~~~~~~~~~~~~~~~~~

.. list-table::
   :widths: 20 80
   :header-rows: 1

   * - Component
     - Requirement
   * - **Operating System**
     - Linux, macOS, or Windows
   * - **Rust Toolchain**
     - Rust 1.82.0 or later
   * - **Terminal**
     - Any modern terminal emulator
   * - **Internet Connection**
     - Required for AI provider API calls

API Keys
~~~~~~~~

You'll need an API key from at least one AI provider:

.. tabs::

   .. tab:: OpenAI

      1. Visit `OpenAI Platform <https://platform.openai.com>`_
      2. Sign up or log in to your account
      3. Navigate to API Keys section
      4. Create a new API key
      5. Copy and save it securely

      .. code-block:: bash

         export OPENAI_API_KEY="sk-your-openai-api-key-here"

   .. tab:: Anthropic

      1. Visit `Anthropic Console <https://console.anthropic.com>`_
      2. Sign up or log in
      3. Go to API Keys
      4. Generate a new key
      5. Save it securely

      .. code-block:: bash

         export ANTHROPIC_API_KEY="your-anthropic-api-key-here"

   .. tab:: Google

      1. Visit `Google AI Studio <https://aistudio.google.com>`_
      2. Create or select a project
      3. Generate API key
      4. Configure authentication

      .. code-block:: bash

         export GOOGLE_API_KEY="your-google-api-key-here"

   .. tab:: Ollama (Local)

      1. Install Ollama from `ollama.ai <https://ollama.ai>`_
      2. Pull a model
      3. Start Ollama service

      .. code-block:: bash

         ollama pull llama3.2
         # Ollama service starts automatically

Quick Installation
------------------

Method 1: From Source (Recommended)
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: bash

   # Clone the repository
   git clone https://github.com/eonseed/perspt.git
   cd perspt

   # Build the project
   cargo build --release

   # Install to your PATH (optional)
   cargo install --path .

   # Or run directly
   ./target/release/perspt

Method 2: Using Cargo
~~~~~~~~~~~~~~~~~~~~~

.. code-block:: bash

   # Install from crates.io (when published)
   cargo install perspt

   # Run Perspt
   perspt

Method 3: Download Binary
~~~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: bash

   # Download the latest release (replace with actual URL)
   curl -L https://github.com/eonseed/perspt/releases/latest/download/perspt-linux-x86_64.tar.gz | tar xz

   # Make executable and move to PATH
   chmod +x perspt
   sudo mv perspt /usr/local/bin/

Your First Conversation
-----------------------

Let's start your first AI conversation with Perspt!

Zero-Config Quick Start
~~~~~~~~~~~~~~~~~~~~~~~

**NEW!** Perspt now features intelligent automatic provider detection. Simply set an environment variable for any supported provider, and Perspt will automatically detect and use it - no additional configuration needed!

.. note::
   **Automatic Provider Detection Priority:**
   
   1. OpenAI (``OPENAI_API_KEY``)
   2. Anthropic (``ANTHROPIC_API_KEY``) 
   3. Google Gemini (``GEMINI_API_KEY``)
   4. Groq (``GROQ_API_KEY``)
   5. Cohere (``COHERE_API_KEY``)
   6. XAI (``XAI_API_KEY``)
   7. DeepSeek (``DEEPSEEK_API_KEY``)
   8. Ollama (no API key needed - auto-detected if running)

.. tabs::

   .. tab:: OpenAI (Recommended)

      .. code-block:: bash

         # Set your API key
         export OPENAI_API_KEY="sk-your-actual-api-key-here"
         
         # Launch Perspt - that's it!
         perspt
         # Automatically uses OpenAI with gpt-4o-mini

   .. tab:: Anthropic Claude

      .. code-block:: bash

         # Set your API key
         export ANTHROPIC_API_KEY="sk-ant-your-key"
         
         # Launch Perspt - zero config needed!
         perspt
         # Automatically uses Anthropic with claude-3-5-sonnet-20241022

   .. tab:: Google Gemini

      .. code-block:: bash

         # Set your API key
         export GEMINI_API_KEY="your-gemini-key"
         
         # Launch Perspt
         perspt
         # Automatically uses Gemini with gemini-1.5-flash

   .. tab:: Ollama (Local)

      .. code-block:: bash

         # Just make sure Ollama is running
         ollama serve
         
         # Launch Perspt (no API key needed!)
         perspt
         # Auto-detects Ollama if no other providers found

Step 1: Set Your API Key (Manual Configuration)
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

If you prefer manual configuration or want to override automatic detection:

.. code-block:: bash

   # For OpenAI (most common)
   export OPENAI_API_KEY="sk-your-actual-api-key-here"

   # Verify it's set
   echo $OPENAI_API_KEY

Step 2: Launch Perspt
~~~~~~~~~~~~~~~~~~~~~

.. code-block:: bash

   # Start with automatic detection (recommended)
   perspt

   # Or specify provider manually
   perspt --provider openai --model gpt-4o-mini

You should see a welcome screen like this:

.. code-block:: text

   ┌──────────────────────────────────────────────────────────┐
   │                     Welcome to Perspt!                   │
   │              Your Terminal's Window to AI                │
   ├──────────────────────────────────────────────────────────┤
   │                                                          │
   │  Provider: OpenAI                                        │
   │  Model: gpt-4o-mini                                      │
   │  Status: Ready                                           │
   │                                                          │
   │  Type your message and press Enter to start chatting!    │
   │  Press Ctrl+C to exit                                    │
   │                                                          │
   └──────────────────────────────────────────────────────────┘

   You: 

Step 3: Start Chatting
~~~~~~~~~~~~~~~~~~~~~~

Type your first message and press Enter:

.. code-block:: text

   You: Hello! Can you explain what Rust is in simple terms?

   Assistant: Hello! Rust is a modern programming language that's designed to be both 
   fast and safe. Here are the key things that make Rust special:

   **Speed**: Rust programs run as fast as C and C++ programs because it compiles 
   directly to machine code.

   **Safety**: Unlike C/C++, Rust prevents common programming errors like accessing 
   invalid memory or data races in concurrent programs.

   **No Garbage Collector**: Rust manages memory automatically without needing a 
   garbage collector, which keeps programs fast and predictable.

   **Growing Ecosystem**: It's increasingly used for web backends, system programming, 
   blockchain, and even WebAssembly applications.

   Think of Rust as giving you the performance of low-level languages like C, but 
   with the safety and ergonomics of higher-level languages like Python or Java.

   You: 

Congratulations! 🎉 You've successfully started your first conversation with Perspt.

Basic Commands
--------------

While chatting, you can use these keyboard shortcuts:

.. list-table::
   :widths: 20 80
   :header-rows: 1

   * - Shortcut
     - Action
   * - **Enter**
     - Send your message
   * - **Ctrl+C**
     - Exit Perspt
   * - **↑/↓ Arrow Keys**
     - Scroll through chat history
   * - **Page Up/Down**
     - Scroll chat quickly
   * - **Ctrl+L**
     - Clear the screen

Switching Models
----------------

You can easily switch between different AI models and providers:

OpenAI Models
~~~~~~~~~~~~~

.. code-block:: bash

   # Use GPT-4
   perspt --model-name gpt-4

   # Use GPT-4 Turbo
   perspt --model-name gpt-4-turbo-preview

   # Use GPT-4o Mini (recommended for most use cases)
   perspt --model-name gpt-4o-mini

   # Use latest GPT-4.1
   perspt --model-name gpt-4.1

Other Providers
~~~~~~~~~~~~~~~

.. code-block:: bash

   # Use Anthropic Claude
   perspt --provider-type anthropic --model-name claude-3-sonnet-20240229

   # Use Google Gemini
   perspt --provider-type google --model-name gemini-pro

   # Use Ollama (Local)
   perspt --provider-type ollama --model-name llama3.2

List Available Models
~~~~~~~~~~~~~~~~~~~~~

.. code-block:: bash

   # See all available models for your provider
   perspt --list-models

Basic Configuration
-------------------

For frequent use, create a configuration file to set your preferences:

Create Config File
~~~~~~~~~~~~~~~~~~

.. code-block:: bash

   # Create a config.json file
   touch config.json

Add your configuration:

.. code-block:: json

   {
     "api_key": "your-api-key-here",
     "default_model": "gpt-4o-mini",
     "default_provider": "openai",
     "provider_type": "openai"
   }

Use Config File
~~~~~~~~~~~~~~~

.. code-block:: bash

   # Use your configuration file
   perspt --config config.json

   # Or place config.json in the same directory as perspt
   perspt

Common First-Time Issues
------------------------

Issue: "API key not found"
~~~~~~~~~~~~~~~~~~~~~~~~~~

**Solution**: Make sure your API key is properly set:

.. code-block:: bash

   # Check if the key is set
   echo $OPENAI_API_KEY

   # If empty, set it again
   export OPENAI_API_KEY="sk-your-key-here"

Issue: "Model not available"
~~~~~~~~~~~~~~~~~~~~~~~~~~~~

**Solution**: Check available models for your provider:

.. code-block:: bash

   # List available models
   perspt --list-models

   # Use a specific model that's available
   perspt --model-name gpt-4o-mini

Issue: "Network connection failed"
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

**Solution**: Check your internet connection and API key permissions:

.. code-block:: bash

   # Test with a simple curl command
   curl -H "Authorization: Bearer $OPENAI_API_KEY" \
        "https://api.openai.com/v1/models"

Issue: Terminal display problems
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

**Solution**: Ensure your terminal supports modern features:

.. code-block:: bash

   # Try a different terminal emulator if needed
   # Update your terminal if it's very old
   # Check terminal size
   echo $COLUMNS x $LINES

Next Steps
----------

Now that you have Perspt running, explore these topics:

.. grid:: 2
   :gutter: 3

   .. grid-item-card:: 📖 User Guide
      :link: user-guide/index
      :link-type: doc

      Learn about advanced features, keyboard shortcuts, and productivity tips.

   .. grid-item-card:: ⚙️ Configuration
      :link: configuration
      :link-type: doc

      Customize Perspt to fit your workflow perfectly.

   .. grid-item-card:: 🔀 Multiple Providers
      :link: user-guide/providers
      :link-type: doc

      Set up and switch between different AI providers.

   .. grid-item-card:: 🛠️ Advanced Features
      :link: user-guide/advanced-features
      :link-type: doc

      Discover powerful features like input queuing and markdown rendering.

Example Conversations
---------------------

Here are some example conversations to inspire your Perspt usage:

Code Review
~~~~~~~~~~~

.. code-block:: text

   You: Can you review this Rust function?

   ```rust
   fn calculate_fibonacci(n: u32) -> u32 {
       if n <= 1 {
           return n;
       }
       calculate_fibonacci(n - 1) + calculate_fibonacci(n - 2)
   }
   ```

   Assistant: I see you've implemented a recursive Fibonacci function. Here's my review:

   **Issues:**
   1. **Performance**: This has exponential time complexity O(2^n) due to repeated calculations
   2. **Stack overflow**: Large values of `n` will cause stack overflow
   3. **Type limitations**: `u32` limits the range of Fibonacci numbers you can calculate

   **Improved version:**
   ```rust
   fn fibonacci_iterative(n: u32) -> u64 {
       if n <= 1 {
           return n as u64;
       }
       
       let (mut a, mut b) = (0u64, 1u64);
       for _ in 2..=n {
           let temp = a + b;
           a = b;
           b = temp;
       }
       b
   }
   ```

   This iterative version is O(n) time and O(1) space, much more efficient!

Technical Explanation
~~~~~~~~~~~~~~~~~~~~~

.. code-block:: text

   You: Explain how async/await works in Rust

   Assistant: Async/await in Rust is a powerful feature for handling concurrent operations. 
   Here's how it works:

   ## The Basics

   **Async Functions**: Return a `Future` instead of executing immediately
   ```rust
   async fn fetch_data() -> String {
       // This doesn't run until awaited
       "data".to_string()
   }
   ```

   **Await**: Actually executes the future
   ```rust
   let result = fetch_data().await;
   ```

   ## Under the Hood

   1. **State Machine**: The compiler transforms async functions into state machines
   2. **Zero-cost**: No heap allocations for simple futures
   3. **Cooperative**: Tasks yield control at await points

   ## Key Concepts

   - **Futures are lazy**: They don't do anything until polled
   - **Executors**: Runtime systems (like Tokio) that drive futures to completion
   - **Non-blocking**: Awaiting doesn't block the thread

   This makes Rust excellent for high-performance concurrent applications!

Troubleshooting
----------------

No Provider Detected
~~~~~~~~~~~~~~~~~~~~

If you see an error message like this when launching Perspt:

.. code-block:: text

   ❌ No LLM provider configured!

   To get started, either:
     1. Set an environment variable for a supported provider:
        • OPENAI_API_KEY=sk-your-key
        • ANTHROPIC_API_KEY=sk-ant-your-key
        • GEMINI_API_KEY=your-key
        # ... (shows all supported providers)

     2. Use command line arguments:
        perspt --provider openai --api-key sk-your-key

**Solution:** Set at least one API key environment variable:

.. code-block:: bash

   # Quick fix - set any supported provider
   export OPENAI_API_KEY="sk-your-actual-key"
   perspt  # Should now auto-detect and start

Provider Priority
~~~~~~~~~~~~~~~~~

If you have multiple API keys set and want to use a specific provider:

.. code-block:: bash

   # Override automatic detection
   perspt --provider anthropic  # Forces Anthropic even if OpenAI key is set
   
   # Or unset other providers temporarily
   unset OPENAI_API_KEY
   export ANTHROPIC_API_KEY="your-key"
   perspt  # Now auto-detects Anthropic

Connection Issues
~~~~~~~~~~~~~~~~~

If Perspt detects your provider but can't connect:

1. **Check your API key**: Ensure it's valid and has sufficient credits
2. **Test your connection**: Try a simple curl request to the provider's API
3. **Check firewall**: Ensure your network allows HTTPS connections
4. **Try Ollama**: For offline usage, install Ollama for local models

.. code-block:: bash

   # Test OpenAI connection
   curl -H "Authorization: Bearer $OPENAI_API_KEY" \
        https://api.openai.com/v1/models

Tips for Success
----------------

1. **Start Simple**: Begin with basic conversations before exploring advanced features
2. **Experiment**: Try different models and providers to find what works best for your use case
3. **Use Configuration**: Set up a config file for your most common settings
4. **Join the Community**: Connect with other Perspt users for tips and support
5. **Stay Updated**: Check for updates regularly to get new features and improvements

.. seealso::

   - :doc:`installation` - Detailed installation instructions
   - :doc:`configuration` - Complete configuration guide
   - :doc:`user-guide/basic-usage` - Everyday usage patterns
   - :doc:`user-guide/troubleshooting` - Common issues and solutions
