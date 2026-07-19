use std::fs;
use std::path::Path;
use std::process::Command;

use tetra_config::bluestation::CfgAudioPlayer;

use crate::net_audio::{TETRA_PCM_SAMPLE_RATE, TETRA_PCM_SAMPLES_PER_BLOCK, TetraSpeechEncoder};

use super::types::PreparedAudio;

pub(crate) fn prepare_audio(
    config: &CfgAudioPlayer,
    job_id: String,
    source: super::types::ResolvedAudioSource,
    target_type: super::types::AudioTargetType,
    target_id: u32,
    priority: u8,
) -> Result<PreparedAudio, String> {
    let metadata = fs::metadata(&source.path).map_err(|e| format!("cannot stat {}: {e}", source.path.display()))?;
    if !metadata.is_file() {
        return Err("selected path is not a regular file".to_string());
    }
    let max_bytes = config.max_file_size_mb.saturating_mul(1024 * 1024);
    if metadata.len() > max_bytes {
        return Err(format!(
            "file is too large ({} bytes; limit {} MiB)",
            metadata.len(), config.max_file_size_mb
        ));
    }

    let extension = source
        .path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let mut pcm = match extension.as_str() {
        "wav" => decode_wav_native(&source.path).or_else(|native_err| {
            tracing::debug!(
                "AudioPlayer: native WAV decode failed for {}: {}; trying ffmpeg",
                source.path.display(),
                native_err
            );
            decode_with_ffmpeg(&config.ffmpeg_path, &source.path, config.max_duration_seconds)
        })?,
        "mp3" => decode_with_ffmpeg(&config.ffmpeg_path, &source.path, config.max_duration_seconds)?,
        _ => return Err("only .wav and .mp3 files are supported".to_string()),
    };

    let max_samples = (config.max_duration_seconds as usize).saturating_mul(TETRA_PCM_SAMPLE_RATE as usize);
    if pcm.len() > max_samples {
        return Err(format!(
            "decoded audio is longer than the configured {} second limit",
            config.max_duration_seconds
        ));
    }
    if pcm.is_empty() {
        return Err("decoded audio contains no samples".to_string());
    }

    let duration_ms = pcm.len() as u64 * 1000 / TETRA_PCM_SAMPLE_RATE as u64;
    let remainder = pcm.len() % TETRA_PCM_SAMPLES_PER_BLOCK;
    if remainder != 0 {
        pcm.resize(pcm.len() + (TETRA_PCM_SAMPLES_PER_BLOCK - remainder), 0);
    }

    let mut encoder = TetraSpeechEncoder::new().ok_or_else(|| "tetra encoder creation failed".to_string())?;
    let mut blocks = Vec::with_capacity(pcm.len() / TETRA_PCM_SAMPLES_PER_BLOCK + config.tail_silence_blocks as usize);
    for block in pcm.chunks_exact(TETRA_PCM_SAMPLES_PER_BLOCK) {
        let encoded = encoder
            .encode_complete_block(block)
            .ok_or_else(|| "failed to encode a complete TETRA speech block".to_string())?;
        blocks.push(encoded);
    }
    let silence = [0i16; TETRA_PCM_SAMPLES_PER_BLOCK];
    for _ in 0..config.tail_silence_blocks {
        let encoded = encoder
            .encode_complete_block(&silence)
            .ok_or_else(|| "failed to encode TETRA tail silence".to_string())?;
        blocks.push(encoded);
    }

    Ok(PreparedAudio {
        job_id,
        target_type,
        target_id,
        priority,
        duration_ms,
        blocks,
    })
}

fn decode_with_ffmpeg(ffmpeg_path: &str, path: &Path, max_duration_seconds: u32) -> Result<Vec<i16>, String> {
    // Bound decoder output as well as source-file size. A highly compressed or malformed
    // input must not be able to make the preparation worker allocate unbounded PCM.
    let decode_limit = max_duration_seconds.saturating_add(1).to_string();
    let output = Command::new(ffmpeg_path)
        .args(["-nostdin", "-hide_banner", "-loglevel", "error", "-i"])
        .arg(path)
        .args(["-vn", "-t"])
        .arg(&decode_limit)
        .args(["-ac", "1", "-ar", "8000", "-f", "s16le", "pipe:1"])
        .output()
        .map_err(|e| format!("failed to start ffmpeg '{}': {e}", ffmpeg_path))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            format!("ffmpeg exited with {}", output.status)
        } else {
            format!("ffmpeg failed: {stderr}")
        });
    }
    if output.stdout.len() % 2 != 0 {
        return Err("ffmpeg returned an odd number of PCM bytes".to_string());
    }
    Ok(output
        .stdout
        .chunks_exact(2)
        .map(|bytes| i16::from_le_bytes([bytes[0], bytes[1]]))
        .collect())
}

fn decode_wav_native(path: &Path) -> Result<Vec<i16>, String> {
    let data = fs::read(path).map_err(|e| e.to_string())?;
    if data.len() < 12 || &data[0..4] != b"RIFF" || &data[8..12] != b"WAVE" {
        return Err("not a RIFF/WAVE file".to_string());
    }

    let mut offset = 12usize;
    let mut format: Option<WavFormat> = None;
    let mut pcm_data: Option<&[u8]> = None;
    while offset + 8 <= data.len() {
        let id = &data[offset..offset + 4];
        let size = read_u32_le(&data[offset + 4..offset + 8])? as usize;
        let start = offset + 8;
        let end = start.checked_add(size).ok_or_else(|| "WAV chunk size overflow".to_string())?;
        if end > data.len() {
            return Err("truncated WAV chunk".to_string());
        }
        match id {
            b"fmt " => format = Some(parse_wav_format(&data[start..end])?),
            b"data" => pcm_data = Some(&data[start..end]),
            _ => {}
        }
        offset = end + (size & 1);
    }

    let format = format.ok_or_else(|| "WAV fmt chunk missing".to_string())?;
    let bytes = pcm_data.ok_or_else(|| "WAV data chunk missing".to_string())?;
    let decoded = decode_wav_samples(bytes, format)?;
    Ok(if format.sample_rate == TETRA_PCM_SAMPLE_RATE {
        decoded
    } else {
        resample_linear(&decoded, format.sample_rate, TETRA_PCM_SAMPLE_RATE)
    })
}

