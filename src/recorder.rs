use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, SampleRate, Stream, StreamConfig};

/// Audio recorder that captures microphone input into a buffer.
///
/// Uses `cpal` to open the default input device and record 16 kHz mono f32 samples.
/// If the hardware doesn't support 16 kHz natively, records at the device's default
/// sample rate and resamples to 16 kHz on `stop()`.
pub struct Recorder {
    device: Device,
    config: StreamConfig,
    native_sample_rate: u32,
    buffer: Arc<Mutex<Vec<f32>>>,
    stream: Option<Stream>,
}

/// Target sample rate for whisper transcription.
const TARGET_SAMPLE_RATE: u32 = 16_000;

impl Recorder {
    /// Creates a new `Recorder` by opening the default input device and selecting a config.
    ///
    /// Prefers 16 kHz mono f32. Falls back to the device's default input config if 16 kHz
    /// is not directly supported.
    pub fn new() -> Result<Self, String> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or("no default input device available")?;

        let default_config = device
            .default_input_config()
            .map_err(|e| format!("failed to get default input config: {e}"))?;

        // Try to find a config that supports 16 kHz mono.
        let (config, native_sample_rate) = if let Some(cfg) = find_16khz_config(&device) {
            (cfg, TARGET_SAMPLE_RATE)
        } else {
            // Fall back to the default config (we'll resample later).
            let rate = default_config.sample_rate().0;
            let mut cfg: StreamConfig = default_config.into();
            cfg.channels = 1;
            (cfg, rate)
        };

        Ok(Self {
            device,
            config,
            native_sample_rate,
            buffer: Arc::new(Mutex::new(Vec::new())),
            stream: None,
        })
    }

    /// Starts recording. Samples are pushed into an internal buffer.
    ///
    /// Does nothing if already recording.
    pub fn start(&mut self) -> Result<(), String> {
        if self.stream.is_some() {
            return Ok(());
        }

        // Clear buffer for a fresh recording.
        self.buffer.lock().unwrap().clear();

        let buffer = Arc::clone(&self.buffer);
        let channels = self.config.channels as usize;

        let stream = self
            .device
            .build_input_stream(
                &self.config,
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    let mut buf = buffer.lock().unwrap();
                    if channels == 1 {
                        buf.extend_from_slice(data);
                    } else {
                        // Mix down to mono by averaging channels.
                        for chunk in data.chunks(channels) {
                            let sample: f32 = chunk.iter().sum::<f32>() / channels as f32;
                            buf.push(sample);
                        }
                    }
                },
                move |err| {
                    log::error!("audio input stream error: {err}");
                },
                None, // no timeout
            )
            .map_err(|e| format!("failed to build input stream: {e}"))?;

        stream
            .play()
            .map_err(|e| format!("failed to start input stream: {e}"))?;

        self.stream = Some(stream);
        Ok(())
    }

    /// Stops recording and returns the accumulated PCM buffer at 16 kHz mono f32.
    ///
    /// If the device recorded at a different sample rate, the buffer is resampled to 16 kHz.
    /// Returns an empty `Vec` if `start()` was never called.
    pub fn stop(&mut self) -> Vec<f32> {
        // Drop the stream to stop recording.
        self.stream.take();

        let samples = {
            let mut buf = self.buffer.lock().unwrap();
            std::mem::take(&mut *buf)
        };

        if samples.is_empty() {
            return samples;
        }

        if self.native_sample_rate == TARGET_SAMPLE_RATE {
            samples
        } else {
            resample(&samples, self.native_sample_rate, TARGET_SAMPLE_RATE)
        }
    }
}

/// Attempts to find a supported input config at 16 kHz mono f32.
fn find_16khz_config(device: &Device) -> Option<StreamConfig> {
    let supported = device.supported_input_configs().ok()?;
    for range in supported {
        if range.channels() == 1
            && range.min_sample_rate() <= SampleRate(TARGET_SAMPLE_RATE)
            && range.max_sample_rate() >= SampleRate(TARGET_SAMPLE_RATE)
            && range.sample_format() == cpal::SampleFormat::F32
        {
            let cfg = range
                .with_sample_rate(SampleRate(TARGET_SAMPLE_RATE))
                .config();
            return Some(cfg);
        }
    }
    None
}

/// Resamples a mono f32 PCM buffer from `from_rate` to `to_rate` using linear interpolation.
fn resample(samples: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    if from_rate == to_rate || samples.is_empty() {
        return samples.to_vec();
    }

    let ratio = from_rate as f64 / to_rate as f64;
    let output_len = (samples.len() as f64 / ratio).ceil() as usize;
    let mut output = Vec::with_capacity(output_len);

    for i in 0..output_len {
        let src_idx = i as f64 * ratio;
        let idx_floor = src_idx.floor() as usize;
        let frac = src_idx - idx_floor as f64;

        let sample = if idx_floor + 1 < samples.len() {
            samples[idx_floor] as f64 * (1.0 - frac) + samples[idx_floor + 1] as f64 * frac
        } else if idx_floor < samples.len() {
            samples[idx_floor] as f64
        } else {
            0.0
        };

        output.push(sample as f32);
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resample_same_rate_is_identity() {
        let input = vec![1.0, 2.0, 3.0, 4.0];
        let output = resample(&input, 16_000, 16_000);
        assert_eq!(input, output);
    }

    #[test]
    fn resample_empty_returns_empty() {
        let output = resample(&[], 48_000, 16_000);
        assert!(output.is_empty());
    }

    #[test]
    fn resample_downsample_reduces_length() {
        // 48kHz -> 16kHz should produce roughly 1/3 the samples.
        let input: Vec<f32> = (0..4800).map(|i| (i as f32).sin()).collect();
        let output = resample(&input, 48_000, 16_000);
        // Should be approximately 1600 samples.
        assert!(
            (output.len() as i64 - 1600).abs() <= 1,
            "expected ~1600 samples, got {}",
            output.len()
        );
    }

    #[test]
    fn resample_upsample_increases_length() {
        // 8kHz -> 16kHz should produce roughly 2x the samples.
        let input: Vec<f32> = (0..800).map(|i| (i as f32).sin()).collect();
        let output = resample(&input, 8_000, 16_000);
        assert!(
            (output.len() as i64 - 1600).abs() <= 1,
            "expected ~1600 samples, got {}",
            output.len()
        );
    }
}
