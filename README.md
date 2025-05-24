# 👁️ Perspt: Your Terminal's Window to the AI World 🤖

> "The keyboard hums, the screen aglow,\
>  AI's wisdom, a steady flow.\
>  Will robots take over, it's quite the fright,\
>  Or just provide insights, day and night?\
>  We ponder and chat, with code as our guide,\
>  Is AI our helper or our human pride?"

**Perspt** (pronounced "perspect," short for **Per**sonal **S**pectrum **P**ertaining **T**houghts) is a command-line interface (CLI) application that gives you a peek into the mind of Large Language Models (LLMs). It allows you to chat with various AI models, including those from OpenAI, Gemini, and even locally run GGUF-format models, directly in your terminal.

## ✨ Features

-   **🎨 Interactive Chat Interface:** A colorful and responsive chat interface powered by Ratatui.
-   **⚡ Streaming Responses:** Real-time streaming of LLM responses for an interactive experience.
-   **🔀 Multiple Provider Support**: Seamlessly switch between different LLM providers:
    -   OpenAI (e.g., GPT-3.5-turbo, GPT-4)
    -   Gemini (e.g., Gemini Pro)
    -   Local LLMs (via the `llm` crate, supporting GGUF-format models like Llama, Mistral, etc.)
-   **⚙️ Configurable:** Flexible configuration via JSON files or command-line arguments.
-   **🔄 Input Queuing:** Type and submit new questions even while the AI is generating a previous response. Your inputs are queued and processed sequentially.
-   **💅 UI Feedback:** The input field is visually disabled during active LLM processing to prevent accidental submissions.
-   **📜 Markdown Rendering:** Assistant's responses are rendered with basic Markdown support (headings, inline code, lists, blockquotes) directly in the terminal.
-   **🛡️ Graceful Error Handling:** Handles network issues, API errors, and JSON parsing.

## 🚀 Getting Started

### 🛠️ Prerequisites

