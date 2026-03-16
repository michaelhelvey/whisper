mod config;
mod hotkey;
mod injector;
mod menu_bar;
mod recorder;
mod transcriber;

use std::cell::RefCell;
use std::sync::mpsc;

use log::{error, info, warn};
use objc2::rc::Retained;
use objc2::{define_class, msg_send, sel, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::NSApplication;
use objc2_foundation::{NSObject, NSObjectProtocol, NSTimer};

use menu_bar::{IconState, MenuBar};
use recorder::Recorder;

// 8.3 — Application state machine: Idle → Recording → Transcribing → Idle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AppState {
    Idle,
    Recording,
    Transcribing,
}

// 8.7 — Orchestrator holds all shared state, accessed from the main-thread timer.
struct Orchestrator {
    state: AppState,
    menu_bar: MenuBar,
    recorder: Option<Recorder>,
    hotkey_rx: mpsc::Receiver<()>,
    transcript_rx: mpsc::Receiver<String>,
    transcript_tx: mpsc::Sender<String>,
}

thread_local! {
    static ORCHESTRATOR: RefCell<Option<Orchestrator>> = const { RefCell::new(None) };
}

// Timer target class — NSTimer calls `tick:` on each fire.
define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "TimerTarget"]
    struct TimerTarget;

    impl TimerTarget {
        #[unsafe(method(tick:))]
        fn tick(&self, _timer: *mut NSTimer) {
            ORCHESTRATOR.with(|cell| {
                let mut borrow = cell.borrow_mut();
                let orch = borrow.as_mut().expect("orchestrator not initialized");
                let mtm = MainThreadMarker::from(self);

                // Poll for hotkey presses.
                while let Ok(()) = orch.hotkey_rx.try_recv() {
                    match orch.state {
                        // 8.4 — Idle → Recording: update icon, spawn recorder.
                        AppState::Idle => {
                            orch.menu_bar.set_state(IconState::Recording, mtm);
                            match Recorder::new() {
                                Ok(mut rec) => {
                                    if let Err(e) = rec.start() {
                                        error!("failed to start recording: {e}");
                                        orch.menu_bar.set_state(IconState::Idle, mtm);
                                        continue;
                                    }
                                    orch.recorder = Some(rec);
                                    orch.state = AppState::Recording;
                                }
                                Err(e) => {
                                    error!("failed to create recorder: {e}");
                                    orch.menu_bar.set_state(IconState::Idle, mtm);
                                }
                            }
                        }
                        // 8.5 — Recording → Transcribing: stop recorder, run
                        // transcription on a background thread.
                        AppState::Recording => {
                            let pcm = orch
                                .recorder
                                .as_mut()
                                .expect("recorder must exist in Recording state")
                                .stop();
                            orch.recorder = None;
                            info!(
                                "recording stopped: {} samples ({:.1}s at 16kHz)",
                                pcm.len(),
                                pcm.len() as f64 / 16_000.0
                            );
                            orch.menu_bar.set_state(IconState::Transcribing, mtm);
                            orch.state = AppState::Transcribing;

                            let tx = orch.transcript_tx.clone();
                            std::thread::spawn(move || {
                                let text =
                                    transcriber::transcribe(&pcm).unwrap_or_else(|e| {
                                        error!("transcription error: {e}");
                                        String::new()
                                    });
                                info!("transcription result: {:?}", text);
                                let _ = tx.send(text);
                            });
                        }
                        // Ignore hotkey while transcribing.
                        AppState::Transcribing => {}
                    }
                }

                // 8.6 — Check for completed transcriptions: inject text,
                // restore icon to idle.
                if orch.state == AppState::Transcribing
                    && let Ok(text) = orch.transcript_rx.try_recv()
                {
                    if !text.is_empty() {
                        info!("injecting text: {:?}", text);
                        injector::inject(&text);
                    } else {
                        warn!("transcription returned empty text, skipping injection");
                    }
                    orch.menu_bar.set_state(IconState::Idle, mtm);
                    orch.state = AppState::Idle;
                }
            });
        }
    }

    unsafe impl NSObjectProtocol for TimerTarget {}
);

impl TimerTarget {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        unsafe { msg_send![Self::alloc(mtm), init] }
    }
}

fn main() {
    // Initialize file-based logging so we can debug when launched via `open`.
    let log_path = config::expand_tilde("~/.config/whisper/whisper.log");
    if let Ok(file) = std::fs::File::create(&log_path) {
        simplelog::WriteLogger::init(
            simplelog::LevelFilter::Info,
            simplelog::Config::default(),
            file,
        )
        .expect("failed to initialize logger");
    }

    let mtm = MainThreadMarker::new().expect("must run on the main thread");

    // 8.1 — Initialize menu bar (NSApplication, status item, menu).
    let menu_bar = MenuBar::new(mtm);

    // 8.2 — Initialize global hotkey event tap.
    let hotkey_rx = hotkey::install();

    // 8.7 — Set up communication channels for transcription results.
    let (transcript_tx, transcript_rx) = mpsc::channel();

    // Store orchestrator state in thread-local for timer access.
    ORCHESTRATOR.with(|cell| {
        *cell.borrow_mut() = Some(Orchestrator {
            state: AppState::Idle,
            menu_bar,
            recorder: None,
            hotkey_rx,
            transcript_rx,
            transcript_tx,
        });
    });

    // Set up a repeating NSTimer (~10ms) to poll channels from the main run loop.
    let timer_target = TimerTarget::new(mtm);
    let _timer = unsafe {
        NSTimer::scheduledTimerWithTimeInterval_target_selector_userInfo_repeats(
            0.01,
            &timer_target,
            sel!(tick:),
            None,
            true,
        )
    };

    // Run the NSApplication event loop (blocks forever).
    let app = NSApplication::sharedApplication(mtm);
    app.run();
}
