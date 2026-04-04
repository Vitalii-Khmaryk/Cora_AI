//! Audio recording with adaptive VAD, beep feedback, and utility functions.
//!
//! Recording now returns raw WAV bytes in memory (no disk I/O).

use std::f32::consts::PI;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use log::{error, info, warn};

use crate::constants::*;

// ═══════════════════════════════════════════════════════════════════
// Audio Beep Feedback (sine wave via cpal output)
// ═══════════════════════════════════════════════════════════════════

/// Plays a short sine-wave beep at the given frequency and duration.
/// Uses the default audio *output* device. Non-blocking: spawns a thread.
pub fn play_beep(frequency: f32, duration_ms: u64) {
    std::thread::spawn(move || {
        if let Err(e) = play_beep_blocking(frequency, duration_ms) {
            warn!("Beep failed (non-fatal): {e:#}");
        }
    });
}

fn play_beep_blocking(frequency: f32, duration_ms: u64) -> Result<()> {
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .context("No audio output device for beep")?;
    let config = device.default_output_config()?;
    let sample_rate = config.sample_rate().0 as f32;
    let channels = config.channels() as usize;
    let total_samples = (sample_rate * duration_ms as f32 / 1000.0) as usize;

    let sample_idx = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let done = Arc::new(AtomicBool::new(false));

    let idx = sample_idx.clone();
    let finished = done.clone();

    let stream_config: cpal::StreamConfig = config.into();
    let stream = device.build_output_stream(
        &stream_config,
        move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
            for frame in data.chunks_mut(channels) {
                let i = idx.fetch_add(1, Ordering::Relaxed);
                if i >= total_samples {
                    finished.store(true, Ordering::Relaxed);
                    for s in frame.iter_mut() {
                        *s = 0.0;
                    }
                } else {
                    // Sine wave with quick fade-in/out to avoid clicks
                    let t = i as f32 / sample_rate;
                    let mut sample = (2.0 * PI * frequency * t).sin() * 0.3;

                    // 5ms fade envelope
                    let fade_samples = (sample_rate * 0.005) as usize;
                    if i < fade_samples {
                        sample *= i as f32 / fade_samples as f32;
                    } else if i > total_samples - fade_samples {
                        sample *= (total_samples - i) as f32 / fade_samples as f32;
                    }

                    for s in frame.iter_mut() {
                        *s = sample;
                    }
                }
            }
        },
        |e| error!("Beep stream error: {e}"),
        None,
    )?;
    stream.play()?;

    // Wait for playback to complete
    while !done.load(Ordering::Relaxed) {
        std::thread::sleep(Duration::from_millis(5));
    }
    // Brief tail to flush the audio buffer
    std::thread::sleep(Duration::from_millis(20));
    drop(stream);
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════
// Smart Audio Recording with Adaptive VAD
// ═══════════════════════════════════════════════════════════════════

