# Whisper

A macOS menu bar app that records audio via a global hotkey, transcribes it locally with
[whisper.cpp](https://github.com/ggerganov/whisper.cpp), and pastes the result into whatever text
field is focused. Runs entirely on-device — no data leaves your machine.

Built in Rust for Apple Silicon Macs.

## Prerequisites

- macOS on Apple Silicon
- [Rust toolchain](https://rustup.rs/) (stable, edition 2024)
- `cmake` — `brew install cmake`

## Setup

```sh
git clone https://github.com/helvetici/whisper.git
cd whisper

# Download the whisper model (~466 MB)
make download-model

# Build, bundle, and run
make run
```

On first run:

1. **Microphone** — macOS will prompt you automatically. Click Allow.
2. **Accessibility** — your terminal app (e.g. Ghostty, iTerm, Terminal) needs Accessibility
   permission so the global hotkey and synthetic paste work. Go to _System Settings → Privacy &
   Security → Accessibility_ and enable your terminal. This is a one-time step.

## Usage

Once running, a 🎤 icon appears in your menu bar.

| Action                    | What happens                                      |
| ------------------------- | ------------------------------------------------- |
| Press **Ctrl+Space**      | Start recording (icon → 🔴)                       |
| Press **Ctrl+Space** again | Stop recording → transcribe (icon → ⏳) → paste   |

The transcript is pasted into whatever text field has focus. Your clipboard is saved beforehand and
restored after pasting.

Click the menu bar icon and select **Quit** to exit, or press **⌘Q** while the menu is open.

## Configuration

Settings are compile-time constants in [`src/config.rs`](src/config.rs). Edit and rebuild to change:

| Setting        | Default                                      | Description                    |
| -------------- | -------------------------------------------- | ------------------------------ |
| Hotkey         | Ctrl+Space                                   | Global record toggle           |
| Model          | `~/.config/whisper/models/ggml-small.en.bin` | Whisper model path             |
| Language       | `en`                                         | Transcription language         |
| Paste delay    | 500 ms                                       | Wait before restoring clipboard|

### Model Options

| Model                         | Size    | Latency (30s clip) | Notes                        |
| ----------------------------- | ------- | ------------------- | ---------------------------- |
| `ggml-small.en.bin` (default) | ~466 MB | 2–4s                | Best accuracy/speed tradeoff |
| `ggml-base.en.bin`            | ~142 MB | 1–2s                | Lighter, slightly less accurate |
| `ggml-tiny.en.bin`            | ~75 MB  | <1s                 | Fastest, noisier output      |

To use a different model:

```sh
curl -L -o ~/.config/whisper/models/ggml-base.en.bin \
  https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.en.bin
```

Then update `MODEL_PATH` in `src/config.rs` and rebuild with `make run`.

## Logs

Stderr is redirected to `~/.config/whisper/whisper.log`. Check this file if something isn't working:

```sh
tail -f ~/.config/whisper/whisper.log
```

## Make Targets

| Target                | Description                                     |
| --------------------- | ----------------------------------------------- |
| `make run`            | Build, bundle, and launch the app               |
| `make build`          | Compile release binary only                     |
| `make bundle`         | Build + assemble `.app` bundle with codesigning |
| `make download-model` | Download whisper model to `~/.config/whisper/`  |
| `make clean`          | Remove all build artifacts                      |
