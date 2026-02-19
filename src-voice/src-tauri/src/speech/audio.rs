use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use rubato::{Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction};
use std::sync::{Arc, Mutex};
use tracing::{info, warn};

#[derive(serde::Serialize, Clone, Debug)]
pub struct AudioDeviceInfo {
    pub name: String,
    pub is_default: bool,
}

pub fn list_input_devices() -> Result<Vec<AudioDeviceInfo>> {
    let host = cpal::default_host();
    let default_name = host
        .default_input_device()
        .and_then(|d| d.name().ok());

    let devices = host
        .input_devices()
        .context("Failed to enumerate input devices")?;

    let mut result = Vec::new();
    for device in devices {
        if let Ok(name) = device.name() {
            let is_default = default_name.as_deref() == Some(&name);
            result.push(AudioDeviceInfo { name, is_default });
        }
    }
    Ok(result)
}

pub struct AudioRecorder;

impl AudioRecorder {
    /// Run audio recording on the current thread until stop_signal is set.
    /// All state is passed via Arc<Mutex<_>> so nothing !Send leaks out.
    pub fn run_blocking(
        buffer: Arc<Mutex<Vec<f32>>>,
        device_rate_out: Arc<Mutex<u32>>,
        stop_signal: Arc<Mutex<bool>>,
        device_name: Option<String>,
        gain: f32,
    ) -> Result<()> {
        let host = cpal::default_host();
        let device = if let Some(ref name) = device_name {
            host.input_devices()
                .context("Failed to enumerate input devices")?
                .find(|d| d.name().ok().as_deref() == Some(name.as_str()))
                .or_else(|| {
                    warn!("Device {:?} not found, falling back to default", name);
                    host.default_input_device()
                })
                .context("No input audio device available")?
        } else {
            host.default_input_device()
                .context("No input audio device available")?
        };

        let config = device
            .default_input_config()
            .context("Failed to get default input config")?;

        let sample_rate = config.sample_rate().0;
        let channels = config.channels();

        info!(
            "Audio device: {:?}, sample_rate: {}, channels: {}",
            device.name().unwrap_or_default(),
            sample_rate,
            channels
        );

        // Store device sample rate so caller can resample later
        {
            let mut rate = device_rate_out.lock().unwrap();
            *rate = sample_rate;
        }

        let buf_clone = buffer.clone();
        let err_fn = |err: cpal::StreamError| {
            warn!("Audio stream error: {}", err);
        };

        let stream_config: cpal::StreamConfig = config.into();
        let stream = device.build_input_stream(
            &stream_config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                let mut buf = buf_clone.lock().unwrap();
                if channels > 1 {
                    for chunk in data.chunks(channels as usize) {
                        let mono: f32 =
                            chunk.iter().sum::<f32>() / channels as f32 * gain;
                        buf.push(mono);
                    }
                } else if (gain - 1.0).abs() < f32::EPSILON {
                    buf.extend_from_slice(data);
                } else {
                    buf.extend(data.iter().map(|&s| s * gain));
                }
            },
            err_fn,
            None,
        )?;

        stream.play()?;
        info!("Audio recording started");

        // Poll stop signal (10ms intervals)
        loop {
            std::thread::sleep(std::time::Duration::from_millis(10));
            let stop = stop_signal.lock().unwrap();
            if *stop {
                break;
            }
        }

        // Stream is dropped here, stopping recording
        drop(stream);
        info!("Audio recording stopped");
        Ok(())
    }
}

pub fn resample(samples: &[f32], from_rate: u32, to_rate: u32) -> Result<Vec<f32>> {
    if from_rate == to_rate {
        return Ok(samples.to_vec());
    }

    let ratio = to_rate as f64 / from_rate as f64;
    let params = SincInterpolationParameters {
        sinc_len: 256,
        f_cutoff: 0.95,
        interpolation: SincInterpolationType::Linear,
        oversampling_factor: 256,
        window: WindowFunction::BlackmanHarris2,
    };

    let mut resampler = SincFixedIn::<f32>::new(
        ratio,
        2.0,
        params,
        samples.len(),
        1, // mono
    )?;

    let input = vec![samples.to_vec()];
    let output = resampler.process(&input, None)?;
    Ok(output.into_iter().next().unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resample_48k_to_16k() {
        let from_rate = 48000u32;
        let to_rate = 16000u32;
        let samples: Vec<f32> = (0..from_rate)
            .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / from_rate as f32).sin())
            .collect();

        let resampled = resample(&samples, from_rate, to_rate).unwrap();
        let expected_len = to_rate as usize;
        let tolerance = expected_len / 10;
        assert!(
            (resampled.len() as i64 - expected_len as i64).unsigned_abs() < tolerance as u64,
            "Expected ~{} samples, got {}",
            expected_len,
            resampled.len()
        );
    }

    #[test]
    fn test_resample_identity() {
        let samples: Vec<f32> = (0..1600)
            .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 16000.0).sin())
            .collect();

        let result = resample(&samples, 16000, 16000).unwrap();
        assert_eq!(result.len(), samples.len());
    }
}
