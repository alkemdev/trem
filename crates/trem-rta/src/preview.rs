//! Play pre-rendered **f32** PCM once on the default output device (no graph).

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use anyhow::Context;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, StreamConfig};

fn resample_stereo_linear(l: &[f32], r: &[f32], src_sr: f64, dst_sr: f64) -> Vec<f32> {
    if l.is_empty() {
        return Vec::new();
    }
    let ratio = dst_sr / src_sr;
    let out_frames = ((l.len() as f64) * ratio).round().max(1.0) as usize;
    let last = l.len() - 1;
    let mut out = Vec::with_capacity(out_frames * 2);
    for i in 0..out_frames {
        let t = i as f64 / ratio;
        let idx = t.floor() as usize;
        let idx = idx.min(last);
        let idx2 = (idx + 1).min(last);
        let frac = (t - idx as f64) as f32;
        let sl = l[idx] * (1.0 - frac) + l[idx2] * frac;
        let sr = r[idx] * (1.0 - frac) + r[idx2] * frac;
        out.push(sl);
        out.push(sr);
    }
    out
}

fn map_stereo_to_device_channels(stereo_lr: &[f32], dst_ch: usize) -> Vec<f32> {
    let frames = stereo_lr.len() / 2;
    let mut out = Vec::with_capacity(frames * dst_ch);
    for i in 0..frames {
        let l = stereo_lr[i * 2];
        let r = stereo_lr[i * 2 + 1];
        match dst_ch {
            0 => {}
            1 => out.push((l + r) * 0.5),
            _ => {
                out.push(l);
                out.push(r);
                for _ in 2..dst_ch {
                    out.push(0.0);
                }
            }
        }
    }
    out
}

fn play_interleaved_blocking(interleaved: Vec<f32>, config: &StreamConfig) -> anyhow::Result<()> {
    let channels = config.channels as usize;
    if channels == 0 {
        return Ok(());
    }
    let sample_rate = config.sample_rate as f64;
    let total = interleaved.len();
    let cursor = Arc::new(AtomicUsize::new(0));
    let data = Arc::new(interleaved);

    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .context("no default output audio device")?;

    let cursor_cb = Arc::clone(&cursor);
    let data_cb = Arc::clone(&data);
    let stream = device
        .build_output_stream(
            config,
            move |out: &mut [f32], _: &cpal::OutputCallbackInfo| {
                let mut i = cursor_cb.load(Ordering::Relaxed);
                for sample in out.iter_mut() {
                    *sample = *data_cb.get(i).unwrap_or(&0.0);
                    i += 1;
                }
                cursor_cb.store(i, Ordering::Relaxed);
            },
            |err| eprintln!("cpal stream error: {err}"),
            None,
        )
        .context("build_output_stream")?;

    stream.play().context("play")?;
    let frames = total / channels;
    let secs = frames as f64 / sample_rate;
    thread::sleep(Duration::from_secs_f64(secs + 0.08));
    drop(stream);
    Ok(())
}

/// Plays interleaved stereo `f32` once, resampling to the device rate if needed.
///
/// Requires the default output stream format to be [`SampleFormat::F32`].
pub fn play_stereo_f32(left: &[f32], right: &[f32], source_sample_rate: f64) -> anyhow::Result<()> {
    if left.len() != right.len() {
        anyhow::bail!("left/right length mismatch");
    }
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .context("no default output audio device")?;
    let supported = device
        .default_output_config()
        .context("default_output_config")?;
    if supported.sample_format() != SampleFormat::F32 {
        anyhow::bail!(
            "default output is {:?}, need {:?} (use WAV export or another device)",
            supported.sample_format(),
            SampleFormat::F32
        );
    }
    let stream_cfg: StreamConfig = supported.config();
    let device_sr = stream_cfg.sample_rate as f64;
    let device_ch = stream_cfg.channels as usize;

    let mut interleaved = resample_stereo_linear(left, right, source_sample_rate, device_sr);
    if device_ch != 2 {
        interleaved = map_stereo_to_device_channels(&interleaved, device_ch);
    }
    play_interleaved_blocking(interleaved, &stream_cfg)
}
