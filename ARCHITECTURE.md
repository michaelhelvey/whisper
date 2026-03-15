# Whisper — Architecture Design

A macOS menu bar utility that records audio via a global hotkey, transcribes it with whisper.cpp,
and pastes the result into whatever text field is focused.

Built entirely in Rust. No Xcode project. No Swift. `cargo build --release` and a shell script to
assemble the `.app` bundle.

---

## Flow

```
[⌥+Space pressed] → recording starts, icon turns red
[⌥+Space pressed] → recording stops
  → PCM buffer (16kHz mono f32) sent to whisper.cpp
  → transcript text received
  → saved to pasteboard, synthetic ⌘V fired
  → original pasteboard restored
  → icon returns to idle
```

All of this happens in-process. No subprocesses, no IPC, no sidecar binaries.

---

## Crate Map

| Concern              | Crate                                          | Why this one                                                                                       |
| -------------------- | ---------------------------------------------- | -------------------------------------------------------------------------------------------------- |
| Menu bar icon + menu | `objc2` + `objc2-app-kit` + `objc2-foundation` | Direct NSStatusBar/NSMenu/NSPasteboard access. No wrapper abstraction to fight.                    |
| Audio capture        | `cpal`                                         | Pure Rust, CoreAudio backend on macOS. Gives raw PCM callbacks from the mic.                       |
| Transcription        | `whisper-rs`                                   | Mature whisper.cpp bindings (184K downloads). Compiles whisper.cpp from source with Metal support. |
| Global hotkey        | `core-graphics`                                | CGEventTapCreate / CGEventPost for intercepting and synthesizing keyboard events.                  |
| Text injection       | `core-graphics` + `objc2-app-kit`              | CGEvent for synthetic ⌘V, NSPasteboard for clipboard manipulation.                                 |
| Config               | (none — compile-time constants)                | Hard-coded in `config.rs`. Recompile to change.                                                    |

### Why no tokio

This app does three things sequentially: record → transcribe → paste. There is no concurrency. The
`NSApplication` run loop is the only event loop, driven by `objc2-app-kit`.

### Why `core-graphics` and not a higher-level hotkey crate

The existing Rust hotkey crates (e.g. `global-hotkey`) pull in event loop abstractions we don't need
and sometimes conflict with a raw `NSApplication` run loop. `CGEventTapCreate` is ~30 lines of
unsafe FFI via the `core-graphics` crate. It's the same API Swift apps use. We need it anyway for
the synthetic paste, so there's no additional dependency.

---

## Component Design

### Menu Bar (`menu_bar.rs`)

Uses `objc2-app-kit` to create an `NSStatusItem` with a text title (e.g. `"🎤"` idle, `"🔴"`
recording, `"⏳"` transcribing). Text titles avoid needing to bundle template images.

The `NSMenu` has two items: a disabled status label and "Quit".

Setup:

1. `NSApplication::sharedApplication()` with `setActivationPolicy(.accessory)` — no dock icon, no
   main window.
2. `NSStatusBar::systemStatusBar().statusItemWithLength(NSVariableStatusItemLength)`.
3. Assign an `NSMenu` to the status item.
4. Run the `NSApplication` run loop.

Menu item actions use `objc2`'s `define_class!` macro to create a small Objective-C class with
`@objc` methods that Rust closures delegate to.

### Global Hotkey (`hotkey.rs`)

`CGEvent.tapCreate()` with a mask for `kCGEventKeyDown`. The callback inspects each key event for
the configured modifier+key combo (hard-coded default: `⌥+Space`). On match, returns `nil` to swallow the event
and toggles recording state.

Requires Accessibility permission. On first run, the user must manually add the `.app` bundle (or
the bare binary) to `System Settings → Privacy & Security → Accessibility`. Since this is
personal-use software with no distribution, that's a one-time manual step.

The event tap is added to the current `CFRunLoop`, which is the same one backing the `NSApplication`
run loop.

### Audio Recorder (`recorder.rs`)

Uses `cpal` to open the default input device and build an input stream at 16kHz mono f32. (If the
hardware doesn't support 16kHz natively, `cpal` will report it; we record at the hardware's native
rate and resample before feeding whisper. The `rubato` crate handles resampling if needed, but most
Mac mics support 16kHz directly.)

Recording flow:

