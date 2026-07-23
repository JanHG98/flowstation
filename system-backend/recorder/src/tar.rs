use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn create_tar(output: &Path, files: &[(PathBuf, String)]) -> Result<(), String> {
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
    }
    let part = output.with_extension(format!("tar.{}.part", uuid::Uuid::new_v4()));
    let mut writer = File::create(&part)
        .map_err(|error| format!("cannot create {}: {error}", part.display()))?;
    for (source, archive_name) in files {
        append_file(&mut writer, source, archive_name)?;
    }
    writer
        .write_all(&[0u8; 1024])
        .map_err(|error| format!("cannot finish TAR: {error}"))?;
    writer
        .sync_all()
        .map_err(|error| format!("cannot sync {}: {error}", part.display()))?;
    fs::rename(&part, output).map_err(|error| {
        format!(
            "cannot publish {} -> {}: {error}",
            part.display(),
            output.display()
        )
    })
}

fn append_file(writer: &mut File, source: &Path, archive_name: &str) -> Result<(), String> {
    if archive_name.as_bytes().len() > 100 {
        return Err(format!("TAR path is too long: {archive_name}"));
    }
    let metadata = fs::metadata(source)
        .map_err(|error| format!("cannot stat {}: {error}", source.display()))?;
    if !metadata.is_file() {
        return Err(format!("{} is not a regular file", source.display()));
    }

    let mut header = [0u8; 512];
    header[..archive_name.len()].copy_from_slice(archive_name.as_bytes());
    write_octal(&mut header[100..108], 0o644)?;
    write_octal(&mut header[108..116], 0)?;
    write_octal(&mut header[116..124], 0)?;
    write_octal(&mut header[124..136], metadata.len())?;
    let mtime = metadata
        .modified()
        .unwrap_or_else(|_| SystemTime::now())
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    write_octal(&mut header[136..148], mtime)?;
    header[148..156].fill(b' ');
    header[156] = b'0';
    header[257..263].copy_from_slice(b"ustar\0");
    header[263..265].copy_from_slice(b"00");
    let checksum = header.iter().map(|byte| *byte as u64).sum::<u64>();
    write_checksum(&mut header[148..156], checksum)?;
    writer
        .write_all(&header)
        .map_err(|error| format!("cannot write TAR header: {error}"))?;

    let mut input = File::open(source)
        .map_err(|error| format!("cannot open {}: {error}", source.display()))?;
    let mut buffer = [0u8; 64 * 1024];
    let mut written = 0u64;
    loop {
        let read = input
            .read(&mut buffer)
            .map_err(|error| format!("cannot read {}: {error}", source.display()))?;
        if read == 0 {
            break;
        }
        writer
            .write_all(&buffer[..read])
            .map_err(|error| format!("cannot append {}: {error}", source.display()))?;
        written = written.saturating_add(read as u64);
    }
    let padding = (512 - (written % 512)) % 512;
    if padding > 0 {
        writer
            .write_all(&vec![0u8; padding as usize])
            .map_err(|error| format!("cannot pad TAR entry: {error}"))?;
    }
    Ok(())
}

fn write_octal(field: &mut [u8], value: u64) -> Result<(), String> {
    if field.len() < 2 {
        return Err("TAR octal field is too short".to_string());
    }
    field.fill(b'0');
    let text = format!("{value:o}");
    if text.len() + 1 > field.len() {
        return Err(format!("value {value} does not fit TAR field"));
    }
    let start = field.len() - 1 - text.len();
    field[start..start + text.len()].copy_from_slice(text.as_bytes());
    field[field.len() - 1] = 0;
    Ok(())
}

fn write_checksum(field: &mut [u8], value: u64) -> Result<(), String> {
    if field.len() != 8 {
        return Err("TAR checksum field must be 8 bytes".to_string());
    }
    let text = format!("{value:06o}\0 ");
    if text.len() != 8 {
        return Err("invalid TAR checksum".to_string());
    }
    field.copy_from_slice(text.as_bytes());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn creates_uncompressed_tar_archive() {
        let root = std::env::temp_dir().join(format!("netcore-tar-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).expect("root");
        let source = root.join("source.txt");
        fs::write(&source, b"hello").expect("source");
        let output = root.join("export.tar");
        create_tar(&output, &[(source, "recording/source.txt".to_string())]).expect("tar");
        let bytes = fs::read(&output).expect("output");
        assert!(bytes.len() >= 2048);
        assert_eq!(&bytes[..20], b"recording/source.txt");
        let _ = fs::remove_dir_all(root);
    }
}
