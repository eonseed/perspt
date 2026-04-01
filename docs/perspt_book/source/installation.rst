.. _installation:

Installation
============

System Requirements
-------------------

- **OS**: macOS, Linux, Windows (WSL recommended)
- **Rust**: 1.75+ (for building from source)
- **Terminal**: 256-color support recommended

Install from Source
-------------------

.. code-block:: bash

   # Clone the repository
   git clone https://github.com/eonseed/perspt.git
   cd perspt

   # Build in release mode
   cargo build --release

   # The binary is at target/release/perspt
   # Optionally, copy to your PATH:
   cp target/release/perspt ~/.local/bin/

Install with Cargo
------------------

.. code-block:: bash

   cargo install --path .


Verify Installation
-------------------

.. code-block:: bash

   perspt --version
   # perspt 0.5.6

   perspt --help


Provider Setup
--------------

Set at least one API key:

.. tabs::

   .. tab:: Gemini (recommended)

      .. code-block:: bash

         export GEMINI_API_KEY="your-key"

   .. tab:: OpenAI

      .. code-block:: bash

         export OPENAI_API_KEY="sk-xxx"

   .. tab:: Anthropic

      .. code-block:: bash

         export ANTHROPIC_API_KEY="sk-ant-xxx"

   .. tab:: Ollama (local)

      .. code-block:: bash

         # No API key needed
         ollama serve
         ollama pull llama3.2

See :doc:`user-guide/providers` for all supported providers.


Agent Mode Prerequisites
------------------------

For agent mode, install the tool binaries for your target language:

.. tabs::

   .. tab:: Python

      .. code-block:: bash

         # uv (package manager + project init)
         curl -LsSf https://astral.sh/uv/install.sh | sh

         # ty (type checker / LSP)
         pip install ty

         # pytest (test runner)
         pip install pytest

   .. tab:: Rust

      .. code-block:: bash

         # Rust toolchain
         curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

         # rust-analyzer (LSP)
         rustup component add rust-analyzer

   .. tab:: JavaScript

      .. code-block:: bash

         # Node.js and npm
         # (install via your package manager)

         # TypeScript (LSP)
         npm install -g typescript


Optional: Documentation Build
------------------------------

To build the documentation locally:

.. code-block:: bash

   # Install uv (Python package manager)
   curl -LsSf https://astral.sh/uv/install.sh | sh

   # Build HTML docs
   cd docs/perspt_book && uv run make html

   # Open in browser
   open docs/perspt_book/build/html/index.html
