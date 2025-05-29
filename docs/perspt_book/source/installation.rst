Installation Guide
==================

This comprehensive guide covers all the ways to install Perspt on your system, from simple binary downloads to building from source.

System Requirements
-------------------

Minimum Requirements
~~~~~~~~~~~~~~~~~~~~

.. list-table::
   :widths: 25 75
   :header-rows: 1

   * - Component
     - Requirement
   * - **Operating System**
     - Linux, macOS, Windows 10+
   * - **Architecture**
     - x86_64, ARM64 (Apple Silicon)
   * - **Memory**
     - 50MB RAM minimum
   * - **Storage**
     - 10MB disk space
   * - **Terminal**
     - Any terminal with UTF-8 support
   * - **Network**
     - Internet connection for AI API calls

Recommended Requirements
~~~~~~~~~~~~~~~~~~~~~~~~

.. list-table::
   :widths: 25 75
   :header-rows: 1

   * - Component
     - Recommendation
   * - **Terminal**
     - Modern terminal with 256+ colors and Unicode support
   * - **Font**
     - Monospace font with good Unicode coverage (e.g., Fira Code, JetBrains Mono)
   * - **Shell**
     - Bash, Zsh, Fish, or PowerShell
   * - **Memory**
     - 100MB+ RAM for optimal performance

Quick Install
-------------

Choose your preferred installation method:

.. tabs::

   .. tab:: 📦 Binary Download (Fastest)

      Download pre-built binaries for immediate use:

      **Linux x86_64:**
      
      .. code-block:: bash

         curl -L https://github.com/yourusername/perspt/releases/latest/download/perspt-linux-x86_64.tar.gz | tar xz
         chmod +x perspt
         sudo mv perspt /usr/local/bin/

      **macOS (Intel):**
      
      .. code-block:: bash

         curl -L https://github.com/yourusername/perspt/releases/latest/download/perspt-darwin-x86_64.tar.gz | tar xz
         chmod +x perspt
         sudo mv perspt /usr/local/bin/

      **macOS (Apple Silicon):**
      
      .. code-block:: bash

         curl -L https://github.com/yourusername/perspt/releases/latest/download/perspt-darwin-arm64.tar.gz | tar xz
         chmod +x perspt
         sudo mv perspt /usr/local/bin/

      **Windows:**
      
      .. code-block:: powershell

         # Download from GitHub releases page
         # Extract perspt.exe and add to PATH

   .. tab:: 🦀 Cargo Install

      Install using Rust's package manager:

      .. code-block:: bash

         # Install from crates.io
         cargo install perspt

         # Or install the latest development version
         cargo install --git https://github.com/yourusername/perspt

   .. tab:: 🏗️ Build from Source

      Build the latest version from source:

      .. code-block:: bash

         # Clone repository
         git clone https://github.com/yourusername/perspt.git
         cd perspt

         # Build release version
         cargo build --release

         # Install to cargo bin
         cargo install --path .

Package Managers
----------------

Homebrew (macOS/Linux)
~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: bash

   # Add tap (when available)
   brew tap yourusername/perspt
   
   # Install
   brew install perspt

   # Update
   brew upgrade perspt

Scoop (Windows)
~~~~~~~~~~~~~~~

.. code-block:: powershell

   # Add bucket (when available)
   scoop bucket add perspt https://github.com/yourusername/scoop-perspt
   
   # Install
   scoop install perspt

   # Update
   scoop update perspt

Chocolatey (Windows)
~~~~~~~~~~~~~~~~~~~~

.. code-block:: powershell

   # Install (when available)
   choco install perspt

   # Update
   choco upgrade perspt

APT (Debian/Ubuntu)
~~~~~~~~~~~~~~~~~~~

.. code-block:: bash

   # Add repository (when available)
   curl -fsSL https://releases.perspt.dev/gpg | sudo gpg --dearmor -o /usr/share/keyrings/perspt.gpg
   echo "deb [signed-by=/usr/share/keyrings/perspt.gpg] https://releases.perspt.dev/apt stable main" | sudo tee /etc/apt/sources.list.d/perspt.list

   # Install
   sudo apt update
   sudo apt install perspt

RPM (Red Hat/Fedora)
~~~~~~~~~~~~~~~~~~~~

.. code-block:: bash

   # Add repository (when available)
   sudo dnf config-manager --add-repo https://releases.perspt.dev/rpm/perspt.repo
   
   # Install
   sudo dnf install perspt

Building from Source
--------------------

Prerequisites
~~~~~~~~~~~~~

.. code-block:: bash

   # Install Rust (if not already installed)
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   source $HOME/.cargo/env

   # Verify installation
   rustc --version
   cargo --version

