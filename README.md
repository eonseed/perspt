# 👁️ Perspt: Your Terminal's Window to the AI World 🤖

> "The keyboard hums, the screen aglow,\
>  AI's wisdom, a steady flow.\
>  Will robots take over, it's quite the fright,\
>  Or just provide insights, day and night?\
>  We ponder and chat, with code as our guide,\
>  Is AI our helper or our human pride?"

**Perspt** (pronounced "perspect," short for **Per**sonal **S**pectrum **P**ertaining **T**houghts) is a high-performance command-line interface (CLI) application that gives you a peek into the mind of Large Language Models (LLMs). Built with Rust for speed and reliability, it allows you to chat with various AI models from multiple providers directly in your terminal using the modern **genai** crate's unified API.

[![Perspt in Action](docs/screencast/perspt_terminal_ui.jpg)](https://github.com/user-attachments/assets/f80f7109-1615-487b-b2a8-b76e16ebf6a7)

## 🎯 Why Perspt?

- **🚀 Latest Model Support**: Built on the modern `genai` crate with support for latest reasoning models like Google's Gemini 2.5 Pro and OpenAI's o1-mini
- **⚡ Real-time Streaming**: Ultra-responsive streaming responses with proper reasoning chunk handling
- **🛡️ Rock-solid Reliability**: Comprehensive panic recovery and error handling that keeps your terminal safe
- **🎨 Beautiful Interface**: Modern terminal UI with markdown rendering and smooth animations
- **🤖 Zero-Config Startup**: Automatic provider detection from environment variables - just set your API key and go!
- **🔧 Flexible Configuration**: CLI arguments, environment variables, and JSON config files all work seamlessly

## ✨ Features

-   **🎨 Interactive Chat Interface:** A colorful and responsive chat interface powered by Ratatui with smooth scrolling and custom markdown rendering.
-   **⚡ Advanced Streaming:** Real-time streaming of LLM responses with support for reasoning chunks and proper event handling.
-   **🤖 Automatic Provider Detection:** Zero-config startup that automatically detects and uses available providers based on environment variables (set `OPENAI_API_KEY`, `ANTHROPIC_API_KEY`, etc. and just run `perspt`!).
-   **🔀 Latest Provider Support**: Built on the modern `genai` crate with support for cutting-edge models:
    -   **OpenAI** (GPT-4, GPT-4-turbo, GPT-3.5-turbo, GPT-4o, GPT-4o-mini, **GPT-4.1**, **o1-mini**, **o1-preview**, **o3-mini**, and more)
    -   **Anthropic** (Claude-3 Opus, Sonnet, Haiku, Claude-3.5 Sonnet, Claude-3.5 Haiku, and more)
    -   **Google Gemini** (Gemini-1.5-pro, Gemini-1.5-flash, **Gemini-2.0-flash**, **Gemini-2.5-Pro**, and more)
    -   **Groq** (Llama models with ultra-fast inference, Mixtral, Gemma, and more)
    -   **Cohere** (Command models, Command-R, Command-R+, and more)
    -   **XAI** (Grok models and more)
    -   **DeepSeek** (DeepSeek-chat, DeepSeek-reasoner, and more)
    -   **Ollama** (Local models: Llama, Mistral, Code Llama, Vicuna, and custom models)
-   **🔧 Robust CLI Options**: Full command-line support for API keys, models, and provider types that actually work.
-   **🔄 Flexible Authentication**: API keys work via CLI arguments, environment variables, or configuration files.
-   **⚙️ Smart Configuration:** Intelligent configuration loading with fallbacks and validation.
-   **🔄 Input Queuing:** Type and submit new questions even while the AI is generating a previous response.
-   **� Conversation Export:** Save your chat conversations to text files using the `/save` command with timestamped filenames.
-   **�💅 Enhanced UI Feedback:** Visual indicators for processing states and improved responsiveness.
-   **📜 Custom Markdown Parser:** Built-in markdown parser optimized for terminal rendering with proper streaming buffer management.
-   **🛡️ Graceful Error Handling:** Robust handling of network issues, API errors, edge cases with user-friendly error messages.
-   **📚 Extensive Documentation:** Comprehensive code documentation and user guides.

## 🚀 Getting Started

### 🤖 Zero-Config Automatic Provider Detection

**NEW!** Perspt now features intelligent automatic provider detection. Simply set an environment variable for any supported provider, and Perspt will automatically detect and use it - no additional configuration needed!

**Priority Detection Order:**
1. OpenAI (`OPENAI_API_KEY`)
2. Anthropic (`ANTHROPIC_API_KEY`) 
3. Google Gemini (`GEMINI_API_KEY`)
4. Groq (`GROQ_API_KEY`)
5. Cohere (`COHERE_API_KEY`)
6. XAI (`XAI_API_KEY`)
7. DeepSeek (`DEEPSEEK_API_KEY`)
8. Ollama (no API key needed - auto-detected if running)

**Quick Start Examples:**

```bash
# Option 1: OpenAI (will be auto-detected and used)
export OPENAI_API_KEY="sk-your-openai-key"
./target/release/perspt  # That's it! Uses OpenAI with gpt-4o-mini

# Option 2: Anthropic (will be auto-detected and used)
export ANTHROPIC_API_KEY="sk-ant-your-key"
./target/release/perspt  # Uses Anthropic with claude-3-5-sonnet-20241022

# Option 3: Google Gemini (will be auto-detected and used)
export GEMINI_API_KEY="your-gemini-key"
./target/release/perspt  # Uses Gemini with gemini-1.5-flash

# Option 4: Ollama (no API key needed!)
# Just make sure Ollama is running: ollama serve
./target/release/perspt  # Auto-detects Ollama if no other providers found
```

**What happens behind the scenes:**
- Perspt scans your environment variables for supported provider API keys
- Automatically selects the first available provider (based on priority order)
- Sets appropriate default model for the detected provider
- Starts up immediately - no config files or CLI arguments needed!

**When no providers are detected:**
If no API keys are found, Perspt shows helpful setup instructions:

```bash
❌ No LLM provider configured!

To get started, either:
  1. Set an environment variable for a supported provider:
     • OPENAI_API_KEY=sk-your-key
     • ANTHROPIC_API_KEY=sk-ant-your-key
     # ... (shows all supported providers)

  2. Use command line arguments:
     perspt --provider-type openai --api-key sk-your-key

  3. Create a config.json file with provider settings
```

### **Read the [perspt book](docs/perspt.pdf)** - This illustrated guide walks through the project and explains key Rust concepts

### 🛠️ Prerequisites

-   **Rust:** Ensure you have the Rust toolchain installed. Get it from [rustup.rs](https://rustup.rs/).
-   **🔑 LLM API Key:** For cloud providers, you'll need an API key from the respective provider:
    -   **OpenAI**: Get yours at [platform.openai.com](https://platform.openai.com) (supports o1-mini, o1-preview, o3-mini, GPT-4.1)
    -   **Anthropic**: Get yours at [console.anthropic.com](https://console.anthropic.com)
    -   **Google Gemini**: Get yours at [aistudio.google.com](https://aistudio.google.com) (supports Gemini 2.5 Pro)
    -   **Groq**: Get yours at [console.groq.com](https://console.groq.com)
    -   **Cohere**: Get yours at [dashboard.cohere.com](https://dashboard.cohere.com)
    -   **XAI**: Get yours at [console.x.ai](https://console.x.ai)
    -   **DeepSeek**: Get yours at [platform.deepseek.com](https://platform.deepseek.com)
    -   **Ollama**: For local models, install Ollama from [ollama.ai](https://ollama.ai) (no API key needed)

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
    "gemini": "https://generativelanguage.googleapis.com/v1beta/",
    "groq": "https://api.groq.com/openai/v1",
    "cohere": "https://api.cohere.com/v1",
    "xai": "https://api.x.ai/v1",
    "deepseek": "https://api.deepseek.com/v1",
    "ollama": "http://localhost:11434/v1"
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
    -   Valid values: `"openai"`, `"anthropic"`, `"gemini"`, `"groq"`, `"cohere"`, `"xai"`, `"deepseek"`, `"ollama"`
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
        "gemini": "https://generativelanguage.googleapis.com/v1beta/"
    },
    "api_key": "YOUR_GEMINI_API_KEY",
    "default_model": "gemini-1.5-flash",
    "provider_type": "gemini"
}
```

**Groq:**
```json
{
    "providers": {
        "groq": "https://api.groq.com/openai/v1"
    },
    "api_key": "YOUR_GROQ_API_KEY",
    "default_model": "llama-3.1-70b-versatile",
    "provider_type": "groq"
}
```

**Cohere:**
```json
{
    "providers": {
        "cohere": "https://api.cohere.com/v1"
    },
    "api_key": "YOUR_COHERE_API_KEY",
    "default_model": "command-r-plus",
    "provider_type": "cohere"
}
```

**XAI (Grok):**
```json
{
    "providers": {
        "xai": "https://api.x.ai/v1"
    },
    "api_key": "YOUR_XAI_API_KEY",
    "default_model": "grok-beta",
    "provider_type": "xai"
}
```

**DeepSeek:**
```json
{
    "providers": {
        "deepseek": "https://api.deepseek.com/v1"
    },
    "api_key": "YOUR_DEEPSEEK_API_KEY",
    "default_model": "deepseek-chat",
    "provider_type": "deepseek"
}
```

**Ollama (Local Models):**
```json
{
    "providers": {
        "ollama": "http://localhost:11434/v1"
    },
    "api_key": "not-required",
    "default_model": "llama3.2",
    "provider_type": "ollama"
}
```

#### ⌨️ Command-Line Arguments

The CLI now has **fully working** argument support with proper API key handling:

-   `-c <FILE>`, `--config <FILE>`: Path to a custom configuration file.
-   `-p <TYPE>`, `--provider-type <TYPE>`: Specify the provider type (`openai`, `anthropic`, `gemini`, `groq`, `cohere`, `xai`, `deepseek`, `ollama`). 
-   `-k <API_KEY>`, `--api-key <API_KEY>`: Your API key for the LLM provider (works properly now!).
-   `-m <MODEL>`, `--model <MODEL>`: The model name (e.g., `gpt-4o-mini`, `o1-mini`, `claude-3-5-sonnet-20241022`, `gemini-2.5-pro`, `llama3.2`).
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
target/release/perspt --provider-type gemini --api-key YOUR_GEMINI_API_KEY --model gemini-2.0-flash-exp

# Gemini 1.5 Pro (balanced performance)
target/release/perspt --provider-type gemini --api-key YOUR_GEMINI_API_KEY --model gemini-1.5-pro
```

**Anthropic:**
```bash
target/release/perspt --provider-type anthropic --api-key YOUR_ANTHROPIC_API_KEY --model claude-3-5-sonnet-20241022
```

**Groq (Ultra-fast inference):**
```bash
# Llama models with lightning-fast inference
target/release/perspt --provider-type groq --api-key YOUR_GROQ_API_KEY --model llama-3.1-70b-versatile

# Mixtral model
target/release/perspt --provider-type groq --api-key YOUR_GROQ_API_KEY --model mixtral-8x7b-32768
```

**Cohere:**
```bash
# Command-R+ (latest reasoning model)
target/release/perspt --provider-type cohere --api-key YOUR_COHERE_API_KEY --model command-r-plus

# Command-R (balanced performance)
target/release/perspt --provider-type cohere --api-key YOUR_COHERE_API_KEY --model command-r
```

**XAI (Grok):**
```bash
target/release/perspt --provider-type xai --api-key YOUR_XAI_API_KEY --model grok-beta
```

**DeepSeek:**
```bash
# DeepSeek Chat
target/release/perspt --provider-type deepseek --api-key YOUR_DEEPSEEK_API_KEY --model deepseek-chat

# DeepSeek Reasoner
target/release/perspt --provider-type deepseek --api-key YOUR_DEEPSEEK_API_KEY --model deepseek-reasoner
```

**Ollama (Local Models - No API Key Required!):**
```bash
# First, make sure Ollama is running locally:
# ollama serve

# Llama 3.2 (3B - fast and efficient)
target/release/perspt --provider-type ollama --model llama3.2

# Llama 3.1 (8B - more capable)
target/release/perspt --provider-type ollama --model llama3.1:8b

# Code Llama (for coding tasks)
target/release/perspt --provider-type ollama --model codellama

# Mistral (7B - general purpose)
target/release/perspt --provider-type ollama --model mistral

# Custom model (if you've imported one)
target/release/perspt --provider-type ollama --model your-custom-model
```

**Using environment variables:**
```bash
# Set once, use multiple times
export OPENAI_API_KEY="your-key-here"
export GOOGLE_API_KEY="your-gemini-key-here"
export GROQ_API_KEY="your-groq-key-here"

# Now you can skip the --api-key argument
target/release/perspt --provider-type openai --model gpt-4o-mini
target/release/perspt --provider-type gemini --model gemini-2.0-flash-exp
target/release/perspt --provider-type groq --model llama-3.1-70b-versatile

# Ollama doesn't need API keys
target/release/perspt --provider-type ollama --model llama3.2
```

**Using a config file:**
```bash
target/release/perspt --config my_config.json
```
(Ensure `my_config.json` is correctly set up with `provider_type`, `api_key`, and `default_model`).

### 🎯 Model Discovery & Validation

Perspt uses the modern **genai** crate for robust model handling and validation:

```bash
# List OpenAI models (including o1-mini, o1-preview, o3-mini, GPT-4.1)
target/release/perspt --provider-type openai --api-key YOUR_API_KEY --list-models

# List Google models (including Gemini 2.5 Pro, 2.0 Flash)
target/release/perspt --provider-type gemini --api-key YOUR_API_KEY --list-models

# List Anthropic models  
target/release/perspt --provider-type anthropic --api-key YOUR_API_KEY --list-models

# List Groq models (ultra-fast inference)
target/release/perspt --provider-type groq --api-key YOUR_API_KEY --list-models

# List Cohere models
target/release/perspt --provider-type cohere --api-key YOUR_API_KEY --list-models

# List XAI models
target/release/perspt --provider-type xai --api-key YOUR_API_KEY --list-models

# List DeepSeek models
target/release/perspt --provider-type deepseek --api-key YOUR_API_KEY --list-models

# List Ollama models (local, no API key needed)
target/release/perspt --provider-type ollama --list-models
```

**✅ Enhanced Model Support:**
- **Real Model Validation**: Models are validated before starting the UI to prevent runtime errors
- **Latest Model Support**: Built on genai crate which supports cutting-edge models like o1-mini and Gemini 2.5 Pro
- **Proper Error Handling**: Clear error messages when models don't exist or aren't available
- **Reasoning Model Support**: Full support for models with reasoning capabilities and special event handling

## 💬 Chat Interface & Commands

### Built-in Commands

Perspt includes several built-in commands that you can use during your chat session:

**`/save` - Export Conversation**
```bash
# Save with a timestamped filename (e.g., conversation_1735123456.txt)
/save

# Save with a custom filename
/save my_important_chat.txt
```

The `/save` command exports your entire conversation history (user messages and AI responses) to a plain text file. System messages are excluded from the export. The saved file includes:
- A header with the conversation title
- Timestamped messages in chronological order  
- Raw text content without terminal formatting

**Example saved conversation:**
```
Perspt Conversation
==================
[2024-01-01 12:00:00] User: Hello, how are you?
[2024-01-01 12:00:01] Assistant: Hello! I'm doing well, thank you for asking...

[2024-01-01 12:01:30] User: Can you help me with Python?
[2024-01-01 12:01:31] Assistant: Of course! I'd be happy to help you with Python...
```

### Key Bindings

-   `Enter`: Send your input to the LLM or queue it if the LLM is busy.
-   `Esc`: Exit the application safely with proper terminal restoration.
-   `Ctrl+C` / `Ctrl+D`: Exit the application with graceful cleanup.
-   `Up Arrow` / `Down Arrow`: Scroll through chat history smoothly.
-   `Page Up` / `Page Down`: Fast scroll through long conversations.

**✅ UI Improvements:**
- Faster response times with 50ms event timeouts
- Better streaming buffer management for smooth markdown rendering with custom parser
- Visual feedback during model processing
- Proper terminal restoration on all exit paths

## 🏠 Using Ollama for Local Models

Ollama provides a fantastic way to run AI models locally on your machine without needing API keys or internet connectivity. This is perfect for privacy-conscious users, offline work, or simply experimenting with different models.

### 🛠️ Setting Up Ollama

1. **Install Ollama:**
   ```bash
   # macOS
   brew install ollama
   
   # Linux
   curl -fsSL https://ollama.ai/install.sh | sh
   
   # Or download from: https://ollama.ai
   ```

2. **Start the Ollama service:**
   ```bash
   ollama serve
   ```
   This starts the Ollama server at `http://localhost:11434`

3. **Download models:**
   ```bash
   # Llama 3.2 (3B) - Great balance of speed and capability
   ollama pull llama3.2
   
   # Llama 3.1 (8B) - More capable, slightly slower
   ollama pull llama3.1:8b
   
   # Code Llama - Optimized for coding tasks
   ollama pull codellama
   
   # Mistral - General purpose model
   ollama pull mistral
   
   # Phi-3 - Microsoft's efficient model
   ollama pull phi3
   ```

4. **List available models:**
   ```bash
   ollama list
   ```

### 🚀 Using Ollama with Perspt

Once Ollama is running, you can use it with Perspt:

```bash
# Basic usage (no API key needed!)
target/release/perspt --provider-type ollama --model llama3.2

# List available Ollama models
target/release/perspt --provider-type ollama --list-models

# Use different models
target/release/perspt --provider-type ollama --model codellama  # For coding
target/release/perspt --provider-type ollama --model mistral   # General purpose
target/release/perspt --provider-type ollama --model llama3.1:8b  # More capable

# With configuration file
cat > ollama_config.json << EOF
{
  "provider_type": "ollama",
  "default_model": "llama3.2",
  "api_key": "not-required"
}
EOF

target/release/perspt --config ollama_config.json
```

### 🎯 Ollama Model Recommendations

| **Model** | **Size** | **Best For** | **Speed** | **Quality** |
|-----------|----------|--------------|-----------|-------------|
| `llama3.2` | 3B | General chat, quick responses | ⚡⚡⚡ | ⭐⭐⭐ |
| `llama3.1:8b` | 8B | Balanced performance | ⚡⚡ | ⭐⭐⭐⭐ |
| `codellama` | 7B | Code generation, programming help | ⚡⚡ | ⭐⭐⭐⭐ |
| `mistral` | 7B | General purpose, good reasoning | ⚡⚡ | ⭐⭐⭐⭐ |
| `phi3` | 3.8B | Efficient, good for resource-constrained systems | ⚡⚡⚡ | ⭐⭐⭐ |

### 🔧 Ollama Troubleshooting

**❌ "Connection refused" errors:**
```bash
# Make sure Ollama is running
ollama serve

# Check if it's responding
curl http://localhost:11434/api/tags
```

**❌ "Model not found" errors:**
```bash
# List available models
ollama list

# Pull the model if not available
ollama pull llama3.2
```

**❌ Performance issues:**
```bash
# Use smaller models for better performance
target/release/perspt --provider-type ollama --model llama3.2

# Or check system resources
htop  # Monitor CPU/Memory usage
```

### 🌟 Ollama Advantages

- **🔒 Privacy**: All processing happens locally, no data sent to external servers
- **💰 Cost-effective**: No API fees or usage limits
- **⚡ Offline capable**: Works without internet connectivity
- **🎛️ Full control**: Choose exactly which models to run
- **🔄 Easy model switching**: Download and switch between models easily

## 🏗️ Architecture & Technical Features

### Built on Modern genai Crate

Perspt is built using the **genai** crate (v0.3.5), providing:

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
   - CLI arguments (working properly!)
   - Environment variables
   - JSON configuration files
   - Smart fallbacks and validation

### Custom Markdown Parser

Perspt includes a custom-built markdown parser optimized for terminal rendering:

- **Stream-optimized**: Handles real-time streaming content efficiently
- **Terminal-native**: Designed specifically for terminal color capabilities
- **Lightweight**: No external dependencies, built for performance
- **Robust**: Handles partial and malformed markdown gracefully
- **Buffer-managed**: Intelligent buffering for smooth rendering during streaming

### Key Technical Improvements

- **Fixed CLI Arguments**: API keys and model selection now work correctly via command line
- **Enhanced Streaming**: Improved buffering and event handling for smooth response rendering
- **Better Authentication**: Proper environment variable mapping for different providers
- **Responsive UI**: Reduced timeouts and improved responsiveness (50ms vs 100ms)
- **Custom Markdown Rendering**: Built-in parser eliminates external dependencies
- **Comprehensive Documentation**: Extensive code documentation and user guides

## 🖐️ Key Bindings

-   `Enter`: Send your input to the LLM or queue it if the LLM is busy.
-   `Esc`: Exit the application safely with proper terminal restoration.
-   `Ctrl+C` / `Ctrl+D`: Exit the application with graceful cleanup.
-   `Up Arrow` / `Down Arrow`: Scroll through chat history smoothly.
-   `Page Up` / `Page Down`: Fast scroll through long conversations.

**✅ UI Improvements:**
- Faster response times with 50ms event timeouts
- Better streaming buffer management for smooth markdown rendering with custom parser
- Visual feedback during model processing
- Proper terminal restoration on all exit paths

## 🔥 Recent Major Updates (v0.4.0)

### Migration to genai Crate

We've migrated from the `allms` crate to the modern **genai** crate (v0.3.5), bringing significant improvements:

**🎯 Fixed Critical Issues:**
1. ✅ **CLI Arguments Now Work**: API keys, models, and provider types work correctly via command line
2. ✅ **Flexible Authentication**: API keys work via CLI, environment variables, or config files
3. ✅ **Responsive UI**: Fixed keystroke waiting issues - UI now responds immediately
4. ✅ **Custom Markdown Parser**: Built-in markdown parser eliminates external dependencies

**🚀 New Features:**
- Support for latest reasoning models (o1-mini, o1-preview, Gemini 2.5 Pro)
- Enhanced streaming with proper reasoning chunk handling
- Custom markdown parser optimized for terminal rendering
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
- Smoother markdown rendering with custom parser
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
export GROQ_API_KEY="your-groq-key-here"
export COHERE_API_KEY="your-cohere-key-here"
export XAI_API_KEY="your-xai-key-here"
export DEEPSEEK_API_KEY="your-deepseek-key-here"

# Method 3: Ollama doesn't need API keys
perspt --provider-type ollama --model llama3.2
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
   perspt --provider-type gemini --model gemini-1.5-flash
   perspt --provider-type ollama --model llama3.2  # No API key needed!
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
