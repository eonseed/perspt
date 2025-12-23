.. _local-models:

Local Models with Ollama
========================

Run AI models locally for privacy and offline use.

Install Ollama
--------------

.. tabs::

   .. tab:: macOS

      .. code-block:: bash

         brew install ollama

   .. tab:: Linux

      .. code-block:: bash

         curl -fsSL https://ollama.ai/install.sh | sh

   .. tab:: Windows

      Download from `ollama.ai <https://ollama.ai>`_

Start Ollama
------------

.. code-block:: bash

   ollama serve

Pull a Model
------------

.. code-block:: bash

   # Recommended: Fast and capable
   ollama pull llama3.2

   # For coding
   ollama pull codellama

   # Verify
   ollama list

Use with Perspt
---------------

.. code-block:: bash

   perspt --provider-type ollama --model llama3.2

Model Recommendations
---------------------

.. list-table::
   :widths: 25 15 25 35
   :header-rows: 1

   * - Model
     - Size
     - RAM
     - Best For
   * - ``llama3.2``
     - 3B
     - 4GB
     - Quick chat, general use
   * - ``codellama``
     - 7B
     - 7GB
     - Code generation
   * - ``mistral``
     - 7B
     - 7GB
     - Balanced performance

Troubleshooting
---------------

**Connection failed**:

.. code-block:: bash

   # Check if Ollama is running
   curl http://localhost:11434/api/tags

   # Restart
   ollama serve

**Slow responses**: Use smaller models (``llama3.2`` vs ``llama3.1:8b``)

**Out of memory**: Close other apps or use lighter models
