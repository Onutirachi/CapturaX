//! The CapturaX platform layer: the only crate that calls OS-specific APIs (constitution IV).

#[cfg(windows)]
mod windows;

#[cfg(target_os = "linux")]
mod linux;
