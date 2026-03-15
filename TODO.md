# Whisper — Task Breakdown

Architecture document (read this first): ./ARCHITECTURE.md

## 1. Project Scaffolding

- [x] **1.1** Create `Cargo.toml` with all dependencies (objc2, objc2-foundation, objc2-app-kit,
      core-graphics, cpal, whisper-rs)
- [x] **1.2** Create `Info.plist` with bundle identifier, LSUIElement, and
      NSMicrophoneUsageDescription
- [x] **1.3** Create `Makefile` with build, bundle, download-model, setup, run, and clean targets
- [x] **1.4** Create project layout: `src/main.rs`, empty module files (`menu_bar.rs`, `hotkey.rs`,
      `recorder.rs`, `transcriber.rs`, `injector.rs`, `config.rs`)
- [x] **1.5** Add `models/` to `.gitignore`
- [x] **1.6** Verify `cargo build` succeeds with stub modules

## 2. Config (`config.rs`)

- [x] **2.1** Define compile-time constants for hotkey (key + modifiers), model path, language, and
      paste delay
- [x] **2.2** Implement tilde expansion helper for `MODEL_PATH` at runtime

## 3. Menu Bar (`menu_bar.rs`)

- [x] **3.1** Set up `NSApplication` with `setActivationPolicy(.accessory)` — no dock icon, no main
      window
- [x] **3.2** Create `NSStatusItem` on the system status bar with a text title (`"🎤"`)
- [x] **3.3** Create `NSMenu` with two items: disabled status label and "Quit"
- [x] **3.4** Implement the Objective-C delegate class via `define_class!` macro for menu item
      actions
- [x] **3.5** Wire "Quit" to `NSApplication::terminate`
- [x] **3.6** Implement icon state changes: idle (`"🎤"`), recording (`"🔴"`), transcribing (`"⏳"`)

## 4. Global Hotkey (`hotkey.rs`)

- [x] **4.1** Create a `CGEventTap` with `kCGEventKeyDown` mask using `CGEvent::tapCreate`
- [x] **4.2** Implement the tap callback: inspect keycode and modifier flags for the hard-coded
      combo (default: ⌥+Space)
- [x] **4.3** On match, swallow the event (return `nil`) and signal a toggle
- [x] **4.4** Add the event tap to the current `CFRunLoop` (shared with the NSApplication run loop)
- [x] **4.5** Expose a mechanism (channel or callback) to notify `main.rs` of hotkey presses

## 5. Audio Recorder (`recorder.rs`)

- [x] **5.1** Open the default input device via `cpal` and query its supported configs
- [x] **5.2** Build an input stream at 16kHz mono f32 (or native rate if 16kHz unsupported)
- [x] **5.3** Implement `start()`: begin stream, push samples into `Arc<Mutex<Vec<f32>>>`
- [x] **5.4** Implement `stop() -> Vec<f32>`: drop the stream, return accumulated buffer
- [x] **5.5** If recorded at non-16kHz rate, resample to 16kHz using `rubato` (add dependency if
      needed)

## 6. Transcriber (`transcriber.rs`)

- [x] **6.1** Implement lazy model loading: load `WhisperContext` from `config::MODEL_PATH` on
      first use
- [x] **6.2** Implement `transcribe(pcm: &[f32]) -> String`: create `WhisperState`, run `full()`,
      extract and join segment texts
- [x] **6.3** Configure whisper params: language from `config::LANGUAGE`, single-segment mode
      suitable for dictation

## 7. Text Injector (`injector.rs`)

- [x] **7.1** Read and stash current `NSPasteboard.generalPasteboard` contents (all items/types)
- [x] **7.2** Clear pasteboard and write transcript string as `NSStringPboardType`
- [x] **7.3** Create and post `CGEvent` key-down + key-up for `⌘V` (keycode 9 + command flag) to
      `kCGHIDEventTap`
- [x] **7.4** After a 50ms `thread::sleep`, restore original pasteboard contents

## 8. Main Orchestration (`main.rs`)

- [x] **8.1** Initialize menu bar (NSApplication, status item, menu)
- [ ] **8.2** Initialize global hotkey event tap
- [ ] **8.3** Implement state machine: Idle → Recording → Transcribing → Pasting → Idle
- [ ] **8.4** On hotkey press while idle: update icon to recording, spawn background thread, start
      recorder
- [ ] **8.5** On hotkey press while recording: stop recorder, update icon to transcribing, run
      transcription on background thread
- [ ] **8.6** After transcription: dispatch back to main thread, inject text, restore icon to idle
- [ ] **8.7** Wire communication between components via `std::sync::mpsc` channels or
      `Arc<Mutex<State>>`

## 9. Build & Bundle

- [ ] **9.1** Verify `make build` produces a working release binary
- [ ] **9.2** Verify `make bundle` assembles a valid `.app` bundle with Info.plist and binary
- [ ] **9.3** Verify `make download-model` fetches the whisper model to `~/.config/whisper/models/`
- [ ] **9.4** Remove quarantine with `xattr -cr` and test `make run` launches the menu bar app

## 10. Permissions & Smoke Test

- [ ] **10.1** Confirm microphone permission prompt appears on first recording attempt
- [ ] **10.2** Add `.app` to Accessibility in System Settings, confirm hotkey works globally
- [ ] **10.3** End-to-end test: press hotkey → record → release hotkey → transcript pasted into a
      text field
