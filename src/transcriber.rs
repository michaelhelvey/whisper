#![allow(dead_code)]

use std::sync::Mutex;

use whisper_rs::{
    FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters, WhisperError,
};

use crate::config;

/// Global lazily-initialized whisper context. Loaded on first call to `transcribe()`.
static CONTEXT: Mutex<Option<WhisperContext>> = Mutex::new(None);

/// Returns a reference-counted handle to the loaded context, loading it if necessary.
fn with_context<T>(f: impl FnOnce(&WhisperContext) -> Result<T, WhisperError>) -> Result<T, WhisperError> {
    let mut guard = CONTEXT.lock().expect("whisper context lock poisoned");
    if guard.is_none() {
        let path = config::model_path();
        let ctx = WhisperContext::new_with_params(&path, WhisperContextParameters::default())?;
        *guard = Some(ctx);
    }
    f(guard.as_ref().unwrap())
}

/// Transcribes 16 kHz mono f32 PCM audio into text.
///
/// Lazily loads the whisper model on first invocation. Subsequent calls reuse the loaded context.
/// Returns the concatenated text of all segments produced by whisper.
pub fn transcribe(pcm: &[f32]) -> Result<String, WhisperError> {
    with_context(|ctx| {
        let mut state = ctx.create_state()?;

        let mut params = FullParams::new(SamplingStrategy::BeamSearch {
            beam_size: 5,
            patience: -1.0,
        });

        // Configure language for dictation.
        params.set_language(Some(config::LANGUAGE));

        // Single-segment mode is suitable for short dictation snippets.
        params.set_single_segment(true);

        // Suppress stdout noise from whisper.cpp.
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);

        state.full(params, pcm)?;

        let text: String = state
            .as_iter()
            .map(|seg| seg.to_string())
            .collect::<Vec<_>>()
            .join("");

        Ok(text.trim().to_string())
    })
}