-   **Rust:** Ensure you have the Rust toolchain installed. Get it from [rustup.rs](https://rustup.rs/).
-   **🔑 LLM API Key (for API providers):** For OpenAI or Gemini, you'll need an API key from the respective provider.
-   **🧠 Local Model File (for local_llm):** For local inference, download a GGUF-format model file. See the "Using Local Models" section below.
-   **C Compiler (potentially for local_llm):** The `llm` crate may require a C compiler (like GCC or Clang) to build its backends (e.g., for `ggml`). Ensure you have one installed if you plan to use local models.

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

Create a `config.json` in the root directory of the project, or in `~/.config/perspt/config.json`, or specify a custom path using the `-c` CLI argument.

**General Structure:**

```json
{
    "providers": {
        "gemini": "https://generativelanguage.googleapis.com/v1beta",
        "openai": "https://api.openai.com/v1"
    },
    "api_key": "YOUR_API_KEY_IF_SHARED_OR_FOR_DEFAULT_PROVIDER",
    "default_model": "MODEL_NAME_OR_PATH_TO_LOCAL_MODEL",
    "default_provider": "PROVIDER_PROFILE_NAME",
    "provider_type": "openai|gemini|local_llm"
}
```

-   **`providers`** (Optional): A map of provider profile names to their API base URLs. Useful if you use multiple API endpoints or custom deployments.
    -   Example: `"openai_custom": "http://localhost:8080/v1"`
-   **`api_key`** (Optional): Your API key. This can be a general key, or you might prefer setting it via CLI for specific providers if you use multiple.
-   **`default_model`**:
    -   For API providers (OpenAI, Gemini): The model name (e.g., "gpt-3.5-turbo", "gemini-pro").
    -   For `local_llm`: **The full, absolute path to your local GGUF model file** (e.g., "/path/to/your/model.gguf").
-   **`default_provider`** (Optional): The name of the provider profile from the `providers` map to use by default (e.g., "openai", "gemini"). For `local_llm`, this field is less critical if `provider_type` is set to "local_llm".
-   **`provider_type`**: Specifies the type of LLM provider. This is a key field.
    -   Valid values: `"openai"`, `"gemini"`, `"local_llm"`.
    -   If using `local_llm`, ensure `default_model` points to the GGUF file path.

**Example `config.json` for OpenAI:**
```json
{
    "providers": {
        "openai": "https://api.openai.com/v1"
    },
    "api_key": "sk-YOUR_OPENAI_API_KEY",
    "default_model": "gpt-3.5-turbo",
    "provider_type": "openai"
}
```

**Example `config.json` for Gemini:**
```json
{
    "providers": {
        "gemini": "https://generativelanguage.googleapis.com/v1beta"
    },
    "api_key": "YOUR_GEMINI_API_KEY",
    "default_model": "gemini-pro",
    "provider_type": "gemini"
}
```

**Example `config.json` for Local LLM:**
```json
{
    "default_model": "/path/to/your/llama-2-7b-chat.Q4_K_M.gguf",
    "provider_type": "local_llm"
    // api_key and providers map are not needed for local_llm
}
```

#### ⌨️ Command-Line Arguments

Key arguments include:

-   `-c <FILE>`, `--config <FILE>`: Path to a custom configuration file.
-   `-t <TYPE>`, `--provider-type <TYPE>`: Specify the provider type (`openai`, `gemini`, `local_llm`). **This is important for choosing the backend.**
-   `-k <API_KEY>`, `--api-key <API_KEY>`: Your API key (for OpenAI/Gemini).
-   `-m <MODEL/PATH>`, `--model-name <MODEL/PATH>`:
    -   For API providers: The model name (e.g., `gpt-4`, `gemini-pro`).
    -   For `local_llm`: **The full path to your GGUF model file.**
-   `-p <PROVIDER_PROFILE>`, `--provider <PROVIDER_PROFILE>`: Choose a pre-configured provider profile from your `config.json`'s `providers` map. (Less relevant if directly using `-t local_llm`).
-   `--list-models`: List available models (primarily for API providers, may show placeholder for local).

Run `target/release/perspt --help` for a full list.

### 🏃 Usage Examples

**OpenAI:**
```bash
target/release/perspt -t openai -k YOUR_OPENAI_API_KEY -m gpt-3.5-turbo
```

**Gemini:**
```bash
target/release/perspt -t gemini -k YOUR_GEMINI_API_KEY -m gemini-pro
```

**Local LLM (GGUF):**
```bash
target/release/perspt -t local_llm -m /path/to/your/model.gguf
```
(No API key needed for local models.)

**Using a config file:**
```bash
target/release/perspt --config my_config.json
```
(Ensure `my_config.json` is correctly set up, especially `provider_type` and `default_model`).

### 🧠 Using Local Models (GGUF)

Perspt uses the [`llm`](https://crates.io/crates/llm) crate to run GGUF-format models locally on your CPU (or GPU if supported by the underlying backend and enabled).

1.  **Download a GGUF Model:**
    *   You can find GGUF models on Hugging Face (e.g., search for "Llama GGUF", "Mistral GGUF"). Popular sources include TheBloke.
    *   Choose a quantization level that suits your RAM and performance needs (e.g., Q4_K_M is a common balanced choice).
    *   Example: `llama-2-7b-chat.Q4_K_M.gguf`

2.  **Run Perspt with Local Model:**
    Use the `-t local_llm` and `-m /path/to/model.gguf` arguments:
    ```bash
    target/release/perspt -t local_llm -m /path/to/your/llama-2-7b-chat.Q4_K_M.gguf
    ```
    Or configure it in `config.json` as shown above.

3.  **Compilation Note:**
    The `llm` crate and its backends (like `ggml`) are compiled when you build Perspt. This process might require a C compiler (like GCC or Clang) and can take some time. If you encounter build issues related to `llm` or `ggml`, ensure you have a working C development toolchain.

## 🖐️ Key Bindings

-   `Enter`: Send your input to the LLM or queue it if the LLM is busy.
-   `Esc`: Exit the application.
-   `Ctrl+C` / `Ctrl+D`: Exit the application.
-   `Up Arrow` / `Down Arrow`: Scroll through chat history.

## 🤝 Contributing

Contributions are welcome! Please open issues or submit pull requests for any bugs, features, or improvements.

## 📜 License

Perspt is released under the **GNU Lesser General Public License v3.0** (LGPL-3.0). See the [`LICENSE`](LICENSE) file for details.

## ✍️ Author

-   Vikrant Rathore

---
Perspt: **Per**sonal **S**pectrum **P**ertaining **T**houghts – the human lens through which we explore the enigma of AI and its implications for humanity.
