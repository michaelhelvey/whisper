/// Virtual keycode for the hotkey (49 = Space).
pub const HOTKEY_KEYCODE: u16 = 49;

/// Path to the whisper model file (tilde-expanded at runtime via `model_path()`).
const MODEL_PATH: &str = "~/.config/whisper/models/ggml-small.en.bin";

/// Language code for whisper transcription.
pub const LANGUAGE: &str = "en";

/// Delay in milliseconds after synthetic paste before restoring the pasteboard.
pub const PASTE_DELAY_MS: u64 = 500;

/// Returns the expanded model path, replacing a leading `~` with the user's home directory.
pub fn model_path() -> String {
    expand_tilde(MODEL_PATH)
}

/// Expands a leading `~` in `path` to the value of the `HOME` environment variable.
pub fn expand_tilde(path: &str) -> String {
    if let Some(rest) = path.strip_prefix('~')
        && let Ok(home) = std::env::var("HOME")
    {
        return format!("{home}{rest}");
    }
    path.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tilde_expansion_replaces_home() {
        let expanded = expand_tilde("~/foo/bar");
        assert!(!expanded.starts_with('~'));
        assert!(expanded.ends_with("/foo/bar"));
    }

    #[test]
    fn no_tilde_unchanged() {
        assert_eq!(expand_tilde("/absolute/path"), "/absolute/path");
    }

    #[test]
    fn model_path_is_absolute() {
        let p = model_path();
        assert!(p.starts_with('/'), "model_path should be absolute: {p}");
        assert!(p.ends_with("ggml-small.en.bin"));
    }
}
