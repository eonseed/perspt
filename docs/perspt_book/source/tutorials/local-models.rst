.. _tutorial-local-models:

Local Models with Ollama
========================

Run AI locally with no API keys or internet required.

Why Local Models?
-----------------

.. list-table::
   :widths: 20 80

   * - 🔒 **Privacy**
     - All data stays on your machine
   * - 💰 **Cost**
     - No API fees or usage limits
   * - ⚡ **Offline**
     - Works without internet
   * - 🧪 **Experimentation**
     - Test models freely

Install Ollama
--------------

.. tab-set::

   .. tab-item:: macOS

      .. code-block:: bash

         brew install ollama

   .. tab-item:: Linux

      .. code-block:: bash

         curl -fsSL https://ollama.ai/install.sh | sh

   .. tab-item:: Windows

      Download from `ollama.ai <https://ollama.ai/download>`_

Start Ollama
------------

.. code-block:: bash

   ollama serve

Pull a Model
------------

.. code-block:: bash

   # Recommended for coding
   ollama pull llama3.2        # General purpose
   ollama pull codellama       # Code-focused
   ollama pull deepseek-coder  # Coding specialist
   ollama pull qwen2.5-coder   # Code completion

Use with Perspt
---------------

.. code-block:: bash

   # Chat mode
   perspt chat --model llama3.2

   # Agent mode
   perspt agent --model codellama "Create a Python script"

Model Recommendations
---------------------

.. list-table::
   :header-rows: 1
   :widths: 25 25 50

   * - Task
     - Model
     - Notes
   * - General chat
     - ``llama3.2``
     - Best all-around
   * - Code generation
     - ``codellama:13b``
     - Good for agent mode
   * - Code completion
     - ``qwen2.5-coder``
     - Fast, accurate
   * - Reasoning
     - ``deepseek-coder:33b``
     - Complex tasks

Agent Mode with Local Models
----------------------------

Local models can power SRBN, but with considerations:

.. code-block:: bash

   # Use local for all tiers
   perspt agent \
     --architect-model deepseek-coder:33b \
     --actuator-model codellama:13b \
     --verifier-model llama3.2 \
     --speculator-model llama3.2 \
     "Create a web scraper"

.. admonition:: Performance Note
   :class: warning

   Local models are slower than cloud APIs. For complex agent tasks,
   consider using a capable cloud model for the Architect tier.

Hybrid Approach
---------------

Use cloud for planning, local for execution:

.. code-block:: bash

   perspt agent \
     --architect-model gpt-5.2 \
     --actuator-model codellama:13b \
     "Build an API"

GPU Acceleration
----------------

For faster inference:

.. code-block:: bash

   # Check GPU usage
   ollama ps

   # Most models auto-detect GPU
   # For manual control:
   OLLAMA_GPU_LAYERS=35 ollama serve

Troubleshooting
---------------

**Model not found**:

.. code-block:: bash

   ollama list     # Show installed models
   ollama pull <model>  # Install missing model

**Slow performance**:

- Use smaller models (7B instead of 13B)
- Ensure GPU is being used
- Increase ``OLLAMA_NUM_PARALLEL``

**Connection refused**:

.. code-block:: bash

   # Ensure Ollama is running
   ollama serve

   # Check port (default 11434)
   curl http://localhost:11434/api/tags

See Also
--------

- `Ollama Documentation <https://ollama.ai/docs>`_
- :doc:`first-chat` - Basic usage
- :doc:`agent-mode` - Autonomous coding
