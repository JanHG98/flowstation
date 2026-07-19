use std::ptr::NonNull;

pub const TETRA_PCM_SAMPLE_RATE: u32 = 8_000;
pub const TETRA_PCM_SAMPLES_PER_FRAME: usize = 240;
pub const TETRA_PCM_SAMPLES_PER_BLOCK: usize = TETRA_PCM_SAMPLES_PER_FRAME * 2;
pub const TETRA_CODED_BITS_PER_FRAME: usize = 137;
const TETRA_CODED_BYTES_PER_FRAME: usize = (TETRA_CODED_BITS_PER_FRAME + 7) / 8;
const TETRA_TMD_BITS_PER_BLOCK: usize = TETRA_CODED_BITS_PER_FRAME * 2;
const TETRA_TMD_PACKED_BYTES: usize = (TETRA_TMD_BITS_PER_BLOCK + 7) / 8;

#[repr(C)]
struct RawTetraCodec {
    _private: [u8; 0],
}

#[link(name = "tetra-codec")]
unsafe extern "C" {
    fn tetra_encoder_create() -> *mut RawTetraCodec;
    fn tetra_decoder_create() -> *mut RawTetraCodec;
    fn tetra_codec_destroy(st: *mut RawTetraCodec);
    fn tetra_encode(st: *mut RawTetraCodec, pcm: *const i16, coded: *mut u8);
    fn tetra_decode(st: *mut RawTetraCodec, coded: *const u8, pcm: *mut i16, bfi: i32);
}

struct CodecHandle {
    ptr: NonNull<RawTetraCodec>,
}

// One codec state belongs to one media stream and is only used through &mut self.
unsafe impl Send for CodecHandle {}

impl CodecHandle {
    fn from_raw(ptr: *mut RawTetraCodec) -> Option<Self> {
        NonNull::new(ptr).map(|ptr| Self { ptr })
    }
}

impl Drop for CodecHandle {
    fn drop(&mut self) {
        unsafe { tetra_codec_destroy(self.ptr.as_ptr()) }
    }
}

/// Stateful decoder for one TETRA speech stream.
pub struct TetraSpeechDecoder {
    decoder: CodecHandle,
}

impl TetraSpeechDecoder {
    pub fn new() -> Option<Self> {
        Some(Self {
            decoder: CodecHandle::from_raw(unsafe { tetra_decoder_create() })?,
        })
    }

    /// Decode one 60 ms TMD speech block into 480 signed 16-bit PCM samples at 8 kHz.
    /// Both packed 35-byte blocks and the 274-byte one-bit-per-byte uplink representation
    /// produced by LMAC are accepted.
    pub fn decode_tmd_to_pcm(&mut self, acelp: &[u8]) -> Option<Vec<i16>> {
        let coded = split_tmd_block_to_codec_frames(acelp)?;
        let mut out = Vec::with_capacity(TETRA_PCM_SAMPLES_PER_BLOCK);
        for frame in &coded {
            let mut pcm = [0i16; TETRA_PCM_SAMPLES_PER_FRAME];
            unsafe {
                tetra_decode(self.decoder.ptr.as_ptr(), frame.as_ptr(), pcm.as_mut_ptr(), 0);
            }
            out.extend_from_slice(&pcm);
        }
        Some(out)
    }
}

/// Stateful encoder for one TETRA speech stream.
pub struct TetraSpeechEncoder {
    encoder: CodecHandle,
    pcm_buffer: Vec<i16>,
}

impl TetraSpeechEncoder {
    pub fn new() -> Option<Self> {
        Some(Self {
            encoder: CodecHandle::from_raw(unsafe { tetra_encoder_create() })?,
            pcm_buffer: Vec::with_capacity(TETRA_PCM_SAMPLES_PER_BLOCK * 2),
        })
    }

    /// Queue 8-kHz mono PCM and return every complete 60-ms TMD block now available.
    pub fn push_pcm(&mut self, samples: &[i16]) -> Vec<Vec<u8>> {
        self.pcm_buffer.extend_from_slice(samples);
        let mut out = Vec::new();
        while self.pcm_buffer.len() >= TETRA_PCM_SAMPLES_PER_BLOCK {
            let block: Vec<i16> = self.pcm_buffer.drain(..TETRA_PCM_SAMPLES_PER_BLOCK).collect();
            if let Some(encoded) = self.encode_complete_block(&block) {
                out.push(encoded);
            }
        }
        out
    }