Clone and Build
~~~~~~~~~~~~~~~

.. code-block:: bash

   # Clone the repository
   git clone https://github.com/yourusername/perspt.git
   cd perspt

   # Build in release mode
   cargo build --release

   # The binary will be in target/release/perspt
   ./target/release/perspt --version

Install System-Wide
~~~~~~~~~~~~~~~~~~~

.. code-block:: bash

   # Option 1: Using cargo install
   cargo install --path .

   # Option 2: Manual installation
   sudo cp target/release/perspt /usr/local/bin/
   sudo chmod +x /usr/local/bin/perspt

   # Option 3: User-local installation
   mkdir -p ~/.local/bin
   cp target/release/perspt ~/.local/bin/
   echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.bashrc
   source ~/.bashrc

Development Build
~~~~~~~~~~~~~~~~~

For development and testing:

.. code-block:: bash

   # Clone with all development tools
   git clone https://github.com/yourusername/perspt.git
   cd perspt

   # Install development dependencies
   cargo install cargo-watch cargo-edit

   # Build in debug mode
   cargo build

   # Run tests
   cargo test

   # Run with hot reload during development
   cargo watch -x run

Docker Installation
-------------------

Run Perspt in a Docker container:

Basic Usage
~~~~~~~~~~~

.. code-block:: bash

   # Pull the image
   docker pull ghcr.io/yourusername/perspt:latest

   # Run interactively
   docker run -it --rm \
     -e OPENAI_API_KEY="$OPENAI_API_KEY" \
     ghcr.io/yourusername/perspt:latest

With Configuration
~~~~~~~~~~~~~~~~~~

.. code-block:: bash

   # Create a config directory
   mkdir -p ~/.config/perspt

   # Create your config.json
   cat > ~/.config/perspt/config.json << EOF
   {
     "api_key": "your-api-key-here",
     "default_model": "gpt-4o-mini",
     "default_provider": "openai"
   }
   EOF

   # Run with mounted config
   docker run -it --rm \
     -v ~/.config/perspt:/app/config \
     ghcr.io/yourusername/perspt:latest \
     --config /app/config/config.json

Docker Compose
~~~~~~~~~~~~~~

Create a `docker-compose.yml` file:

.. code-block:: yaml

   version: '3.8'
   services:
     perspt:
       image: ghcr.io/yourusername/perspt:latest
       stdin_open: true
       tty: true
       environment:
         - OPENAI_API_KEY=${OPENAI_API_KEY}
       volumes:
         - ./config:/app/config
       command: ["--config", "/app/config/config.json"]

Run with:

.. code-block:: bash

   docker-compose run --rm perspt

Platform-Specific Instructions
------------------------------

Linux
~~~~~

**Ubuntu/Debian:**

.. code-block:: bash

   # Update package list
   sudo apt update

   # Install dependencies for building (if building from source)
   sudo apt install build-essential curl git

   # Install Rust
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   source $HOME/.cargo/env

   # Install Perspt
   cargo install perspt

**Arch Linux:**

.. code-block:: bash

   # Install from AUR (when available)
   yay -S perspt

   # Or build from source
   sudo pacman -S rust git
   git clone https://github.com/yourusername/perspt.git
   cd perspt
   cargo build --release

**CentOS/RHEL/Fedora:**

.. code-block:: bash

   # Install Rust
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   source $HOME/.cargo/env

   # Install development tools
   sudo dnf groupinstall "Development Tools"
   sudo dnf install git

   # Install Perspt
   cargo install perspt

macOS
~~~~~

**Using Homebrew (Recommended):**

.. code-block:: bash

   # Install Homebrew if not already installed
   /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"

   # Install Rust
   brew install rust

   # Install Perspt
   cargo install perspt

**Using MacPorts:**

.. code-block:: bash

   # Install Rust
   sudo port install rust

   # Install Perspt
   cargo install perspt

**Manual Installation:**

.. code-block:: bash

   # Install Xcode command line tools
   xcode-select --install

   # Install Rust
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   source $HOME/.cargo/env

   # Install Perspt
   cargo install perspt

Windows
~~~~~~~

**Using Chocolatey:**

.. code-block:: powershell

   # Install Chocolatey
   Set-ExecutionPolicy Bypass -Scope Process -Force
   [System.Net.ServicePointManager]::SecurityProtocol = [System.Net.ServicePointManager]::SecurityProtocol -bor 3072
   iex ((New-Object System.Net.WebClient).DownloadString('https://community.chocolatey.org/install.ps1'))

   # Install Rust
   choco install rust

   # Install Perspt
   cargo install perspt

**Using Scoop:**