1. `start()`: Open stream, begin pushing samples into a `Vec<f32>` behind an `Arc<Mutex<>>`.
2. `stop() -> Vec<f32>`: Drop the stream, return the accumulated buffer.

The buffer lives in memory. A 60-second recording at 16kHz mono f32 is ~3.8 MB. No temp files
needed.

### Transcriber (`transcriber.rs`)

Loads the model once at startup. `WhisperContext` is kept alive for the app's lifetime. Each
transcription creates a new `WhisperState`, runs `full()` on the PCM buffer, extracts segment texts,
and joins them.

Model selection: `ggml-small.en.bin` (~466 MB) is the recommended default for English-only close-mic
dictation. `ggml-base.en.bin` (~142 MB) is a lighter alternative. `ggml-tiny.en.bin` (~75 MB) for
minimal resource usage. All use Metal acceleration on Apple Silicon automatically via whisper-rs's
build flags.

### Text Injector (`injector.rs`)

1. Read current `NSPasteboard.generalPasteboard` contents (all items/types) and stash them.
2. Clear pasteboard, write transcript as `NSStringPboardType`.
3. Create `CGEvent` key-down for `V` (virtual keycode 9) with `.maskCommand`, post to
   `kCGHIDEventTap`.
4. Create corresponding key-up, post it.
5. After a short delay (~50ms, via `thread::sleep`), restore original pasteboard contents.

The delay is necessary because the paste target app processes the ⌘V asynchronously. 50ms is
conservative; 20ms works in practice.

### Config (`config.rs`)

All configuration is defined as compile-time constants in `config.rs`. There is no external config
file. This is a local-only project — recompile to change settings.

```rust
pub const HOTKEY_KEY: &str = "space";
pub const HOTKEY_MODIFIERS: &[&str] = &["option"];

pub const MODEL_PATH: &str = "~/.config/whisper/models/ggml-small.en.bin";
pub const LANGUAGE: &str = "en";

pub const PASTE_DELAY_MS: u64 = 50;
```

Edit `config.rs` and `cargo build --release` to apply changes.

---

## Resource Profile

| State               | Memory                               | CPU                             |
| ------------------- | ------------------------------------ | ------------------------------- |
| Idle (model loaded) | ~80-500 MB depending on model        | 0% — event-driven, no polling   |
| Idle (model lazy)   | ~15-25 MB                            | 0%                              |
| Recording           | +3-4 MB per 60s                      | <1% (CoreAudio does the work)   |
| Transcribing        | no additional (model already loaded) | High for 1-3s (Metal GPU + CPU) |

For minimal idle footprint, lazy-load the whisper model on first use rather than at startup. First
transcription has a ~1-2s model load penalty; subsequent ones are instant.

---

## Project Layout

```
whisper/
├── Cargo.toml
├── Info.plist
├── Makefile                    # build + bundle + model download
├── src/
│   ├── main.rs                 # NSApplication setup, run loop, wire everything
│   ├── menu_bar.rs             # NSStatusItem, NSMenu, icon state
│   ├── hotkey.rs               # CGEventTap for global shortcut
│   ├── recorder.rs             # cpal mic capture → Vec<f32>
│   ├── transcriber.rs          # whisper-rs transcription
│   ├── injector.rs             # NSPasteboard + CGEvent synthetic paste
│   └── config.rs               # compile-time constants (edit + recompile to change)
└── models/                     # gitignored, populated by `make download-model`
```

### Cargo.toml (key dependencies)

```toml
[package]
name = "whisper"
version = "0.1.0"
edition = "2024"

[dependencies]
objc2 = "0.6"
objc2-foundation = { version = "0.3", features = [
    "NSString", "NSRunLoop", "NSThread", "NSNotification",
] }
objc2-app-kit = { version = "0.3", features = [
    "NSApplication", "NSStatusBar", "NSStatusItem", "NSStatusBarButton",
    "NSMenu", "NSMenuItem", "NSImage", "NSPasteboard", "NSEvent",
    "NSRunningApplication",
] }
core-graphics = "0.24"
cpal = "0.15"
whisper-rs = { version = "0.15", features = ["metal"] }
```

No `tokio`. No `async`. No proc macros. No network access. No external config files.

