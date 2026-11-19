//! The preview audio between an input and an output (spec 20261113-103922-preview-audio,
//! contracts/audio.md): the bridge that carries the samples, the pairing of a video device with
//! its audio input, and the user's choices. Pure code: the device streams are in
//! `crates/platform`, which the app connects to the bridge.

pub mod bridge;
pub mod choice;
pub mod pairing;
pub mod resample;
pub mod resync;
pub mod volume;
