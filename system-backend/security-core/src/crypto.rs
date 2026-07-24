use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

use sha2::{Digest, Sha256};

pub fn load_or_create_seed(path: &Path) -> Result<Vec<u8>, String> {
    if path.exists() {
        let mut seed = Vec::new();
        File::open(path)
            .and_then(|mut file| file.read_to_end(&mut seed))
            .map_err(|error| format!("read lab seed {}: {error}", path.display()))?;
        if seed.len() < 32 {
            return Err(format!(
                "lab seed {} is too short; expected at least 32 bytes",
                path.display()
            ));
        }
        return Ok(seed);
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("create lab seed directory {}: {error}", parent.display()))?;
    }
    let seed = random_bytes(32)?;
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(path)
        .map_err(|error| format!("create lab seed {}: {error}", path.display()))?;
    file.write_all(&seed)
        .map_err(|error| format!("write lab seed {}: {error}", path.display()))?;
    file.sync_all()
        .map_err(|error| format!("sync lab seed {}: {error}", path.display()))?;
    Ok(seed)
}

pub fn random_bytes(length: usize) -> Result<Vec<u8>, String> {
    let mut bytes = vec![0_u8; length];
    File::open("/dev/urandom")
        .and_then(|mut file| file.read_exact(&mut bytes))
        .map_err(|error| format!("read /dev/urandom: {error}"))?;
    Ok(bytes)
}

pub fn derive_subscriber_key(seed: &[u8], issi: u32) -> Result<Vec<u8>, String> {
    Ok(hmac_sha256(
        seed,
        &[b"netcore-security-core/lab-subscriber/v1", &issi.to_be_bytes()],
    ))
}

pub fn expected_response(
    subscriber_key: &[u8],
    issi: u32,
    node_id: &str,
    context_id: &str,
    challenge: &[u8],
    output_bytes: usize,
) -> Result<Vec<u8>, String> {
    if output_bytes > 32 {
        return Err("response length may not exceed 32 bytes".to_string());
    }
    let digest = hmac_sha256(
        subscriber_key,
        &[
            b"netcore-security-core/lab-response/v1",
            &issi.to_be_bytes(),
            &(node_id.len() as u32).to_be_bytes(),
            node_id.as_bytes(),
            &(context_id.len() as u32).to_be_bytes(),
            context_id.as_bytes(),
            &(challenge.len() as u32).to_be_bytes(),
            challenge,
        ],
    );
    Ok(digest[..output_bytes].to_vec())
}

pub fn derive_dck(
    subscriber_key: &[u8],
    issi: u32,
    node_id: &str,
    context_id: &str,
    challenge: &[u8],
    response: &[u8],
    output_bytes: usize,
) -> Result<Vec<u8>, String> {
    if output_bytes > 32 {
        return Err("DCK length may not exceed 32 bytes".to_string());
    }
    let digest = hmac_sha256(
        subscriber_key,
        &[
            b"netcore-security-core/lab-dck/v1",
            &issi.to_be_bytes(),
            &(node_id.len() as u32).to_be_bytes(),
            node_id.as_bytes(),
            &(context_id.len() as u32).to_be_bytes(),
            context_id.as_bytes(),
            challenge,
            response,
        ],
    );
    Ok(digest[..output_bytes].to_vec())
}


fn hmac_sha256(key: &[u8], parts: &[&[u8]]) -> Vec<u8> {
    const BLOCK_BYTES: usize = 64;
    let mut normalised_key = [0_u8; BLOCK_BYTES];
    if key.len() > BLOCK_BYTES {
        let digest = Sha256::digest(key);
        normalised_key[..digest.len()].copy_from_slice(&digest);
    } else {
        normalised_key[..key.len()].copy_from_slice(key);
    }

    let mut inner_pad = [0x36_u8; BLOCK_BYTES];
    let mut outer_pad = [0x5c_u8; BLOCK_BYTES];
    for index in 0..BLOCK_BYTES {
        inner_pad[index] ^= normalised_key[index];
        outer_pad[index] ^= normalised_key[index];
    }

    let mut inner = Sha256::new();
    inner.update(inner_pad);
    for part in parts {
        inner.update(part);
    }
    let inner_digest = inner.finalize();

    let mut outer = Sha256::new();
    outer.update(outer_pad);
    outer.update(inner_digest);
    outer.finalize().to_vec()
}

pub fn fingerprint(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    format!("sha256:{}", encode_hex(&digest[..8]))
}

pub fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

pub fn decode_hex(value: &str) -> Result<Vec<u8>, String> {
    let trimmed = value.trim();
    if trimmed.len() % 2 != 0 {
        return Err("hex value must contain an even number of characters".to_string());
    }
    let mut out = Vec::with_capacity(trimmed.len() / 2);
    let bytes = trimmed.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        let high = decode_nibble(bytes[index])?;
        let low = decode_nibble(bytes[index + 1])?;
        out.push((high << 4) | low);
        index += 2;
    }
    Ok(out)
}

fn decode_nibble(value: u8) -> Result<u8, String> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        b'A'..=b'F' => Ok(value - b'A' + 10),
        _ => Err(format!("invalid hexadecimal character {:?}", value as char)),
    }
}

pub fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    let mut difference = left.len() ^ right.len();
    let maximum = left.len().max(right.len());
    for index in 0..maximum {
        let a = left.get(index).copied().unwrap_or(0);
        let b = right.get(index).copied().unwrap_or(0);
        difference |= usize::from(a ^ b);
    }
    difference == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_roundtrip() {
        let bytes = vec![0x00, 0x01, 0xab, 0xff];
        assert_eq!(decode_hex(&encode_hex(&bytes)).unwrap(), bytes);
    }

    #[test]
    fn response_is_deterministic() {
        let key = derive_subscriber_key(&[7_u8; 32], 4_010_001).unwrap();
        let first = expected_response(&key, 4_010_001, "tbs-1", "ctx-1", &[1, 2, 3], 16).unwrap();
        let second = expected_response(&key, 4_010_001, "tbs-1", "ctx-1", &[1, 2, 3], 16).unwrap();
        assert!(constant_time_eq(&first, &second));
    }
}
