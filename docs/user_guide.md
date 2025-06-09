# Perspt User Guide

```
██████╗ ███████╗██████╗ ███████╗██████╗ ████████╗
██╔══██╗██╔════╝██╔══██╗██╔════╝██╔══██╗╚══██╔══╝
██████╔╝█████╗  ██████╔╝███████╗██████╔╝   ██║   
██╔═══╝ ██╔══╝  ██╔══██╗╚════██║██╔═══╝    ██║   
██║     ███████╗██║  ██║███████║██║        ██║   
╚═╝     ╚══════╝╚═╝  ╚═╝╚══════╝╚═╝        ╚═╝   
                                                  
    Your Terminal's Window to the AI World 🤖
```

## 🚀 Welcome to Perspt!

Perspt (pronounced "perspect," short for **Per**sonal **S**pectrum **P**ertaining **T**houghts) is a blazing-fast terminal-based chat application that lets you talk to various Large Language Models (LLMs) through a beautiful, unified interface. Built with Rust for maximum performance and reliability, using the modern **genai** crate for provider integration.

## ✨ Features

```
┌─────────────────────────────────────────────────────┐
│  🤖 Multiple AI Providers    📡 Real-time Streaming │
│  ⚡ Lightning Fast           🛡️  Robust Error Handling│
│  🎨 Beautiful Terminal UI    📝 Custom Markdown Parser│
│  ⚙️  Flexible Configuration  🔑 Secure Authentication│
└─────────────────────────────────────────────────────┘
```

### Supported AI Providers

- **OpenAI** - GPT-3.5, GPT-4, GPT-4o series, o1-mini, o1-preview, o3-mini
- **Anthropic** - Claude 3 Opus, Sonnet, Haiku, Claude 3.5 series
- **Google** - Gemini Pro, Gemini Flash, Gemini 2.0, Gemini 2.5 Pro
- **Mistral AI** - Mistral 7B, Mixtral 8x7B, Mistral Large
- **Perplexity** - Sonar models, reasoning models
- **DeepSeek** - DeepSeek chat and reasoning models
- **AWS Bedrock** - Multiple foundation models including Amazon Nova

## 🏁 Quick Start

### Installation

```bash
# Clone the repository
git clone https://github.com/your-username/perspt
cd perspt

# Build the application
cargo build --release

# Run Perspt
./target/release/perspt
```

### First Run

```
┌──────────────────────────────────────────────────────┐
│                   🎯 First Time Setup                │
├──────────────────────────────────────────────────────┤
│                                                      │
│  1. Set your API key:                               │
│     export OPENAI_API_KEY="sk-your-api-key"         │
│                                                      │
│  2. Run Perspt:                                      │
│     perspt                                           │
│                                                      │
│  3. Start chatting! 💬                              │
│                                                      │
└──────────────────────────────────────────────────────┘
```

## 🎮 Basic Usage

### Starting a Chat Session

```bash
# Default OpenAI GPT-4o-mini
perspt

# Use a specific model
perspt --model gpt-4

# Use a different provider
perspt --provider-type anthropic --model claude-3-sonnet-20240229

# Custom configuration file
perspt --config /path/to/your/config.json
```

### In-Chat Controls

```
┌─────────────────────────────────────────────────────┐
│                   🎮 Keyboard Controls              │
├─────────────────────────────────────────────────────┤
│                                                     │
│  ↵ Enter      │ Send message to AI                 │
│  ↑ ↓ Arrows   │ Scroll through chat history        │
│  F1 or ?      │ Show/hide help overlay             │
│  Ctrl+C       │ Quit application                   │
│  Esc          │ Close help or quit                 │
│  Backspace    │ Delete characters                  │
│                                                     │
└─────────────────────────────────────────────────────┘
```

### Chat Interface Layout

```
┌────────────────────────────────────────────────────────────┐
│ 🤖 Perspt v0.4.0 │ Model: gpt-4o-mini │ Status: Ready    │
├────────────────────────────────────────────────────────────┤
│                                                            │
│ 👤 You: Hello! How are you today?                         │
│                                                            │
│ 🤖 Assistant: Hello! I'm doing well, thank you for        │
│    asking. I'm here and ready to help you with any        │
│    questions or tasks you might have. How can I           │
│    assist you today?                                       │
│                                                            │
│ ░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░   │
├────────────────────────────────────────────────────────────┤
│ 💬 Type your message: ▌                                   │
├────────────────────────────────────────────────────────────┤
│ ⚡ Ready │ Press F1 for help │ Ctrl+C to quit             │
└────────────────────────────────────────────────────────────┘
```

## ⚙️ Configuration

### Configuration File Format

Create a `config.json` file to customize Perspt:

```json
{
  "api_key": "your-api-key-here",
  "provider_type": "openai",
  "default_model": "gpt-4o-mini",
  "default_provider": "openai",
  "providers": {
    "openai": "https://api.openai.com/v1",
    "anthropic": "https://api.anthropic.com",
    "google": "https://generativelanguage.googleapis.com/v1beta/",
    "groq": "https://api.groq.com/openai/v1",
    "cohere": "https://api.cohere.com/v1",
    "xai": "https://api.x.ai/v1",
    "deepseek": "https://api.deepseek.com/v1",
    "ollama": "http://localhost:11434/v1"
  }
}
```

