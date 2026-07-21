#![allow(dead_code)]

// Protocol-only modules.  These are intentionally available without the
// `runtime` feature so external tools such as `netcore-control-room` can share
// the same wire structs without linking SDR/audio/native runtime libraries.
pub mod health;
pub mod legacy_wap;
pub mod net_control;
pub mod net_control_room;
pub mod net_telemetry;

// Full base-station/entity runtime.  Kept behind `runtime` so the Control Room
// Core can build in a lean LXC without SoapySDR, libgsm or libtetra-codec.
#[cfg(feature = "runtime")]
pub mod cmce;
#[cfg(feature = "runtime")]
pub mod entity_trait;
#[cfg(feature = "runtime")]
pub mod llc;
#[cfg(feature = "runtime")]
pub mod lmac;
#[cfg(feature = "runtime")]
pub mod messagerouter;
#[cfg(feature = "runtime")]
pub mod mle;
#[cfg(feature = "runtime")]
pub mod mm;
#[cfg(feature = "runtime")]
pub mod phy;
#[cfg(feature = "runtime")]
pub mod sndcp;
#[cfg(feature = "runtime")]
pub mod umac;

#[cfg(feature = "runtime")]
pub mod network;

#[cfg(feature = "tetra-codec")]
pub mod net_audio;
#[cfg(feature = "asterisk")]
pub mod net_asterisk;
#[cfg(feature = "runtime")]
pub mod net_brew;
#[cfg(feature = "runtime")]
pub mod net_dapnet;
#[cfg(feature = "runtime")]
pub mod net_dashboard;
#[cfg(feature = "runtime")]
pub mod net_echolink;
#[cfg(feature = "runtime")]
pub mod net_geoalarm;
#[cfg(feature = "runtime")]
pub mod net_meshcom;
#[cfg(feature = "recording")]
pub mod net_recorder;
#[cfg(feature = "audio-player")]
pub mod net_audio_player;
#[cfg(feature = "audio-player")]
pub mod net_tts;
#[cfg(feature = "runtime")]
pub mod net_snom;
#[cfg(feature = "runtime")]
pub mod net_telegram;

#[cfg(feature = "runtime")]
pub mod backlight;
#[cfg(feature = "runtime")]
pub mod service_control;
#[cfg(feature = "runtime")]
pub mod sys_telemetry;
#[cfg(feature = "runtime")]
pub mod tpg2200;
#[cfg(feature = "runtime")]
pub mod wifi;

// Re-export commonly used runtime items from router.
#[cfg(feature = "runtime")]
pub use entity_trait::TetraEntityTrait;
#[cfg(feature = "runtime")]
pub use messagerouter::{MessagePrio, MessageQueue, MessageRouter};