.. code-block:: powershell

   # Install Scoop
   Set-ExecutionPolicy RemoteSigned -Scope CurrentUser
   irm get.scoop.sh | iex

   # Install Rust
   scoop install rust

   # Install Perspt
   cargo install perspt

**Manual Installation:**

1. Download and install Rust from `rustup.rs <https://rustup.rs/>`_
2. Open Command Prompt or PowerShell
3. Run: ``cargo install perspt``

Verification
------------

After installation, verify that Perspt is working correctly:

.. code-block:: bash

   # Check version
   perspt --version

   # Check help
   perspt --help

   # Test basic functionality (requires API key)
   export OPENAI_API_KEY="your-key-here"
   perspt --model-name gpt-3.5-turbo

You should see output similar to:

.. code-block:: text

   perspt 0.4.0
   Your Terminal's Window to the AI World

Updating Perspt
----------------

Cargo Installation
~~~~~~~~~~~~~~~~~~

.. code-block:: bash

   # Update to latest version
   cargo install perspt --force

   # Or update all cargo packages
   cargo install-update -a

Binary Installation
~~~~~~~~~~~~~~~~~~~

.. code-block:: bash

   # Download and replace binary
   curl -L https://github.com/yourusername/perspt/releases/latest/download/perspt-linux-x86_64.tar.gz | tar xz
   sudo mv perspt /usr/local/bin/

Package Managers
~~~~~~~~~~~~~~~~

.. code-block:: bash

   # Homebrew
   brew upgrade perspt

   # APT
   sudo apt update && sudo apt upgrade perspt

   # DNF
   sudo dnf upgrade perspt

   # Chocolatey
   choco upgrade perspt

   # Scoop
   scoop update perspt

Uninstallation
--------------

Cargo Installation
~~~~~~~~~~~~~~~~~~

.. code-block:: bash

   # Uninstall using cargo
   cargo uninstall perspt

Manual Binary
~~~~~~~~~~~~~

.. code-block:: bash

   # Remove binary
   sudo rm /usr/local/bin/perspt

   # Remove configuration (optional)
   rm -rf ~/.config/perspt

Package Managers
~~~~~~~~~~~~~~~~

.. code-block:: bash

   # Homebrew
   brew uninstall perspt

   # APT
   sudo apt remove perspt

   # DNF
   sudo dnf remove perspt

   # Chocolatey
   choco uninstall perspt

   # Scoop
   scoop uninstall perspt

Troubleshooting
---------------

Common Issues
~~~~~~~~~~~~~

**"Command not found" error:**

.. code-block:: bash

   # Check if cargo bin is in PATH
   echo $PATH | grep -q "$HOME/.cargo/bin" && echo "Cargo bin in PATH" || echo "Cargo bin NOT in PATH"

   # Add to PATH if missing
   echo 'export PATH="$HOME/.cargo/bin:$PATH"' >> ~/.bashrc
   source ~/.bashrc

**Permission denied:**

.. code-block:: bash

   # Make sure the binary is executable
   chmod +x /usr/local/bin/perspt

   # Or use without sudo
   mkdir -p ~/.local/bin
   cp perspt ~/.local/bin/
   export PATH="$HOME/.local/bin:$PATH"

**Build failures:**

.. code-block:: bash

   # Update Rust toolchain
   rustup update

   # Clear cargo cache
   cargo clean

   # Rebuild
   cargo build --release

**Missing dependencies on Linux:**

.. code-block:: bash

   # Ubuntu/Debian
   sudo apt install build-essential pkg-config libssl-dev

   # CentOS/RHEL/Fedora
   sudo dnf groupinstall "Development Tools"
   sudo dnf install pkgconfig openssl-devel

Getting Help
~~~~~~~~~~~~

If you encounter issues during installation:

1. **Check the GitHub Issues**: `Issues Page <https://github.com/yourusername/perspt/issues>`_
2. **Join the Discussion**: `GitHub Discussions <https://github.com/yourusername/perspt/discussions>`_
3. **Read the FAQ**: :doc:`user-guide/troubleshooting`
4. **Contact Support**: Create a new issue with:
   - Your operating system and version
   - Rust version (``rustc --version``)
   - Installation method used
   - Complete error message

Next Steps
----------

After successful installation:

1. **Set up API keys**: :doc:`configuration`
2. **Learn basic usage**: :doc:`getting-started`
3. **Explore features**: :doc:`user-guide/index`
4. **Join the community**: `GitHub Discussions <https://github.com/yourusername/perspt/discussions>`_

.. seealso::

   - :doc:`getting-started` - Your first conversation
   - :doc:`configuration` - Setting up API keys and preferences
   - :doc:`user-guide/basic-usage` - Everyday usage patterns
   - :doc:`user-guide/troubleshooting` - Common issues and solutions