### Configuration Options

```
┌─────────────────────────────────────────────────────┐
│                📋 Configuration Fields              │
├─────────────────────────────────────────────────────┤
│                                                     │
│  api_key         │ Your API authentication key     │
│  provider_type   │ AI provider (openai, anthropic) │
│  default_model   │ Model to use by default         │
│  default_provider│ Provider configuration name     │
│  providers       │ Map of provider names to URLs   │
│                                                     │
└─────────────────────────────────────────────────────┘
```

### Environment Variables

```bash
# API Keys (provider-specific)
export OPENAI_API_KEY="sk-your-openai-key"
export ANTHROPIC_API_KEY="sk-ant-your-anthropic-key"
export GOOGLE_API_KEY="your-google-api-key"

# AWS Configuration (for Bedrock)
export AWS_PROFILE="your-aws-profile"
export AWS_REGION="us-east-1"

# Google Cloud (for Vertex AI)
export PROJECT_ID="your-gcp-project-id"
```

## 🔧 Command Line Options

```
USAGE:
    perspt [OPTIONS]

OPTIONS:
    -c, --config <FILE>           Configuration file path
    -k, --api-key <KEY>          API key (overrides config)
    -m, --model <MODEL>          Model name to use
    -p, --provider <PROVIDER>    Provider profile name
        --provider-type <TYPE>   Provider type
    -l, --list-models            List available models
    -h, --help                   Print help information
    -V, --version                Print version information
```

### Examples

```bash
# List available models for OpenAI
perspt --provider-type openai --list-models

# Use Claude with specific model
perspt --provider-type anthropic \
       --model claude-3-opus-20240229 \
       --api-key sk-ant-your-key

# Use custom configuration
perspt --config ~/.config/perspt/config.json

# Override model from config
perspt --config my-config.json --model gpt-4
```

## 🎨 User Interface Guide

### Status Indicators

```
┌─────────────────────────────────────────────────────┐
│                   📊 Status Messages                │
├─────────────────────────────────────────────────────┤
│                                                     │
│  ⚡ Ready          │ System ready for input         │
│  📤 Sending...     │ Request being sent to AI       │
│  🤖 Thinking...    │ AI is processing your message  │
│  🧠 Reasoning...   │ AI using reasoning capabilities │
│  📝 Streaming...   │ Receiving streamed response    │
│  ❌ Error          │ Something went wrong           │
│  🔄 Reconnecting   │ Attempting to reconnect        │
│                                                     │
└─────────────────────────────────────────────────────┘
```

### Message Types

```
👤 You: Your messages appear with a user icon
🤖 Assistant: AI responses appear with a robot icon
⚠️  Warning: Important notices and warnings
❌ Error: Error messages and troubleshooting info
💡 System: System notifications and status updates
```

### Scrolling and Navigation

```
┌─────────────────────────────────────────────────────┐
│                 📜 Chat Navigation                  │
├─────────────────────────────────────────────────────┤
│                                                     │
│  ↑ Arrow Up      │ Scroll up in chat history       │
│  ↓ Arrow Down    │ Scroll down in chat history     │
│  Page Up         │ Scroll up by page               │
│  Page Down       │ Scroll down by page             │
│  Home            │ Jump to top of chat             │
│  End             │ Jump to bottom of chat          │
│                                                     │
└─────────────────────────────────────────────────────┘
```

## 🛠️ Troubleshooting

### Common Issues

```
┌─────────────────────────────────────────────────────┐
│                  🔧 Common Issues                   │
├─────────────────────────────────────────────────────┤
│                                                     │
│  Problem: API key error                             │
│  Solution: Check your API key is valid and active  │
│            Use --api-key or environment variables   │
│                                                     │
│  Problem: Network connection failed                 │
│  Solution: Check internet connection and firewall  │
│                                                     │
│  Problem: Model not found                           │
│  Solution: Use --list-models to see available ones │
│            Models are validated with genai crate   │
│                                                     │
│  Problem: Terminal display corrupted                │
│  Solution: Perspt has panic recovery - restart app │
│                                                     │
│  Problem: Streaming appears slow                    │
│  Solution: Network dependent, parser is optimized  │
│                                                     │
└─────────────────────────────────────────────────────┘
```

### Error Messages

```
┌─────────────────────────────────────────────────────┐
│                  ⚠️  Error Types                    │
├─────────────────────────────────────────────────────┤
│                                                     │
│  🔒 Authentication Error                            │
│      • Check API key validity                      │
│      • Verify environment variables                │
│                                                     │
│  🌐 Network Error                                   │
│      • Check internet connection                   │
│      • Verify firewall settings                    │
│                                                     │
│  📊 Rate Limit Error                                │
│      • Wait before sending next request            │
│      • Consider upgrading API plan                 │
│                                                     │
│  🤖 Invalid Model Error                             │
│      • Use --list-models to see options            │
│      • Check provider documentation                │
│                                                     │
└─────────────────────────────────────────────────────┘
```

