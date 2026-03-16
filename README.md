<div align="center">

<img src="assets/raw_logo.jpg" width="128" alt="Whisper logo" />

# Whisper

</div>

A macOS menu bar app that records audio via a global hotkey, transcribes it locally with
[whisper.cpp](https://github.com/ggerganov/whisper.cpp), and pastes the result into whatever text
field is focused. Runs entirely on-device — no data leaves your machine.

Built in Rust for Apple Silicon Macs.

## Prerequisites

- macOS on Apple Silicon
- [Rust toolchain](https://rustup.rs/) (stable, edition 2024)
- `cmake` — `brew install cmake`

## Setup

There are two ways to run Whisper, depending on whether you set up code signing.

### Option 1: Signed App Bundle (Recommended)

This gives you a stable code signing identity so that **Accessibility and Microphone permissions
persist across rebuilds**. Without this, macOS may silently revoke permissions every time you
recompile.

**One-time Xcode setup:**

1. Install [Xcode Command Line Tools](https://developer.apple.com/xcode/) if you haven't already
   (`xcode-select --install`).
2. Open Xcode → **Settings → Accounts** → add your Apple ID (any free Apple ID works).
3. This creates a free "Personal Team" signing certificate — an **Apple Development** identity in
   your Keychain.
4. You never need to open Xcode again after this.

Verify the certificate exists:

```sh
security find-identity -v -p codesigning
# Should show something like: "Apple Development: you@email.com (XXXXXXXXXX)"
```

Then build and run:

```sh
git clone https://github.com/michaelhelvey/whisper.git
cd whisper

# Download the whisper model (~466 MB)
make download-model

# Build, sign, and install to /Applications
make bundle

# Or build, install, and launch in one step
make run
```

The app is installed to `/Applications/Whisper.app`. Running `make bundle` again after code changes
will overwrite it in place — your Accessibility and Microphone permissions persist because the
signing identity is stable.

On first run:

1. **Microphone** — macOS will prompt you automatically. Click Allow.
2. **Accessibility** — go to _System Settings → Privacy & Security → Accessibility_ and enable
   **Whisper**. This is a one-time step — the permission survives rebuilds.

### Option 2: Run From Terminal (No Signing Required)

If you don't want to set up code signing, you can run the binary directly from a terminal emulator.
The binary inherits the terminal's existing Accessibility and Microphone permissions.

```sh
git clone https://github.com/michaelhelvey/whisper.git
cd whisper

make download-model
cargo build --release
./target/release/whisper
```

Your terminal app (e.g. Ghostty, iTerm, Terminal) must have **Accessibility** and **Microphone**
permissions enabled in _System Settings → Privacy & Security_. The downside is that permissions are
tied to your terminal app, not to Whisper itself, and you won't get a signed `.app` bundle in
`/Applications`.

## Usage

Once running, a 🎤 icon appears in your menu bar.

| Action                     | What happens                                    |
| -------------------------- | ----------------------------------------------- |
| Press **Ctrl+Space**       | Start recording (icon → 🔴)                     |
| Press **Ctrl+Space** again | Stop recording → transcribe (icon → ⏳) → paste |

The transcript is pasted into whatever text field has focus. Your clipboard is saved beforehand and
restored after pasting.

Click the menu bar icon and select **Quit** to exit, or press **⌘Q** while the menu is open.

## Configuration

Settings are compile-time constants in [`src/config.rs`](src/config.rs). Edit and rebuild to change:

| Setting     | Default                                      | Description                     |
| ----------- | -------------------------------------------- | ------------------------------- |
| Hotkey      | Ctrl+Space                                   | Global record toggle            |
| Model       | `~/.config/whisper/models/ggml-small.en.bin` | Whisper model path              |
| Language    | `en`                                         | Transcription language          |
| Paste delay | 500 ms                                       | Wait before restoring clipboard |

### Model Options

| Model                         | Size    | Latency (30s clip) | Notes                           |
| ----------------------------- | ------- | ------------------ | ------------------------------- |
| `ggml-small.en.bin` (default) | ~466 MB | 2–4s               | Best accuracy/speed tradeoff    |
| `ggml-base.en.bin`            | ~142 MB | 1–2s               | Lighter, slightly less accurate |
| `ggml-tiny.en.bin`            | ~75 MB  | <1s                | Fastest, noisier output         |

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

| Target                | Description                                            |
| --------------------- | ------------------------------------------------------ |
| `make run`            | Build, install to `/Applications`, and launch          |
| `make build`          | Compile release binary only                            |
| `make bundle`         | Build + install signed `.app` to `/Applications`       |
| `make download-model` | Download whisper model to `~/.config/whisper/`         |
| `make clean`          | Remove build artifacts and `/Applications/Whisper.app` |
| `make icon`           | Regenerate `AppIcon.icns` from `assets/raw_logo.jpg`   |