#[derive(Clone, Copy)]
struct WavFormat {
    audio_format: u16,
    channels: u16,
    sample_rate: u32,
    bits_per_sample: u16,
    block_align: u16,
}

fn parse_wav_format(data: &[u8]) -> Result<WavFormat, String> {
    if data.len() < 16 {
        return Err("WAV fmt chunk too short".to_string());
    }
    let format = WavFormat {
        audio_format: read_u16_le(&data[0..2])?,
        channels: read_u16_le(&data[2..4])?,
        sample_rate: read_u32_le(&data[4..8])?,
        block_align: read_u16_le(&data[12..14])?,
        bits_per_sample: read_u16_le(&data[14..16])?,
    };
    if format.channels == 0 || format.channels > 32 {
        return Err("invalid WAV channel count".to_string());
    }
    if format.sample_rate == 0 {
        return Err("invalid WAV sample rate".to_string());
    }
    if format.block_align == 0 {
        return Err("invalid WAV block alignment".to_string());
    }
    if !matches!(format.audio_format, 1 | 3) {
        return Err(format!("unsupported WAV encoding {}", format.audio_format));
    }
    Ok(format)
}

fn decode_wav_samples(data: &[u8], format: WavFormat) -> Result<Vec<i16>, String> {
    let frame_bytes = format.block_align as usize;
    let bytes_per_sample = (format.bits_per_sample as usize + 7) / 8;
    if bytes_per_sample == 0 || bytes_per_sample * format.channels as usize > frame_bytes {
        return Err("invalid WAV sample layout".to_string());
    }

    let mut out = Vec::with_capacity(data.len() / frame_bytes);
    for frame in data.chunks_exact(frame_bytes) {
        let mut sum = 0i64;
        for channel in 0..format.channels as usize {
            let start = channel * bytes_per_sample;
            let sample = decode_one_sample(&frame[start..start + bytes_per_sample], format.audio_format, format.bits_per_sample)?;
            sum += sample as i64;
        }
        let mono = sum / format.channels as i64;
        out.push(mono.clamp(i16::MIN as i64, i16::MAX as i64) as i16);
    }
    Ok(out)
}

fn decode_one_sample(bytes: &[u8], audio_format: u16, bits: u16) -> Result<i16, String> {
    if audio_format == 3 {
        if bits != 32 || bytes.len() != 4 {
            return Err("only 32-bit IEEE-float WAV is supported natively".to_string());
        }
        let value = f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        let clamped = value.clamp(-1.0, 1.0);
        return Ok((clamped * i16::MAX as f32) as i16);
    }

    match bits {
        8 => Ok(((bytes[0] as i16) - 128) << 8),
        16 if bytes.len() >= 2 => Ok(i16::from_le_bytes([bytes[0], bytes[1]])),
        24 if bytes.len() >= 3 => {
            let raw = ((bytes[2] as i32) << 24 | (bytes[1] as i32) << 16 | (bytes[0] as i32) << 8) >> 8;
            Ok((raw >> 8).clamp(i16::MIN as i32, i16::MAX as i32) as i16)
        }
        32 if bytes.len() >= 4 => {
            let raw = i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
            Ok((raw >> 16).clamp(i16::MIN as i32, i16::MAX as i32) as i16)
        }
        _ => Err(format!("unsupported PCM WAV bit depth {bits}")),
    }
}

fn resample_linear(input: &[i16], source_rate: u32, target_rate: u32) -> Vec<i16> {
    if input.is_empty() || source_rate == target_rate {
        return input.to_vec();
    }
    let output_len = ((input.len() as u128 * target_rate as u128) / source_rate as u128) as usize;
    let mut output = Vec::with_capacity(output_len);
    for index in 0..output_len {
        let numerator = index as u128 * source_rate as u128;
        let base = (numerator / target_rate as u128) as usize;
        let frac = (numerator % target_rate as u128) as f64 / target_rate as f64;
        let a = input[base.min(input.len() - 1)] as f64;
        let b = input[(base + 1).min(input.len() - 1)] as f64;
        output.push((a + (b - a) * frac).round().clamp(i16::MIN as f64, i16::MAX as f64) as i16);
    }
    output
}

fn read_u16_le(bytes: &[u8]) -> Result<u16, String> {
    let bytes: [u8; 2] = bytes.try_into().map_err(|_| "short u16".to_string())?;
    Ok(u16::from_le_bytes(bytes))
}

fn read_u32_le(bytes: &[u8]) -> Result<u32, String> {
    let bytes: [u8; 4] = bytes.try_into().map_err(|_| "short u32".to_string())?;
    Ok(u32::from_le_bytes(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_resampler_keeps_8khz_input() {
        let input = vec![1i16, 2, 3];
        assert_eq!(resample_linear(&input, 8_000, 8_000), input);
    }

    #[test]
    fn decodes_pcm16_sample() {
        assert_eq!(decode_one_sample(&1234i16.to_le_bytes(), 1, 16).unwrap(), 1234);
    }
}