/// Records audio from the default microphone with Voice Activity Detection.
///
/// Returns a complete WAV file as an in-memory `Vec<u8>` — **no disk I/O**.
///
/// **Adaptive threshold:** The first 300ms of recording is used to measure
/// ambient noise. The silence threshold is set to `ambient_rms × 3.0`,
/// with a floor of `0.008`. This means Cora automatically adapts to both
/// quiet bedrooms and noisy LAN parties.
///
/// **Auto-stop:** Recording stops when silence is detected for 1.5 seconds
/// after voice activity, or after 15 seconds (hard timeout).
pub async fn record_with_vad(audio_amplitude: Arc<AtomicU32>) -> Result<Vec<u8>> {
    tokio::task::spawn_blocking(move || -> Result<Vec<u8>> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .context("No audio input device found for recording")?;
        let supported = device.default_input_config()?;
        let sample_rate = supported.sample_rate().0;
        let channels = supported.channels() as usize;

        info!(
            "Recording at {sample_rate} Hz, {channels}ch — speak now (max {}s)...",
            MAX_RECORDING_DURATION.as_secs()
        );

        let (audio_tx, audio_rx) = std::sync::mpsc::channel::<Vec<f32>>();
        let stream_config: cpal::StreamConfig = supported.into();

        let stream = device.build_input_stream(
            &stream_config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                let _ = audio_tx.send(data.to_vec());
            },
            |e| error!("Recording stream error: {e}"),
            None,
        )?;
        stream.play()?;

        // Pre-allocate buffer for the max possible recording
        let max_samples =
            sample_rate as usize * MAX_RECORDING_DURATION.as_secs() as usize * channels;
        let mut all_samples: Vec<f32> = Vec::with_capacity(max_samples);
        let start = Instant::now();

        // ── Adaptive calibration state ──
        let mut calibration_rms_values: Vec<f32> = Vec::new();
        let mut threshold = MIN_SILENCE_THRESHOLD;
        let mut calibrated = false;

        // ── VAD state ──
        let mut last_voice_time = Instant::now();
        let mut voice_detected = false;

        loop {
            // Receive audio chunks with a timeout to periodically check hard timeout
            let chunk = match audio_rx.recv_timeout(Duration::from_millis(100)) {
                Ok(c) => c,
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    if start.elapsed() > MAX_RECORDING_DURATION {
                        info!("Hard timeout reached ({MAX_RECORDING_DURATION:?}), stopping.");
                        break;
                    }
                    continue;
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    warn!("Recording audio channel disconnected.");
                    break;
                }
            };

            let rms = calculate_rms(&chunk);
            audio_amplitude.store(rms.to_bits(), Ordering::Relaxed);
            all_samples.extend_from_slice(&chunk);

            // ── Calibration (first 300ms) ──
            // Measure ambient noise to set an adaptive threshold.
            if !calibrated {
                calibration_rms_values.push(rms);

                if start.elapsed() >= CALIBRATION_DURATION {
                    let ambient_avg: f32 = calibration_rms_values.iter().sum::<f32>()
                        / calibration_rms_values.len() as f32;
                    let multiplier = crate::constants::get_silence_multiplier();
                    threshold =
                        (ambient_avg * multiplier).clamp(MIN_SILENCE_THRESHOLD, 0.02);
                    calibrated = true;
                    info!(
                        "Calibrated: ambient={ambient_avg:.5}, threshold={threshold:.5} \
                         ({}× ambient, floor={MIN_SILENCE_THRESHOLD})",
                        multiplier
                    );
                }
                continue; // Don't apply VAD during calibration
            }

            // ── Voice Activity Detection ──
            if rms > threshold {
                if !voice_detected {
                    voice_detected = true;
                    info!("Voice activity detected (RMS: {rms:.4})");
                }
                last_voice_time = Instant::now();
            }

            // Auto-stop: silence for SILENCE_DURATION after voice was detected
            if voice_detected && last_voice_time.elapsed() > SILENCE_DURATION {
                info!("Silence detected for {SILENCE_DURATION:?} — stopping recording.");
                break;
            }

            // Hard timeout
            if start.elapsed() > MAX_RECORDING_DURATION {
                info!("Hard timeout reached ({MAX_RECORDING_DURATION:?}), stopping.");
                break;
            }
        }

        // Explicitly stop and drop the stream
        drop(stream);

        // ── Convert stereo → mono if needed ──
        let mono: Vec<f32> = if channels > 1 {
            all_samples
                .chunks(channels)
                .map(|frame| frame.iter().sum::<f32>() / channels as f32)
                .collect()
        } else {
            all_samples
        };

        // ── Encode as 16-bit PCM WAV in memory (zero disk I/O) ──
        let wav_bytes = encode_wav_in_memory(&mono, sample_rate);

        let duration_secs = mono.len() as f32 / sample_rate as f32;
        info!(
            "Recorded {duration_secs:.1}s → {} bytes in memory",
            wav_bytes.len()
        );

        audio_amplitude.store(0.0f32.to_bits(), Ordering::Relaxed);
        Ok(wav_bytes)
    })
    .await?
}

// ═══════════════════════════════════════════════════════════════════
// Utility Functions
// ═══════════════════════════════════════════════════════════════════

/// Calculate the Root Mean Square (RMS) amplitude of an audio buffer.
/// Returns 0.0 for empty buffers.
pub fn calculate_rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum_squares: f32 = samples.iter().map(|&s| s * s).sum();
    (sum_squares / samples.len() as f32).sqrt()
}



/// Encodes f32 mono audio samples as a 16-bit PCM WAV file entirely in RAM.
///
/// Writes the canonical 44-byte RIFF/WAV header followed by the PCM data.
/// Returns the complete WAV file as a byte vector — no disk I/O.
pub fn encode_wav_in_memory(samples: &[f32], sample_rate: u32) -> Vec<u8> {
    let num_channels: u16 = 1;
    let bits_per_sample: u16 = 16;
    let byte_rate = sample_rate * num_channels as u32 * bits_per_sample as u32 / 8;
    let block_align = num_channels * bits_per_sample / 8;
    let data_size = samples.len() as u32 * (bits_per_sample / 8) as u32;
    let file_size = 36 + data_size; // Total file size minus 8 bytes for RIFF header

    let mut buf = Vec::with_capacity(44 + data_size as usize);

    // ── RIFF header ──
    buf.extend_from_slice(b"RIFF");
    buf.extend_from_slice(&file_size.to_le_bytes());
    buf.extend_from_slice(b"WAVE");

    // ── fmt subchunk ──
    buf.extend_from_slice(b"fmt ");
    buf.extend_from_slice(&16u32.to_le_bytes()); // Subchunk1Size (PCM = 16)
    buf.extend_from_slice(&1u16.to_le_bytes()); // AudioFormat (PCM = 1)
    buf.extend_from_slice(&num_channels.to_le_bytes());
    buf.extend_from_slice(&sample_rate.to_le_bytes());
    buf.extend_from_slice(&byte_rate.to_le_bytes());
    buf.extend_from_slice(&block_align.to_le_bytes());
    buf.extend_from_slice(&bits_per_sample.to_le_bytes());

    // ── data subchunk ──
    buf.extend_from_slice(b"data");
    buf.extend_from_slice(&data_size.to_le_bytes());

    for &sample in samples {
        let clamped = sample.clamp(-1.0, 1.0);
        let pcm16 = (clamped * i16::MAX as f32) as i16;
        buf.extend_from_slice(&pcm16.to_le_bytes());
    }

    buf
}
