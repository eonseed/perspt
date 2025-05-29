# 👁️ Perspt: Your Terminal's Window to the AI World 🤖

> "The keyboard hums, the screen aglow,\
>  AI's wisdom, a steady flow.\
>  Will robots take over, it's quite the fright,\
>  Or just provide insights, day and night?\
>  We ponder and chat, with code as our guide,\
>  Is AI our helper or our human pride?"

**Perspt** (pronounced "perspect," short for **Per**sonal **S**pectrum **P**ertaining **T**houghts) is a high-performance command-line interface (CLI) application that gives you a peek into the mind of Large Language Models (LLMs). Built with Rust for speed and reliability, it allows you to chat with various AI models from multiple providers directly in your terminal using the modern **genai** crate's unified API.

## 🎯 Why Perspt?

- **🚀 Latest Model Support**: Built on the modern `genai` crate with support for latest reasoning models like Google's Gemini 2.5 Pro and OpenAI's o1-mini
- **⚡ Real-time Streaming**: Ultra-responsive streaming responses with proper reasoning chunk handling
- **🛡️ Rock-solid Reliability**: Comprehensive panic recovery and error handling that keeps your terminal safe
- **🎨 Beautiful Interface**: Modern terminal UI with markdown rendering and smooth animations
- **🔧 Flexible Configuration**: CLI arguments, environment variables, and JSON config files all work seamlessly

## ✨ Features

-   **🎨 Interactive Chat Interface:** A colorful and responsive chat interface powered by Ratatui with smooth scrolling and markdown rendering.
-   **⚡ Advanced Streaming:** Real-time streaming of LLM responses with support for reasoning chunks and proper event handling.
-   **🔀 Latest Provider Support**: Built on the modern `genai` crate with support for cutting-edge models:
    -   **OpenAI** (GPT-4, GPT-4-turbo, GPT-3.5-turbo, GPT-4o, GPT-4o-mini, **GPT-4.1**, **o1-mini**, **o1-preview**, **o3-mini**, and more)
    -   **Anthropic** (Claude-3 Opus, Sonnet, Haiku, Claude-3.5 Sonnet, Claude-3.5 Haiku, and more)
    -   **Google Gemini** (Gemini-1.5-pro, Gemini-1.5-flash, **Gemini-2.0-flash**, **Gemini-2.5-Pro**, and more)
    -   **Mistral** (Mistral-tiny, small, medium, large, Mistral-nemo, Mixtral models, and more)
    -   **Perplexity** (Sonar, Sonar-pro, Sonar-reasoning, and more)
    -   **DeepSeek** (DeepSeek-chat, DeepSeek-reasoner, and more)
    -   **AWS Bedrock** (Amazon Nova models, and more)
-   **�️ Robust CLI Options**: Full command-line support for API keys, models, and provider types that actually work.
-   **🔄 Flexible Authentication**: API keys work via CLI arguments, environment variables, or configuration files.
-   **⚙️ Smart Configuration:** Intelligent configuration loading with fallbacks and validation.
-   **🔄 Input Queuing:** Type and submit new questions even while the AI is generating a previous response.
-   **💅 Enhanced UI Feedback:** Visual indicators for processing states and improved responsiveness.
-   **📜 Advanced Markdown Rendering:** Full markdown support with proper streaming buffer management.
-   **🛡️ Bulletproof Error Handling:** Comprehensive panic recovery, network resilience, and user-friendly error messages.
-   **📚 Extensive Documentation:** Comprehensive code documentation and user guides.

## 🚀 Getting Started

### 🛠️ Prerequisites

