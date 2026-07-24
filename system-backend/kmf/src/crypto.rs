use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const LAB_ENVELOPE_ALGORITHM: &str = "lab_sha256_stream_mac_v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SealedBlob {
    pub algorithm: String,
    pub nonce_hex: String,
    pub ciphertext_hex: String,
    pub mac_hex: String,
}

pub fn random_bytes(len: usize) -> Result<Vec<u8>, String> {
    let mut file = File::open("/dev/urandom").map_err(|error| error.to_string())?;
    let mut bytes = vec![0u8; len];
    file.read_exact(&mut bytes)
        .map_err(|error| error.to_string())?;
    Ok(bytes)
}

pub fn load_or_create_secret(path: &Path, len: usize) -> Result<Vec<u8>, String> {
    if path.exists() {
        let bytes = fs::read(path).map_err(|error| error.to_string())?;
        if bytes.len() != len {
            return Err(format!(
                "secret {} has {} bytes, expected {}",
                path.display(),
                bytes.len(),
                len
            ));
        }
        return Ok(bytes);
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let bytes = random_bytes(len)?;
    write_private_file(path, &bytes)?;
    Ok(bytes)
}

pub fn write_private_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .mode(0o600)
            .open(path)
            .map_err(|error| error.to_string())?;
        file.write_all(bytes).map_err(|error| error.to_string())?;
        file.sync_all().map_err(|error| error.to_string())?;
    }
    #[cfg(not(unix))]
    {
        fs::write(path, bytes).map_err(|error| error.to_string())?;
    }
    Ok(())
}

pub fn seal(key: &[u8], plaintext: &[u8], context: &[u8]) -> Result<SealedBlob, String> {
    if key.len() < 16 {
        return Err("envelope key must contain at least 16 bytes".to_string());
    }
    let nonce = random_bytes(16)?;
    let stream = keystream(key, &nonce, context, plaintext.len());
    let ciphertext = xor(plaintext, &stream);
    let mac = calculate_mac(key, &nonce, context, &ciphertext);
    Ok(SealedBlob {
        algorithm: LAB_ENVELOPE_ALGORITHM.to_string(),
        nonce_hex: hex_encode(&nonce),
        ciphertext_hex: hex_encode(&ciphertext),
        mac_hex: hex_encode(&mac),
    })
}

pub fn open(key: &[u8], blob: &SealedBlob, context: &[u8]) -> Result<Vec<u8>, String> {
    if blob.algorithm != LAB_ENVELOPE_ALGORITHM {
        return Err(format!("unsupported envelope algorithm {}", blob.algorithm));
    }
    let nonce = hex_decode(&blob.nonce_hex)?;
    let ciphertext = hex_decode(&blob.ciphertext_hex)?;
    let supplied_mac = hex_decode(&blob.mac_hex)?;
    let expected_mac = calculate_mac(key, &nonce, context, &ciphertext);
    if !constant_time_eq(&supplied_mac, &expected_mac) {
        return Err("envelope MAC verification failed".to_string());
    }
    let stream = keystream(key, &nonce, context, ciphertext.len());
    Ok(xor(&ciphertext, &stream))
}

pub fn fingerprint(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    hex_encode(&digest[..8])
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    hex_encode(&Sha256::digest(bytes))
}

pub fn derive_node_transport_key(node_secret: &[u8], node_id: &str) -> Vec<u8> {
    let mut hash = Sha256::new();
    hash.update(b"netcore-kmf-node-transport-v1");
    hash.update(node_secret);
    hash.update(node_id.as_bytes());
    hash.finalize().to_vec()
}

fn keystream(key: &[u8], nonce: &[u8], context: &[u8], len: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(len);
    let mut counter = 0u64;
    while out.len() < len {
        let mut hash = Sha256::new();
        hash.update(b"netcore-kmf-lab-stream-v1");
        hash.update(key);
        hash.update(nonce);
        hash.update(context);
        hash.update(counter.to_be_bytes());
        out.extend_from_slice(&hash.finalize());
        counter = counter.saturating_add(1);
    }
    out.truncate(len);
    out
}

fn calculate_mac(key: &[u8], nonce: &[u8], context: &[u8], ciphertext: &[u8]) -> Vec<u8> {
    let mut inner = Sha256::new();
    inner.update(b"netcore-kmf-lab-mac-inner-v1");
    inner.update(key);
    inner.update(nonce);
    inner.update(context);
    inner.update(ciphertext);
    let inner = inner.finalize();

    let mut outer = Sha256::new();
    outer.update(b"netcore-kmf-lab-mac-outer-v1");
    outer.update(key);
    outer.update(inner);
    outer.finalize().to_vec()
}

fn xor(left: &[u8], right: &[u8]) -> Vec<u8> {
    left.iter()
        .zip(right.iter())
        .map(|(left, right)| left ^ right)
        .collect()
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut diff = 0u8;
    for (left, right) in left.iter().zip(right.iter()) {
        diff |= left ^ right;
    }
    diff == 0
}

pub fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(out, "{byte:02x}");
    }
    out
}

pub fn hex_decode(value: &str) -> Result<Vec<u8>, String> {
    if value.len() % 2 != 0 {
        return Err("hex value has odd length".to_string());
    }
    let mut out = Vec::with_capacity(value.len() / 2);
    for index in (0..value.len()).step_by(2) {
        let byte = u8::from_str_radix(&value[index..index + 2], 16)
            .map_err(|error| format!("invalid hex: {error}"))?;
        out.push(byte);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seal_roundtrip_and_tamper_detection() {
        let key = vec![0x11; 32];
        let blob = seal(&key, b"test secret", b"context").expect("seal");
        assert_eq!(open(&key, &blob, b"context").expect("open"), b"test secret");
        let mut tampered = blob.clone();
        tampered.ciphertext_hex.replace_range(0..2, "ff");
        assert!(open(&key, &tampered, b"context").is_err());
    }
}
