# Whisper — Architecture

A macOS menu bar utility that records audio via a global hotkey, transcribes it locally with
whisper.cpp, and pastes the result into whatever text field is focused.

Built entirely in Rust. No Xcode project, no Swift, no async runtime. `cargo build --release` and a
Makefile to assemble the `.app` bundle.

---

## Flow

```
[Ctrl+Space pressed] → recording starts, icon turns 🔴
[Ctrl+Space pressed] → recording stops
  → PCM buffer (16 kHz mono f32) sent to whisper.cpp
  → transcript text placed on pasteboard
  → synthetic ⌘V fired
  → original pasteboard contents restored after 500 ms
  → icon returns to 🎤 idle
```

All of this happens in-process. No subprocesses, no IPC, no sidecar binaries.

---

## Crate Map

| Concern              | Crate                                          | Why                                                                                                |
| -------------------- | ---------------------------------------------- | -------------------------------------------------------------------------------------------------- |
| Menu bar icon + menu | `objc2` + `objc2-app-kit` + `objc2-foundation` | Direct NSStatusBar/NSMenu/NSPasteboard access. No wrapper abstraction to fight.                    |
| Audio capture        | `cpal`                                         | Pure Rust, CoreAudio backend on macOS. Gives raw PCM callbacks from the mic.                       |
| Transcription        | `whisper-rs`                                   | Mature whisper.cpp bindings. Compiles whisper.cpp from source with Metal support.                   |
| Global hotkey        | `core-graphics`                                | CGEventTap for intercepting keyboard events.                                                       |
| Text injection       | `core-graphics` + `objc2-app-kit`              | CGEvent for synthetic ⌘V, NSPasteboard for clipboard manipulation.                                 |
| Logging              | `libc`                                         | `dup2` to redirect stderr to a log file for debugging when launched as a bundle.                   |
| Config               | (none — compile-time constants)                | Hard-coded in `config.rs`. Recompile to change.                                                    |

### Why no tokio / async

This app does three things sequentially: record → transcribe → paste. The `NSApplication` run loop
is the only event loop. A 10 ms `NSTimer` polls channels for hotkey presses and transcription
results. Background work (recording, transcription) runs on plain `std::thread`s.

### Why `core-graphics` and not a higher-level hotkey crate

The existing Rust hotkey crates pull in event loop abstractions that conflict with a raw
`NSApplication` run loop. `CGEventTap` is ~30 lines of code via the `core-graphics` crate and is
the same API Swift apps use. We need `core-graphics` anyway for the synthetic paste keystroke.

---

## Component Design

### Menu Bar (`menu_bar.rs`)

Uses `objc2-app-kit` to create an `NSStatusItem` with a text title (`"🎤"` idle, `"🔴"` recording,
`"⏳"` transcribing). Text titles avoid needing to bundle template images.

The `NSMenu` has two items: a disabled status label showing the current state, and "Quit".

Setup:
1. `NSApplication::sharedApplication()` with `setActivationPolicy(.accessory)` — no dock icon.
2. `NSStatusBar::systemStatusBar().statusItemWithLength(NSVariableStatusItemLength)`.
3. Assign an `NSMenu` to the status item.

Menu item actions use `objc2`'s `define_class!` macro to create a small Objective-C class that
handles the "Quit" action via `NSApplication::terminate`.

### Global Hotkey (`hotkey.rs`)

`CGEventTap` with a `KeyDown` mask. The callback inspects each key event for **Ctrl+Space**
(keycode 49 + `kCGEventFlagMaskControl`). On match, returns `None` to swallow the event and sends a
signal through an `mpsc` channel.

Requires Accessibility permission. The terminal (e.g. Ghostty) must be granted Accessibility access
in System Settings; the binary inherits this permission when launched from the terminal via
`make run`. If the event tap fails to create (no permission), a warning is logged and the app
continues running without hotkey support.

The event tap is added to the current `CFRunLoop`, shared with the `NSApplication` run loop.

### Audio Recorder (`recorder.rs`)

Uses `cpal` to open the default input device. Prefers 16 kHz mono f32; falls back to the device's
default sample rate and resamples to 16 kHz on stop using linear interpolation.

