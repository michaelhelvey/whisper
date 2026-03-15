# Whisper

A macOS menu bar utility that records audio via a global hotkey, transcribes it locally with
[whisper.cpp](https://github.com/ggerganov/whisper.cpp), and pastes the result into whatever text
field is focused.

Built entirely in Rust, and does not send any data off of your local computer.

## Prerequisites

- macOS on Apple Silicon
- [Rust toolchain](https://rustup.rs/) (stable)
- `cmake` (for compiling whisper.cpp) — `brew install cmake`

## Getting Started

```sh
# Clone the repo
git clone https://github.com/helvetici/whisper.git
cd whisper

# Download the whisper model (~466 MB) to ~/.config/whisper/models/
make download-model

# Build and assemble the .app bundle
make bundle

# Remove quarantine (unsigned app)
xattr -cr target/Whisper.app

# Launch
make run
```

## Permissions

Two one-time manual steps:

1. **Microphone** — macOS prompts automatically on first recording attempt.
2. **Accessibility** — Add `target/whisper.app` to _System Settings → Privacy & Security →
   Accessibility_. Required for the global hotkey and synthetic paste.

## Usage

Once running, a 🎤 icon appears in the menu bar.

| Action                  | Result                                                                |
| ----------------------- | --------------------------------------------------------------------- |
| Press **⌥ Space**       | Start recording (icon → 🔴)                                           |
| Press **⌥ Space** again | Stop recording, transcribe (icon → ⏳), paste text into focused field |

## Configuration

All configuration is hard-coded in `src/config.rs`. Edit the constants and recompile to change
settings.

```rust
pub const HOTKEY_KEY: &str = "space";
pub const HOTKEY_MODIFIERS: &[&str] = &["option"];

pub const MODEL_PATH: &str = "~/.config/whisper/models/ggml-small.en.bin";
pub const LANGUAGE: &str = "en";

pub const PASTE_DELAY_MS: u64 = 50;
```

### Model Options

| Model                         | Size    | Latency (30s audio) | Notes                        |
| ----------------------------- | ------- | ------------------- | ---------------------------- |
| `ggml-small.en.bin` (default) | ~466 MB | 2–4s                | Best accuracy/speed tradeoff |
| `ggml-base.en.bin`            | ~142 MB | 1–2s                | Lighter alternative          |
| `ggml-tiny.en.bin`            | ~75 MB  | <1s                 | Fastest, noisier output      |

To use a different model, download it and update `MODEL_PATH` in `src/config.rs`:

```sh
# Example: download the base model instead
curl -L -o ~/.config/whisper/models/ggml-base.en.bin \
  https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.en.bin
```

## Make Targets

| Target                | Description                                    |
| --------------------- | ---------------------------------------------- |
| `make build`          | Compile release binary                         |
| `make bundle`         | Build + assemble `.app` bundle                 |
| `make download-model` | Download whisper model weights                 |
| `make setup`          | Download model + print permission instructions |
| `make run`            | Bundle + launch the app                        |
| `make clean`          | Remove build artifacts                         |