### Recovery Procedures

```bash
# Reset terminal if display is corrupted
reset

# Check if Perspt is running
ps aux | grep perspt

# Kill hung processes
pkill perspt

# Check configuration
perspt --config /path/to/config.json --list-models

# Test with minimal configuration
perspt --provider-type openai --api-key "your-key" --list-models
```

## 📚 Advanced Usage

### Custom Markdown Parser

Perspt includes a built-in markdown parser optimized for terminal rendering:

```
┌─────────────────────────────────────────────────────┐
│              📝 Markdown Features                   │
├─────────────────────────────────────────────────────┤
│                                                     │
│  ✅ Headers (# ## ###)                             │
│  ✅ Bold (**text**) and Italic (*text*)            │
│  ✅ Code blocks (```language```)                   │
│  ✅ Inline code (`code`)                           │
│  ✅ Lists (- item, 1. item)                        │
│  ✅ Links and references                            │
│  ✅ Stream-optimized rendering                      │
│  ✅ Terminal color support                          │
│                                                     │
└─────────────────────────────────────────────────────┘
```

### genai Crate Integration

Perspt uses the modern **genai** crate for robust LLM integration:

```
┌─────────────────────────────────────────────────────┐
│                🔧 Technical Features                │
├─────────────────────────────────────────────────────┤
│                                                     │
│  📡 Real-time streaming with proper event handling │
│  🧠 Reasoning model support (o1-mini, o1-preview)  │
│  🔄 Automatic model discovery and validation       │
│  🛡️ Comprehensive error handling and recovery      │
│  ⚡ 50ms response times for better user experience │
│  🎯 Latest model support (GPT-4.1, Gemini 2.5)    │
│                                                     │
└─────────────────────────────────────────────────────┘
```

### Custom Endpoints

```json
{
  "providers": {
    "local-openai": "http://localhost:8080/v1",
    "custom-claude": "https://your-proxy.com/anthropic"
  },
  "default_provider": "local-openai"
}
```

### Multiple Configurations

```bash
# Work configuration
perspt --config ~/.config/perspt/work.json

# Personal configuration  
perspt --config ~/.config/perspt/personal.json

# Experimental configuration
perspt --config ~/.config/perspt/experimental.json
```

### Batch Operations

```bash
# Test multiple providers
for provider in openai anthropic google; do
  echo "Testing $provider..."
  perspt --provider-type $provider --list-models
done
```

## 🔐 Security Best Practices

```
┌─────────────────────────────────────────────────────┐
│                  🛡️  Security Tips                  │
├─────────────────────────────────────────────────────┤
│                                                     │
│  ✅ Use environment variables for API keys          │
│  ✅ Restrict file permissions on config files       │
│  ✅ Rotate API keys regularly                       │
│  ✅ Use separate keys for different environments    │
│  ❌ Don't commit API keys to version control        │
│  ❌ Don't share configuration files with keys       │
│                                                     │
└─────────────────────────────────────────────────────┘
```

### File Permissions

```bash
# Secure your configuration file
chmod 600 ~/.config/perspt/config.json

# Create secure config directory
mkdir -p ~/.config/perspt
chmod 700 ~/.config/perspt
```

## 🎯 Tips and Tricks

### Productivity Tips

```
┌─────────────────────────────────────────────────────┐
│                  💡 Pro Tips                        │
├─────────────────────────────────────────────────────┤
│                                                     │
│  • Use aliases for common configurations            │
│  • Create provider-specific config files            │
│  • Use environment variables for easy switching     │
│  • Keep API keys in secure key management           │
│  • Use --list-models to explore new models          │
│  • Try reasoning models (o1-mini) for complex tasks │
│  • Custom markdown parser handles formatting well   │
│  • genai crate provides latest model support        │
│                                                     │
└─────────────────────────────────────────────────────┘
```

### Shell Aliases

```bash
# Add to your .bashrc or .zshrc
alias pgpt='perspt --provider-type openai --model gpt-4'
alias pclaude='perspt --provider-type anthropic --model claude-3-sonnet-20240229'
alias pgemini='perspt --provider-type google --model gemini-pro'
alias pwork='perspt --config ~/.config/perspt/work.json'
```

## 📞 Getting Help

### Built-in Help

```bash
# General help
perspt --help

# Show help overlay in chat
# Press F1 or ? while in chat mode
```

### Resources

```
┌─────────────────────────────────────────────────────┐
│                  📚 Resources                       │
├─────────────────────────────────────────────────────┤
│                                                     │
│  📖 Documentation: docs/                            │
│  🐛 Issues: GitHub Issues                           │
│  💬 Discussions: GitHub Discussions                 │
│  📧 Email: support@perspt.dev                       │
│                                                     │
└─────────────────────────────────────────────────────┘
```

---

```
🎉 Happy chatting with Perspt! 🎉

Built with ❤️ using Rust, genai crate, and custom optimizations
for the best terminal AI chat experience.
```