- `start()` — opens the stream, pushes samples into an `Arc<Mutex<Vec<f32>>>`.
- `stop() -> Vec<f32>` — drops the stream, returns the buffer (resampled if necessary).

A 60-second recording at 16 kHz mono f32 is ~3.8 MB. No temp files.

### Transcriber (`transcriber.rs`)

Lazily loads the whisper model on first use. `WhisperContext` is held in a global `Mutex` for the
app's lifetime. Each transcription creates a `WhisperState`, runs `full()` with beam search
(beam size 5, single-segment mode), and joins the resulting segment texts.

Model: `ggml-small.en.bin` by default. Metal acceleration on Apple Silicon is enabled automatically
via whisper-rs build flags.

### Text Injector (`injector.rs`)

1. Save all items/types from `NSPasteboard.generalPasteboard`.
2. Clear pasteboard, write transcript as `NSPasteboardTypeString`.
3. Post `CGEvent` key-down + key-up for `V` (keycode 9) with Command flag to `kCGHIDEventTap`.
4. On a background thread, wait 500 ms, then restore original pasteboard contents.

The 500 ms delay gives the target app time to process the paste event asynchronously.

### Config (`config.rs`)

All configuration is compile-time constants:

| Constant             | Value                                       | Description                          |
| -------------------- | ------------------------------------------- | ------------------------------------ |
| `HOTKEY_KEYCODE`     | `49` (Space)                                | Virtual keycode for the hotkey       |
| `HOTKEY_MODIFIER_FLAGS` | `0x00040000` (Control)                   | Modifier mask                        |
| `MODEL_PATH`         | `~/.config/whisper/models/ggml-small.en.bin`| Tilde-expanded at runtime            |
| `LANGUAGE`           | `"en"`                                      | Whisper language code                |
| `PASTE_DELAY_MS`     | `500`                                       | ms before restoring pasteboard       |

Edit `config.rs` and `cargo build --release` to change.

### Main Orchestration (`main.rs`)

Redirects stderr to `~/.config/whisper/whisper.log` on startup for debugging.

Implements a state machine polled by a 10 ms `NSTimer`:

```
Idle → (hotkey) → Recording → (hotkey) → Transcribing → (done) → Idle
```

- **Idle → Recording:** update icon, create `Recorder`, start stream.
- **Recording → Transcribing:** stop recorder, spawn `std::thread` for transcription.
- **Transcribing → Idle:** receive transcript via `mpsc`, inject text, restore icon.

Hotkey presses during transcription are ignored.

---

## Thread Model

```
Main thread (NSApplication run loop + 10 ms NSTimer):
  ├── CGEventTap callback → sends () through mpsc channel
  ├── Timer polls hotkey_rx and transcript_rx channels
  ├── State machine transitions
  └── Menu item actions (Quit)

Background thread (spawned per recording cycle):
  ├── whisper-rs transcription
  └── Sends result through mpsc channel

cpal audio thread (managed by CoreAudio):
  └── Pushes samples into Arc<Mutex<Vec<f32>>>

Background thread (spawned per paste):
  └── Sleeps 500 ms, restores pasteboard
```

---

## Permissions

1. **Microphone** — macOS prompts automatically on first recording (triggered by the `Info.plist`
   `NSMicrophoneUsageDescription` key).
2. **Accessibility** — the terminal running `make run` must have Accessibility access in System
   Settings. The binary inherits this permission. Required for `CGEventTap` (global hotkey) and
   `CGEventPost` (synthetic paste).

---

## Project Layout

```
whisper/
├── Cargo.toml
├── Info.plist                  # bundle metadata, mic permission string
├── Makefile                    # build, bundle, model download, run
├── ARCHITECTURE.md
├── README.md
├── AGENTS.md
└── src/
    ├── main.rs                 # NSApplication setup, state machine, timer
    ├── menu_bar.rs             # NSStatusItem, NSMenu, icon states
    ├── hotkey.rs               # CGEventTap for Ctrl+Space
    ├── recorder.rs             # cpal mic capture → Vec<f32>
    ├── transcriber.rs          # whisper-rs lazy model loading + transcription
    ├── injector.rs             # NSPasteboard + CGEvent synthetic paste
    └── config.rs               # compile-time constants
```
