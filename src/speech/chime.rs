use std::io::Write;
use std::process::{Command, Stdio};

pub enum ChimeType {
    Wake,
    Success,
    Error,
}

pub fn play_chime(chime: ChimeType) {
    let sample_rate = 22050;
    let mut pcm: Vec<i16> = Vec::new();

    match chime {
        ChimeType::Wake => {
            // Two-tone rising chime (880Hz -> 1320Hz)
            append_tone(&mut pcm, 880.0, 0.08, sample_rate);
            append_tone(&mut pcm, 1320.0, 0.12, sample_rate);
        }
        ChimeType::Success => {
            // Success chime (587Hz -> 880Hz)
            append_tone(&mut pcm, 587.0, 0.07, sample_rate);
            append_tone(&mut pcm, 880.0, 0.10, sample_rate);
        }
        ChimeType::Error => {
            // Low tone error chime (440Hz -> 330Hz)
            append_tone(&mut pcm, 440.0, 0.10, sample_rate);
            append_tone(&mut pcm, 330.0, 0.12, sample_rate);
        }
    }

    // Stream raw PCM bytes directly to pw-cat (-p --raw)
    let child = Command::new("pw-cat")
        .args([
            "-p",
            "--raw",
            "--rate",
            &sample_rate.to_string(),
            "--channels",
            "1",
            "--format",
            "s16",
            "-",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();

    if let Ok(mut c) = child {
        if let Some(mut stdin) = c.stdin.take() {
            let mut byte_buf = Vec::with_capacity(pcm.len() * 2);
            for sample in pcm {
                byte_buf.extend_from_slice(&sample.to_le_bytes());
            }
            let _ = stdin.write_all(&byte_buf);
        }
        let _ = c.wait();
    }
}

fn append_tone(pcm: &mut Vec<i16>, freq: f64, duration_secs: f64, sample_rate: u32) {
    let total_samples = (duration_secs * sample_rate as f64) as usize;
    let fade_samples = (total_samples as f64 * 0.15) as usize;

    for i in 0..total_samples {
        let t = i as f64 / sample_rate as f64;
        let mut envelope = 1.0;
        if i < fade_samples {
            envelope = i as f64 / fade_samples as f64;
        } else if i > total_samples - fade_samples {
            envelope = (total_samples - i) as f64 / fade_samples as f64;
        }

        let sample_f = (2.0 * std::f64::consts::PI * freq * t).sin() * envelope * 0.35;
        let sample_i16 = (sample_f * 32767.0) as i16;
        pcm.push(sample_i16);
    }
}