-   **Rust:** Ensure you have the Rust toolchain installed. Get it from [rustup.rs](https://rustup.rs/).
-   **🔑 LLM API Key:** For API providers, you'll need an API key from the respective provider:
    -   **OpenAI**: Get yours at [platform.openai.com](https://platform.openai.com) (supports o1-mini, o1-preview, o3-mini, GPT-4.1)
    -   **Anthropic**: Get yours at [console.anthropic.com](https://console.anthropic.com)
    -   **Google Gemini**: Get yours at [aistudio.google.com](https://aistudio.google.com) (supports Gemini 2.5 Pro)
    -   **Mistral**: Get yours at [console.mistral.ai](https://console.mistral.ai)
    -   **Perplexity**: Get yours at [www.perplexity.ai/settings/api](https://www.perplexity.ai/settings/api)
    -   **DeepSeek**: Get yours at [platform.deepseek.com](https://platform.deepseek.com)

### 📦 Installation

1.  **Clone the Repository:**
    ```bash
    git clone <repository-url> # Replace <repository-url> with the actual URL
    cd perspt
    ```

2.  **Build the Project:**
    ```bash
    cargo build --release
    ```
    Find the executable in the `target/release` directory.

3.  **Quick Test (Optional):**
    ```bash
    # Test with OpenAI (replace with your API key)
    ./target/release/perspt --provider-type openai --api-key sk-your-key --model gpt-4o-mini
    
    # Test with Google Gemini (supports latest models)
    ./target/release/perspt --provider-type google --api-key your-key --model gemini-2.0-flash-exp
    
    # Test with Anthropic
    ./target/release/perspt --provider-type anthropic --api-key your-key --model claude-3-5-sonnet-20241022
    ```

### ⚙️ Configuration

Perspt can be configured using a `config.json` file or command-line arguments. Command-line arguments override config file settings.

#### 📝 Config File (`config.json`)

Create a `config.json` in the root directory of the project, or specify a custom path using the `-c` CLI argument.

**Example `config.json`:**

```json
{
  "providers": {
    "openai": "https://api.openai.com/v1",
    "anthropic": "https://api.anthropic.com",
    "google": "https://generativelanguage.googleapis.com/v1beta/",
    "mistral": "https://api.mistral.ai/v1",
    "perplexity": "https://api.perplexity.ai",
    "deepseek": "https://api.deepseek.com/v1",
    "aws-bedrock": "https://bedrock.amazonaws.com"
  },
  "provider_type": "openai",
  "default_provider": "openai",
  "default_model": "gpt-4o-mini",
  "api_key": "your-api-key-here"
}
```

**Configuration Fields:**

-   **`providers`** (Optional): A map of provider profile names to their API base URLs.
-   **`provider_type`**: The type of LLM provider to use. 
    -   Valid values: `"openai"`, `"anthropic"`, `"google"`, `"mistral"`, `"perplexity"`, `"deepseek"`, `"aws-bedrock"`
-   **`default_provider`** (Optional): The name of the provider profile from the `providers` map to use by default.
-   **`default_model`**: The model name to use (e.g., "gpt-4o-mini", "claude-3-5-sonnet-20241022", "gemini-1.5-flash").
-   **`api_key`**: Your API key for the configured provider.

**Example configurations for different providers:**

**OpenAI:**
```json
{
  "provider_type": "openai",
  "default_model": "gpt-4o-mini",
  "api_key": "sk-your-openai-api-key"
}
```

**Anthropic:**
```json
{
    "providers": {
        "anthropic": "https://api.anthropic.com"
    },
    "api_key": "YOUR_ANTHROPIC_API_KEY",
    "default_model": "claude-3-5-sonnet-20241022",
    "provider_type": "anthropic"
}
```

**Google Gemini:**
```json
{
    "providers": {
        "google": "https://generativelanguage.googleapis.com/v1beta/"
    },
    "api_key": "YOUR_GEMINI_API_KEY",
    "default_model": "gemini-1.5-flash",
    "provider_type": "google"
}
```

#### ⌨️ Command-Line Arguments

The CLI now has **fully working** argument support with proper API key handling:

-   `-c <FILE>`, `--config <FILE>`: Path to a custom configuration file.
-   `-p <TYPE>`, `--provider-type <TYPE>`: Specify the provider type (`openai`, `anthropic`, `google`, `mistral`, `perplexity`, `deepseek`, `aws-bedrock`). 
-   `-k <API_KEY>`, `--api-key <API_KEY>`: Your API key for the LLM provider (works properly now!).
-   `-m <MODEL>`, `--model <MODEL>`: The model name (e.g., `gpt-4o-mini`, `o1-mini`, `claude-3-5-sonnet-20241022`, `gemini-2.5-pro`).
-   `--provider <PROVIDER_PROFILE>`: Choose a pre-configured provider profile from your `config.json`'s `providers` map.
-   `--list-models`: List available models for the configured provider.

**✅ Fixed Issues:**
- CLI API keys now properly set environment variables for the genai client
- Model validation works correctly before starting the UI
- Provider type selection is properly handled
- No more "API key only works as environment variable" issues

Run `target/release/perspt --help` for a full list.

### 🏃 Usage Examples

**OpenAI (including latest reasoning models):**
```bash
# Latest GPT-4o-mini (fast and efficient)
target/release/perspt --provider-type openai --api-key YOUR_OPENAI_API_KEY --model gpt-4o-mini

# GPT-4.1 (enhanced capabilities)
target/release/perspt --provider-type openai --api-key YOUR_OPENAI_API_KEY --model gpt-4.1

# OpenAI o1-mini (reasoning model)
target/release/perspt --provider-type openai --api-key YOUR_OPENAI_API_KEY --model o1-mini

# OpenAI o1-preview (advanced reasoning)
target/release/perspt --provider-type openai --api-key YOUR_OPENAI_API_KEY --model o1-preview

# OpenAI o3-mini (latest reasoning model)
target/release/perspt --provider-type openai --api-key YOUR_OPENAI_API_KEY --model o3-mini
```

**Google Gemini (including latest models):**
```bash
# Gemini 2.0 Flash (latest fast model)
target/release/perspt --provider-type google --api-key YOUR_GEMINI_API_KEY --model gemini-2.0-flash-exp

# Gemini 1.5 Pro (balanced performance)
target/release/perspt --provider-type google --api-key YOUR_GEMINI_API_KEY --model gemini-1.5-pro
```

**Anthropic:**
```bash
target/release/perspt --provider-type anthropic --api-key YOUR_ANTHROPIC_API_KEY --model claude-3-5-sonnet-20241022
```

**Using environment variables:**
```bash
# Set once, use multiple times
export OPENAI_API_KEY="your-key-here"
export GOOGLE_API_KEY="your-gemini-key-here"

# Now you can skip the --api-key argument
target/release/perspt --provider-type openai --model gpt-4o-mini
target/release/perspt --provider-type google --model gemini-2.0-flash-exp
```

**Using a config file:**
```bash
target/release/perspt --config my_config.json
```
(Ensure `my_config.json` is correctly set up with `provider_type`, `api_key`, and `default_model`).

### 🎯 Model Discovery & Validation

Perspt now uses the modern **genai** crate for robust model handling and validation:

```bash
# List OpenAI models (including o1-mini, o1-preview, o3-mini, GPT-4.1)
target/release/perspt --provider-type openai --api-key YOUR_API_KEY --list-models

# List Google models (including Gemini 2.5 Pro, 2.0 Flash)
target/release/perspt --provider-type google --api-key YOUR_API_KEY --list-models

# List Anthropic models  
target/release/perspt --provider-type anthropic --api-key YOUR_API_KEY --list-models

# List Mistral models
target/release/perspt --provider-type mistral --api-key YOUR_API_KEY --list-models

# List Perplexity models
target/release/perspt --provider-type perplexity --api-key YOUR_API_KEY --list-models

# List DeepSeek models
target/release/perspt --provider-type deepseek --api-key YOUR_API_KEY --list-models
```

**✅ Enhanced Model Support:**
- **Real Model Validation**: Models are validated before starting the UI to prevent runtime errors
- **Latest Model Support**: Built on genai crate which supports cutting-edge models like o1-mini and Gemini 2.5 Pro
- **Proper Error Handling**: Clear error messages when models don't exist or aren't available
- **Reasoning Model Support**: Full support for models with reasoning capabilities and special event handling

## 🏗️ Architecture & Technical Features

### Built on Modern genai Crate

Perspt has been completely rewritten to use the **genai** crate (v0.3.5), providing:

1. **🎯 Latest Model Support**: Direct support for cutting-edge models including:
   - OpenAI's o1-mini, o1-preview, o3-mini, and GPT-4.1 reasoning models
   - Google's Gemini 2.5 Pro and Gemini 2.0 Flash
   - Latest Claude, Mistral, and other provider models

2. **⚡ Advanced Streaming**: Proper handling of streaming events including:
   - `ChatStreamEvent::Start` - Response initiation
   - `ChatStreamEvent::Chunk` - Regular content chunks  
   - `ChatStreamEvent::ReasoningChunk` - Special reasoning model chunks
   - `ChatStreamEvent::End` - Response completion

3. **🛡️ Robust Error Handling**: Comprehensive error management with:
   - Network failure recovery
   - API authentication validation
   - Model compatibility checking
   - Graceful panic recovery with terminal restoration

4. **🔧 Flexible Configuration**: Multiple configuration methods:
   - CLI arguments (now working properly!)
   - Environment variables
   - JSON configuration files
   - Smart fallbacks and validation

### Key Technical Improvements

- **Fixed CLI Arguments**: API keys and model selection now work correctly via command line
- **Enhanced Streaming**: Improved buffering and event handling for smooth response rendering
- **Better Authentication**: Proper environment variable mapping for different providers
- **Robust UI**: Reduced timeouts and improved responsiveness (50ms vs 100ms)
- **Comprehensive Documentation**: Extensive code documentation and user guides

## 🖐️ Key Bindings

-   `Enter`: Send your input to the LLM or queue it if the LLM is busy.
-   `Esc`: Exit the application safely with proper terminal restoration.
-   `Ctrl+C` / `Ctrl+D`: Exit the application with graceful cleanup.
-   `Up Arrow` / `Down Arrow`: Scroll through chat history smoothly.
-   `Page Up` / `Page Down`: Fast scroll through long conversations.

**✅ UI Improvements:**
- Faster response times with 50ms event timeouts
- Better streaming buffer management for smooth markdown rendering
- Visual feedback during model processing
- Proper terminal restoration on all exit paths

## 🔥 Recent Major Updates (v0.4.0)

### Migration to genai Crate

We've completely migrated from the `allms` crate to the modern **genai** crate (v0.3.5), bringing significant improvements:

**🎯 Fixed Critical Issues:**
1. ✅ **CLI Arguments Now Work**: API keys, models, and provider types work correctly via command line
2. ✅ **Flexible Authentication**: API keys work via CLI, environment variables, or config files
3. ✅ **Responsive UI**: Fixed keystroke waiting issues - UI now responds immediately
4. ✅ **Better Parsing**: Improved markdown rendering with proper streaming buffer management

**🚀 New Features:**
- Support for latest reasoning models (o1-mini, o1-preview, Gemini 2.5 Pro)
- Enhanced streaming with proper reasoning chunk handling
- Comprehensive error handling with terminal restoration
- Model validation before UI startup
- Extensive code documentation and user guides

**🛡️ Reliability Improvements:**
- Bulletproof panic handling that restores terminal state
- Network failure recovery
- Better error messages with troubleshooting tips
- Comprehensive logging for debugging

**🎨 User Experience:**
- Reduced response latency (50ms vs 100ms timeouts)
- Smoother markdown rendering
- Better visual feedback during processing
- Improved chat history navigation

## 🔧 Troubleshooting

### Common Issues & Solutions

**❌ "API key not found" or authentication errors:**
```bash
# Method 1: Use CLI argument (recommended)
perspt --provider-type openai --api-key YOUR_API_KEY --model gpt-4o-mini

# Method 2: Set environment variable
export OPENAI_API_KEY="your-key-here"
export GOOGLE_API_KEY="your-gemini-key-here"
export ANTHROPIC_API_KEY="your-claude-key-here"
```

**❌ "Model not found" errors:**
```bash
# List available models first
perspt --provider-type openai --api-key YOUR_KEY --list-models

# Use exact model names from the list
perspt --provider-type openai --api-key YOUR_KEY --model gpt-4o-mini
```

**❌ Terminal corruption after crash:**
```bash
# Reset terminal (if needed)
reset
stty sane
```

**❌ Permission denied errors:**
```bash
# Make sure the binary is executable
chmod +x target/release/perspt

# Or use cargo run for development
cargo run -- --provider-type openai --api-key YOUR_KEY
```

**❌ Documentation generation errors:**
```bash
# If you see "Unrecognized option" errors when generating docs:
cargo doc --no-deps

# The project includes custom rustdoc styling that's compatible with rustdoc 1.87.0+
```

**✅ Getting Help:**
- Use `--help` for full argument list: `perspt --help`
- Check logs with: `RUST_LOG=debug perspt ...`
- Validate configuration with: `perspt --list-models`
- Test different providers to isolate issues

### Best Practices

1. **Always validate your setup first:**
   ```bash
   perspt --provider-type YOUR_PROVIDER --api-key YOUR_KEY --list-models
   ```

2. **Use environment variables for security:**
   ```bash
   export OPENAI_API_KEY="sk-..."
   perspt --provider-type openai --model gpt-4o-mini
   ```

3. **Start with simple models:**
   ```bash
   # These are reliable and fast
   perspt --provider-type openai --model gpt-4o-mini
   perspt --provider-type google --model gemini-1.5-flash
   ```

4. **Check the logs if issues persist:**
   ```bash
   RUST_LOG=debug perspt --provider-type openai --model gpt-4o-mini
   ```

## 🔄 CI/CD & Releases

This project uses GitHub Actions for comprehensive CI/CD:

### 🧪 Continuous Integration
- **Multi-Platform Testing**: Automated testing on Ubuntu, Windows, and macOS
- **Code Quality**: Automated formatting checks, clippy linting, and security audits
- **Documentation**: Automated building of both Rust API docs and Sphinx documentation

### 📦 Automated Releases
- **Cross-Platform Binaries**: Automatic generation of optimized binaries for:
  - Linux (x86_64)
  - Windows (x86_64)
  - macOS (x86_64 and ARM64)
- **Documentation Packaging**: Complete documentation bundles included in releases
- **Checksum Generation**: SHA256 checksums for all release artifacts

### 📚 Documentation Deployment
- **GitHub Pages**: Automatic deployment of documentation to GitHub Pages
- **Dual Documentation**: Both user guides (Sphinx) and API documentation (rustdoc)
- **Live Updates**: Documentation automatically updates on main branch changes

### 🎯 Getting Pre-built Binaries

Instead of building from source, you can download pre-built binaries from the [releases page](../../releases):

1. Navigate to the latest release
2. Download the appropriate binary for your platform
3. Make it executable: `chmod +x perspt-*` (Linux/macOS)
4. Move to your PATH: `sudo mv perspt-* /usr/local/bin/perspt`

### 📚 Documentation

- **Live Documentation**: [https://eonseed.github.io/perspt/](https://eonseed.github.io/perspt/)
- **User Guide**: Comprehensive tutorials and usage examples
- **API Documentation**: Detailed Rust API documentation

## 🤝 Contributing

Contributions are welcome! Please open issues or submit pull requests for any bugs, features, or improvements.

### Development Workflow
1. Fork the repository
2. Create a feature branch
3. Make your changes with tests
4. Ensure CI passes locally: `cargo test && cargo clippy && cargo fmt --check`
5. Submit a pull request

The CI will automatically test your changes on all supported platforms.

## 📜 License

Perspt is released under the **GNU Lesser General Public License v3.0** (LGPL-3.0). See the [`LICENSE`](LICENSE) file for details.

## ✍️ Author

-   Vikrant Rathore
-   Ronak Rathore
---
Perspt: **Per**sonal **S**pectrum **P**ertaining **T**houghts – the human lens through which we explore the enigma of AI and its implications for humanity.
