# 👁️ Perspt: Your Terminal's Window to the AI World 🤖

> "The keyboard hums, the screen aglow,\
>  AI's wisdom, a steady flow.\
>  Will robots take over, it's quite the fright,\
>  Or just provide insights, day and night?\
>  We ponder and chat, with code as our guide,\
>  Is AI our helper or our human pride?"

**Perspt** (pronounced "perspect," short for **Per**sonal **S**pectrum **P**ertaining **T**houghts) is a command-line interface (CLI) application that gives you a peek into the mind of Large Language Models (LLMs). It allows you to chat with various AI models from multiple providers directly in your terminal using a unified API.

## ✨ Features

-   **🎨 Interactive Chat Interface:** A colorful and responsive chat interface powered by Ratatui.
-   **⚡ Streaming Responses:** Real-time streaming of LLM responses for an interactive experience.
-   **🔀 Multiple Provider Support**: Seamlessly switch between different LLM providers:
    -   **OpenAI** (GPT-4, GPT-4-turbo, GPT-3.5-turbo, GPT-4o, GPT-4o-mini, and more)
    -   **Anthropic** (Claude-3 Opus, Sonnet, Haiku, Claude-3.5 Sonnet, Claude-3.5 Haiku, and more)
    -   **Google Gemini** (Gemini-1.5-pro, Gemini-1.5-flash, Gemini-2.0-flash, and more)
    -   **Mistral** (Mistral-tiny, small, medium, large, Mistral-nemo, Mixtral models, and more)
    -   **Perplexity** (Sonar, Sonar-pro, Sonar-reasoning, and more)
    -   **DeepSeek** (DeepSeek-chat, DeepSeek-reasoner, and more)
    -   **AWS Bedrock** (Amazon Nova models, and more)
-   **🚀 Dynamic Model Discovery**: Automatically discovers and validates available models from the allms crate, ensuring you always have access to the latest models without manual updates.
-   **⚙️ Configurable:** Flexible configuration via JSON files or command-line arguments.
-   **🔄 Input Queuing:** Type and submit new questions even while the AI is generating a previous response. Your inputs are queued and processed sequentially.
-   **💅 UI Feedback:** The input field is visually disabled during active LLM processing to prevent accidental submissions.
-   **📜 Markdown Rendering:** Assistant's responses are rendered with basic Markdown support (headings, inline code, lists, blockquotes) directly in the terminal.
-   **🛡️ Graceful Error Handling:** Handles network issues, API errors, and JSON parsing.

## 🚀 Getting Started

### 🛠️ Prerequisites

-   **Rust:** Ensure you have the Rust toolchain installed. Get it from [rustup.rs](https://rustup.rs/).
-   **🔑 LLM API Key:** For API providers, you'll need an API key from the respective provider:
    -   OpenAI: Get yours at [platform.openai.com](https://platform.openai.com)
    -   Anthropic: Get yours at [console.anthropic.com](https://console.anthropic.com)
    -   Google Gemini: Get yours at [aistudio.google.com](https://aistudio.google.com)
    -   Mistral: Get yours at [console.mistral.ai](https://console.mistral.ai)
    -   Perplexity: Get yours at [www.perplexity.ai/settings/api](https://www.perplexity.ai/settings/api)
    -   DeepSeek: Get yours at [platform.deepseek.com](https://platform.deepseek.com)

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

Key arguments include:

-   `-c <FILE>`, `--config <FILE>`: Path to a custom configuration file.
-   `-p <TYPE>`, `--provider-type <TYPE>`: Specify the provider type (`openai`, `anthropic`, `google`, `mistral`, `perplexity`, `deepseek`, `aws-bedrock`). 
-   `-k <API_KEY>`, `--api-key <API_KEY>`: Your API key for the LLM provider.
-   `-m <MODEL>`, `--model <MODEL>`: The model name (e.g., `gpt-4o-mini`, `claude-3-5-sonnet-20241022`, `gemini-1.5-flash`).
-   `--provider <PROVIDER_PROFILE>`: Choose a pre-configured provider profile from your `config.json`'s `providers` map.
-   `--list-models`: List available models for the configured provider.

Run `target/release/perspt --help` for a full list.

### 🏃 Usage Examples

**OpenAI:**
```bash
target/release/perspt --provider-type openai --api-key YOUR_OPENAI_API_KEY --model gpt-4o-mini
```

**Anthropic:**
```bash
target/release/perspt --provider-type anthropic --api-key YOUR_ANTHROPIC_API_KEY --model claude-3-5-sonnet-20241022
```

**Google Gemini:**
```bash
target/release/perspt --provider-type google --api-key YOUR_GEMINI_API_KEY --model gemini-1.5-flash
```

**Mistral:**
```bash
target/release/perspt --provider-type mistral --api-key YOUR_MISTRAL_API_KEY --model mistral-nemo
```

**Using a config file:**
```bash
target/release/perspt --config my_config.json
```
(Ensure `my_config.json` is correctly set up with `provider_type`, `api_key`, and `default_model`).

### 🎯 Model Listing

Perspt uses dynamic model discovery powered by the allms crate, which means it automatically stays up-to-date with the latest models from each provider. You can list all currently available models for any provider:

```bash
# List OpenAI models
target/release/perspt --provider-type openai --api-key YOUR_API_KEY --list-models

# List Anthropic models  
target/release/perspt --provider-type anthropic --api-key YOUR_API_KEY --list-models

# List Google models
target/release/perspt --provider-type google --api-key YOUR_API_KEY --list-models

# List Mistral models
target/release/perspt --provider-type mistral --api-key YOUR_API_KEY --list-models

# List Perplexity models
target/release/perspt --provider-type perplexity --api-key YOUR_API_KEY --list-models

# List DeepSeek models
target/release/perspt --provider-type deepseek --api-key YOUR_API_KEY --list-models

# List AWS Bedrock models
target/release/perspt --provider-type aws-bedrock --api-key YOUR_API_KEY --list-models
```

The dynamic model discovery feature ensures that:
- **🔄 Always Current**: New models are automatically available as the allms crate is updated
- **✅ Validated**: Only models actually supported by the allms crate are shown
- **🚀 No Maintenance**: No need to manually update model lists in the code

## 🏗️ Architecture & Technical Features

### Dynamic Model Discovery

Perspt leverages the allms crate's type system to provide dynamic model discovery. Instead of maintaining hardcoded model lists, the application:

1. **Dynamic Validation**: Uses the allms crate's `try_from_str()` methods to validate model names against actual supported models
2. **Automatic Updates**: Benefits from new models and providers added to the allms crate without code changes
3. **Consistent API**: Provides a unified interface across all supported providers
4. **Type Safety**: Leverages Rust's type system to ensure only valid models are used

This approach eliminates the maintenance burden of keeping model lists synchronized with provider updates and ensures users always have access to the latest available models.

## 🖐️ Key Bindings

-   `Enter`: Send your input to the LLM or queue it if the LLM is busy.
-   `Esc`: Exit the application.
-   `Ctrl+C` / `Ctrl+D`: Exit the application.
-   `Up Arrow` / `Down Arrow`: Scroll through chat history.

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

---
Perspt: **Per**sonal **S**pectrum **P**ertaining **T**houghts – the human lens through which we explore the enigma of AI and its implications for humanity.
