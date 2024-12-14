# 👁️ Perspt: Your Terminal's Window to the AI World 🤖

> "The keyboard hums, the screen aglow,\
>  AI's wisdom, a steady flow.\
>  Will robots take over, it's quite the fright,\
>  Or just provide insights, day and night?\
>  We ponder and chat, with code as our guide,\
>  Is AI our helper or our human pride?"

**Perspt** (pronounced "perspect," short for **Per**sonal **S**pectrum **P**ertaining **T**houghts) is a command-line interface (CLI) application that gives you a peek into the mind of Large Language Models (LLMs). It's like having a chatty AI companion right in your terminal, all thanks to the magical combination of Ratatui and streaming responses. It's the perfect tool for anyone who's ever wondered what an AI *really* thinks (or at least, what it's programmed to think).

## ✨ Features

-   **🎨 Interactive Chat Interface:** A colorful and responsive chat interface powered by Ratatui. Think of it as a cozy, if slightly digital, tea room for conversations with AI.
-   **⚡ Streaming Responses:** Real-time streaming of LLM responses. It's like watching a thought bubble form, but way faster (and in code).
-   **🔀 Multiple Provider Support**: Support for multiple LLM providers like OpenAI and Gemini. Because variety is the spice of AI!
-   **⚙️ Configurable:** Loads configurations from JSON files or command-line arguments. Because who likes hardcoding when you can just have options?
-   **⌨️ Command-Line Options:**
    -   `-c <FILE>` or `--config <FILE>`: Specify the path to the configuration file. It's like choosing a secret code to unlock special features!
    -   `-k <API_KEY>` or `--api-key <API_KEY>`: Your secret API key to talk to the AI. Remember to keep it safe, like your favorite socks.
    -   `-m <MODEL>` or `--model-name <MODEL>`: Select the LLM model to use (e.g., `gpt-4`, `gemini-pro`). Think of it as choosing your AI's personality.
    -   `-p <PROVIDER>` or `--provider <PROVIDER>`: Choose the LLM provider (e.g., `openai`, `gemini`). Because sometimes, you just need a change of scenery.
    -   `--list-models`: List all the models available for the selected provider. For those moments when you can't decide which AI to talk to.
-   **🛡️ Graceful Error Handling:** Handles network issues, API errors, and JSON parsing like a seasoned diplomat. No meltdowns here!
-   **🖐️ Sane Keybindings:**
    -   `Enter`: Send your thoughts to the AI and wait for its profound (or occasionally silly) reply.
    -   `Esc`: Time to leave the tea room, or exit the chat.
    -   `Ctrl+C`: Another way to say "goodbye for now."
    -   `Ctrl+D`: Or this is another way to exit.

## 🚀 Getting Started

### 🛠️ Prerequisites

-   **Rust:** Make sure you have the Rust toolchain installed. If not, get it from [rustup.rs](https://rustup.rs/).
-   **🔑 An LLM API Key:** You'll need an API key from an OpenAI-compatible provider. Treat it like the key to your intellectual kingdom.

### 📦 Installation

1.  **Clone the Repository:**

    ```bash    git clone <repository-url>    cd perspt    ```

2.  **Build the Project:**

    ```bash    cargo build --release    ```

    Find the magic executable in the `target/release` directory.

### ⚙️ Configuration

Perspt can be configured using a JSON config file or command-line arguments. It's like choosing your own adventure...with AI!

#### 📝 Config File (Optional)

Create a `config.json` in the root directory or specify the path using `-c`. It should look like this:

```json{    "providers": {        "gemini": "https://generativelanguage.googleapis.com/v1beta",        "openai": "https://api.openai.com/v1"    },    "api_key": "YOUR_API_KEY",    "default_model": "gpt-3.5-turbo",    "default_provider": "openai"}```

-   **`providers`**: A map of providers and their base URLs.
-   **`api_key`**: Your API key. Handle with care!
-   **`default_model`**: The default LLM model.
-   **`default_provider`**: The default provider.

Note: Command-line options always win if there is a conflict.

### 🏃 Usage

#### 🗣️ Basic Chat

```bash
target/release/perspt -m gpt-4 -p openai -k <YOUR_API_KEY>
```
or
```bash
target/release/perspt --config config.json
```

#### 📜 Listing Models

```bash
target/release/perspt --list-models -p openai -k <YOUR_API_KEY>
```

#### 🆘 Command-Line Options

```bashtarget/release/perspt --help```

## 🖐️ Key Bindings

-   `Enter`: Send your input to the LLM.
-   `Esc`: Exit the chat.
-   `Ctrl+C`: Exit the chat.
-   `Ctrl+D`: Exit the chat.

## 🤝 Contributing

Contributions are welcome and encouraged! Please feel free to open issues or submit pull requests. Let's make Perspt even more fantastic!

## 📜 License

Perspt is released under the **GNU Lesser General Public License v3.0** (LGPL-3.0). See the [`LICENSE`](LICENSE) file for the full legal mumbo-jumbo.

## ✍️ Author

-   Vikrant Rathore

## 🤔 A Little Poem

*A.I. ponders, we do too.*

Perspt: **Per**sonal **S**pectrum **P**ertaining **T**houghts – the human lens through which we explore the enigma of AI and its implications for humanity.
