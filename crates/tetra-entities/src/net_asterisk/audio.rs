use crate::net_audio::{TETRA_PCM_SAMPLES_PER_BLOCK, TetraSpeechCodec};

pub(crate) const PCMU_PAYLOAD_TYPE: u8 = 0;

pub(crate) struct AsteriskAudioTranscoder {
    codec: TetraSpeechCodec,
}

impl AsteriskAudioTranscoder {
    pub(crate) fn new() -> Option<Self> {
        Some(Self {
            codec: TetraSpeechCodec::new()?,
        })
    }

    pub(crate) fn decode_tmd_to_pcmu(&mut self, acelp: &[u8]) -> Option<Vec<u8>> {
        Some(
            self.codec
                .decoder
                .decode_tmd_to_pcm(acelp)?
                .into_iter()
                .map(linear_to_ulaw)
                .collect(),
        )
    }

    pub(crate) fn encode_pcmu_to_tmd(&mut self, payload: &[u8]) -> Vec<Vec<u8>> {
        let pcm: Vec<i16> = payload.iter().map(|&sample| ulaw_to_linear(sample)).collect();
        self.codec.encoder.push_pcm(&pcm)
    }
}

pub(crate) fn rtp_payload(packet: &[u8]) -> Option<(u8, &[u8])> {
    if packet.len() < 12 || packet[0] >> 6 != 2 {
        return None;
    }

    let has_padding = packet[0] & 0x20 != 0;
    let has_extension = packet[0] & 0x10 != 0;
    let csrc_count = (packet[0] & 0x0f) as usize;
    let payload_type = packet[1] & 0x7f;

    let mut end = packet.len();
    if has_padding {
        let padding = *packet.last()? as usize;
        if padding == 0 || padding > end {
            return None;
        }
        end -= padding;
    }

    let mut offset = 12 + csrc_count * 4;
    if offset > end {
        return None;
    }

    if has_extension {
        if offset + 4 > end {
            return None;
        }
        let extension_words = u16::from_be_bytes([packet[offset + 2], packet[offset + 3]]) as usize;
        offset += 4 + extension_words * 4;
        if offset > end {
            return None;
        }
    }

    Some((payload_type, &packet[offset..end]))
}

fn ulaw_to_linear(sample: u8) -> i16 {
    const BIAS: i16 = 0x84;

    let sample = !sample;
    let mantissa = (sample & 0x0f) as i16;
    let exponent = ((sample & 0x70) >> 4) as u32;
    let value = ((mantissa << 3) + BIAS) << exponent;

    if sample & 0x80 != 0 { BIAS - value } else { value - BIAS }
}

fn linear_to_ulaw(sample: i16) -> u8 {
    const BIAS: i32 = 0x84;
    const CLIP: i32 = 32635;
    const SEG_END: [i32; 8] = [0xff, 0x1ff, 0x3ff, 0x7ff, 0xfff, 0x1fff, 0x3fff, 0x7fff];

    let mut pcm = sample as i32;
    let mask = if pcm < 0 {
        pcm = -pcm;
        0x7f
    } else {
        0xff
    };
    pcm = pcm.min(CLIP) + BIAS;

    let segment = SEG_END.iter().position(|&end| pcm <= end).unwrap_or(SEG_END.len() - 1) as i32;
    let ulaw = ((segment << 4) | ((pcm >> (segment + 3)) & 0x0f)) as u8;

    ulaw ^ mask
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rtp_payload_skips_extension_and_padding() {
        let packet = [
            0b1011_0000,
            PCMU_PAYLOAD_TYPE,
            0,
            1,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            1,
            0xab,
            0xcd,
            0,
            1,
            0xaa,
            0xbb,
            0xcc,
            0xdd,
            0x11,
            0x22,
            2,
            2,
        ];
        let (pt, payload) = rtp_payload(&packet).unwrap();
        assert_eq!(pt, PCMU_PAYLOAD_TYPE);
        assert_eq!(payload, &[0x11, 0x22]);
    }

    #[test]
    fn a_tetra_block_is_sixty_milliseconds() {
        assert_eq!(TETRA_PCM_SAMPLES_PER_BLOCK, 480);
    }
}
