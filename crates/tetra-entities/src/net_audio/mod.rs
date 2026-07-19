//! Shared TETRA speech-codec helpers used by the SIP bridge, recorder and future media player.

pub mod codec;

pub use codec::{
    TETRA_CODED_BITS_PER_FRAME, TETRA_PCM_SAMPLE_RATE, TETRA_PCM_SAMPLES_PER_BLOCK, TETRA_PCM_SAMPLES_PER_FRAME,
    TetraSpeechCodec, TetraSpeechDecoder, TetraSpeechEncoder,
};