    /// Encode exactly 480 8-kHz mono PCM samples into one packed 274-bit TMD block.
    pub fn encode_complete_block(&mut self, pcm: &[i16]) -> Option<Vec<u8>> {
        if pcm.len() != TETRA_PCM_SAMPLES_PER_BLOCK {
            return None;
        }
        let mut coded_a = [0u8; TETRA_CODED_BYTES_PER_FRAME];
        let mut coded_b = [0u8; TETRA_CODED_BYTES_PER_FRAME];
        unsafe {
            tetra_encode(self.encoder.ptr.as_ptr(), pcm[..TETRA_PCM_SAMPLES_PER_FRAME].as_ptr(), coded_a.as_mut_ptr());
            tetra_encode(self.encoder.ptr.as_ptr(), pcm[TETRA_PCM_SAMPLES_PER_FRAME..].as_ptr(), coded_b.as_mut_ptr());
        }
        Some(join_codec_frames_to_tmd_block(&coded_a, &coded_b))
    }

    pub fn buffered_samples(&self) -> usize {
        self.pcm_buffer.len()
    }

    pub fn clear(&mut self) {
        self.pcm_buffer.clear();
    }
}

/// Convenience codec containing an encoder and decoder for bidirectional bridges.
pub struct TetraSpeechCodec {
    pub encoder: TetraSpeechEncoder,
    pub decoder: TetraSpeechDecoder,
}

impl TetraSpeechCodec {
    pub fn new() -> Option<Self> {
        Some(Self {
            encoder: TetraSpeechEncoder::new()?,
            decoder: TetraSpeechDecoder::new()?,
        })
    }
}

fn split_tmd_block_to_codec_frames(data: &[u8]) -> Option<[[u8; TETRA_CODED_BYTES_PER_FRAME]; 2]> {
    let packed = if data.len() == TETRA_TMD_PACKED_BYTES + 1 {
        Some(&data[1..])
    } else if data.len() == TETRA_TMD_PACKED_BYTES {
        Some(data)
    } else {
        None
    };

    let mut frames = [[0u8; TETRA_CODED_BYTES_PER_FRAME]; 2];
    if let Some(packed) = packed {
        for bit_idx in 0..TETRA_TMD_BITS_PER_BLOCK {
            let bit = get_packed_bit(packed, bit_idx);
            set_packed_bit(
                &mut frames[bit_idx / TETRA_CODED_BITS_PER_FRAME],
                bit_idx % TETRA_CODED_BITS_PER_FRAME,
                bit,
            );
        }
        return Some(frames);
    }

    if data.len() < TETRA_TMD_BITS_PER_BLOCK {
        return None;
    }
    for bit_idx in 0..TETRA_TMD_BITS_PER_BLOCK {
        set_packed_bit(
            &mut frames[bit_idx / TETRA_CODED_BITS_PER_FRAME],
            bit_idx % TETRA_CODED_BITS_PER_FRAME,
            data[bit_idx] & 1,
        );
    }
    Some(frames)
}

fn join_codec_frames_to_tmd_block(
    frame_a: &[u8; TETRA_CODED_BYTES_PER_FRAME],
    frame_b: &[u8; TETRA_CODED_BYTES_PER_FRAME],
) -> Vec<u8> {
    let mut out = vec![0u8; TETRA_TMD_PACKED_BYTES];
    for bit_idx in 0..TETRA_TMD_BITS_PER_BLOCK {
        let frame = if bit_idx < TETRA_CODED_BITS_PER_FRAME { frame_a } else { frame_b };
        let frame_bit = bit_idx % TETRA_CODED_BITS_PER_FRAME;
        set_packed_bit(&mut out, bit_idx, get_packed_bit(frame, frame_bit));
    }
    out
}

fn get_packed_bit(data: &[u8], bit_idx: usize) -> u8 {
    (data[bit_idx / 8] >> (7 - (bit_idx % 8))) & 1
}

fn set_packed_bit(data: &mut [u8], bit_idx: usize, bit: u8) {
    if bit & 1 != 0 {
        data[bit_idx / 8] |= 1 << (7 - (bit_idx % 8));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packed_tmd_round_trip_keeps_274_bits() {
        let mut bits = [0u8; TETRA_TMD_BITS_PER_BLOCK];
        for (idx, bit) in bits.iter_mut().enumerate() {
            *bit = (idx % 3 == 0) as u8;
        }
        let frames = split_tmd_block_to_codec_frames(&bits).unwrap();
        let packed = join_codec_frames_to_tmd_block(&frames[0], &frames[1]);
        assert_eq!(packed.len(), TETRA_TMD_PACKED_BYTES);
        assert_eq!(frames, split_tmd_block_to_codec_frames(&packed).unwrap());
    }
}