### Info.plist

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
  "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleIdentifier</key>
    <string>dev.local.whisper</string>
    <key>CFBundleName</key>
    <string>Whisper</string>
    <key>CFBundleExecutable</key>
    <string>whisper</string>
    <key>LSUIElement</key>
    <true/>
    <key>NSMicrophoneUsageDescription</key>
    <string>Whisper needs microphone access to record audio for transcription.</string>
</dict>
</plist>
```

`LSUIElement = true` hides the dock icon. `NSMicrophoneUsageDescription` triggers the mic permission
prompt.

### Makefile

```makefile
APP_NAME := Whisper
BIN_NAME := whisper
MODEL := ggml-small.en.bin
MODEL_URL := https://huggingface.co/ggerganov/whisper.cpp/resolve/main/$(MODEL)
MODEL_DIR := $(HOME)/.config/whisper/models
APP_BUNDLE := target/$(APP_NAME).app

.PHONY: build bundle run clean download-model setup

build:
	cargo build --release

bundle: build
	mkdir -p $(APP_BUNDLE)/Contents/MacOS
	cp target/release/$(BIN_NAME) $(APP_BUNDLE)/Contents/MacOS/
	cp Info.plist $(APP_BUNDLE)/Contents/

download-model:
	mkdir -p $(MODEL_DIR)
	@if [ ! -f "$(MODEL_DIR)/$(MODEL)" ]; then \
		echo "Downloading $(MODEL)..."; \
		curl -L -o "$(MODEL_DIR)/$(MODEL)" "$(MODEL_URL)"; \
	else \
		echo "Model already exists at $(MODEL_DIR)/$(MODEL)"; \
	fi

setup: download-model
	@echo "Add $(APP_BUNDLE) to System Settings → Privacy & Security → Accessibility"

run: bundle
	open $(APP_BUNDLE)

clean:
	cargo clean
	rm -rf $(APP_BUNDLE)
```

Full workflow: `make setup && make run`. That's it.

---

## Permissions (one-time manual steps)

1. **Microphone:** macOS prompts automatically on first recording attempt (triggered by the
   `Info.plist` key).
2. **Accessibility:** Must manually add the `.app` bundle to
   `System Settings → Privacy & Security → Accessibility`. Required for CGEventTap (global hotkey)
   and CGEventPost (synthetic paste). This is the one unavoidable manual step on macOS; there's no
   way to programmatically grant it.

No code signing needed for personal use. Unsigned apps can be granted Accessibility access.
Gatekeeper may quarantine the app on first open — `xattr -cr target/Whisper.app` removes the
quarantine flag.

---

## Model Options

| Model                           | WER     | Latency (30s audio) | Size    | Notes                                       |
| ------------------------------- | ------- | ------------------- | ------- | ------------------------------------------- |
| **ggml-small.en.bin** (default) | ~8-10%  | 2-4s on M-series    | ~466 MB | Best accuracy/speed tradeoff for dictation. |
| **ggml-base.en.bin**            | ~10-12% | 1-2s on M-series    | ~142 MB | Lighter alternative.                        |
| **ggml-tiny.en.bin**            | ~12-15% | <1s on M-series     | ~75 MB  | Minimal resource usage, noisier output.     |

All models use Metal acceleration on Apple Silicon automatically via whisper-rs's build flags.

---

## Thread Model

```
Main thread (NSApplication run loop):
  ├── CGEventTap callback → toggles recording state
  ├── Menu item actions
  └── Dispatches work to background thread

Background thread (spawned per recording cycle):
  ├── cpal input stream → collects samples
  ├── whisper-rs transcription
  └── pasteboard write + synthetic ⌘V (must dispatch back to main thread)
```

The CGEventTap callback and NSPasteboard/CGEventPost calls must happen on the main thread. The
recording and transcription happen on a background `std::thread`. Communication is via
`std::sync::mpsc` channels or a shared `Arc<Mutex<State>>`.

---

## Future Extensions

- **VAD (voice activity detection):** `whisper-cpp-plus` has Silero VAD built in. Or use
  `webrtc-vad` crate. Auto-stop after silence.
- **Audio feedback:** Play a short system sound on record start/stop via `NSSound` (accessible
  through `objc2-app-kit`).
- **Streaming transcription:** Feed audio to whisper incrementally during recording.
- **Multiple profiles:** Different model/prompt combos selectable from the menu.
