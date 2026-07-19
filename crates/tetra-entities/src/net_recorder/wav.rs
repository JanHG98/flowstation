use std::fs::{File, OpenOptions};
use std::io::{self, BufWriter, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use crate::net_audio::TETRA_PCM_SAMPLE_RATE;

const WAV_HEADER_LEN: u64 = 44;

/// Streaming 8-kHz mono PCM WAV writer. The RIFF/data lengths are patched when finalized.
pub struct PcmWavWriter {
    writer: BufWriter<File>,
    part_path: PathBuf,
    samples_written: u64,
}

impl PcmWavWriter {
    pub fn create(part_path: PathBuf) -> io::Result<Self> {
        if let Some(parent) = part_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = OpenOptions::new().create_new(true).read(true).write(true).open(&part_path)?;
        let mut writer = BufWriter::new(file);
        write_header(&mut writer, 0)?;
        Ok(Self {
            writer,
            part_path,
            samples_written: 0,
        })
    }

    pub fn write_samples(&mut self, samples: &[i16]) -> io::Result<()> {
        let new_total = self.samples_written.saturating_add(samples.len() as u64);
        let data_bytes = new_total.saturating_mul(2);
        if data_bytes > u32::MAX as u64 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "WAV data exceeds RIFF 32-bit size limit"));
        }
        for sample in samples {
            self.writer.write_all(&sample.to_le_bytes())?;
        }
        self.samples_written = new_total;
        Ok(())
    }

    pub fn samples_written(&self) -> u64 {
        self.samples_written
    }

    pub fn part_path(&self) -> &Path {
        &self.part_path
    }

    pub fn finalize(mut self, final_path: &Path) -> io::Result<u64> {
        self.writer.flush()?;
        let data_bytes = self.samples_written.saturating_mul(2);
        patch_header(self.writer.get_mut(), data_bytes)?;
        self.writer.flush()?;
        self.writer.get_ref().sync_all()?;
        drop(self.writer);
        std::fs::rename(&self.part_path, final_path)?;
        Ok(data_bytes)
    }
}

fn write_header<W: Write>(writer: &mut W, data_bytes: u64) -> io::Result<()> {
    let data_len = u32::try_from(data_bytes).map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "WAV too large"))?;
    let riff_len = 36u32
        .checked_add(data_len)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "WAV too large"))?;
    writer.write_all(b"RIFF")?;
    writer.write_all(&riff_len.to_le_bytes())?;
    writer.write_all(b"WAVE")?;
    writer.write_all(b"fmt ")?;
    writer.write_all(&16u32.to_le_bytes())?; // PCM fmt chunk length
    writer.write_all(&1u16.to_le_bytes())?; // PCM
    writer.write_all(&1u16.to_le_bytes())?; // mono
    writer.write_all(&TETRA_PCM_SAMPLE_RATE.to_le_bytes())?;
    writer.write_all(&(TETRA_PCM_SAMPLE_RATE * 2).to_le_bytes())?; // byte rate
    writer.write_all(&2u16.to_le_bytes())?; // block align
    writer.write_all(&16u16.to_le_bytes())?; // bits/sample
    writer.write_all(b"data")?;
    writer.write_all(&data_len.to_le_bytes())?;
    Ok(())
}

fn patch_header(file: &mut File, data_bytes: u64) -> io::Result<()> {
    let data_len = u32::try_from(data_bytes).map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "WAV too large"))?;
    let riff_len = 36u32
        .checked_add(data_len)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "WAV too large"))?;
    file.seek(SeekFrom::Start(4))?;
    file.write_all(&riff_len.to_le_bytes())?;
    file.seek(SeekFrom::Start(40))?;
    file.write_all(&data_len.to_le_bytes())?;
    file.seek(SeekFrom::End(0))?;
    Ok(())
}

/// Repair a `.wav.part` left by an unclean shutdown and rename it to the supplied final path.
pub fn recover_part(part_path: &Path, final_path: &Path) -> io::Result<u64> {
    let mut file = OpenOptions::new().read(true).write(true).open(part_path)?;
    let len = file.metadata()?.len();
    if len < WAV_HEADER_LEN {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "partial WAV is shorter than its header"));
    }
    let mut data_bytes = len - WAV_HEADER_LEN;
    if data_bytes % 2 != 0 {
        file.set_len(len - 1)?;
        data_bytes -= 1;
    }
    patch_header(&mut file, data_bytes)?;
    file.sync_all()?;
    drop(file);
    std::fs::rename(part_path, final_path)?;
    Ok(data_bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_valid_pcm_header() {
        let dir = std::env::temp_dir().join(format!("netcore-wav-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let part = dir.join("test.wav.part");
        let final_path = dir.join("test.wav");
        let mut writer = PcmWavWriter::create(part).unwrap();
        writer.write_samples(&[1, -1, 42]).unwrap();
        writer.finalize(&final_path).unwrap();
        let bytes = std::fs::read(final_path).unwrap();
        assert_eq!(&bytes[..4], b"RIFF");
        assert_eq!(&bytes[8..12], b"WAVE");
        assert_eq!(u32::from_le_bytes(bytes[40..44].try_into().unwrap()), 6);
        let _ = std::fs::remove_dir_all(dir);
    }
}
