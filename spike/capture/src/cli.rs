//! Options shared by native-preview, slint-preview and audio-preview (contracts/cli.md).

use crate::mode::{self, Mode};

#[derive(clap::Args, Debug)]
pub struct CommonArgs {
    /// Print devices and their modes, then exit
    #[arg(long)]
    pub list: bool,

    /// Video device: index from --list or part of its name (default: the first one)
    #[arg(long)]
    pub device: Option<String>,

    /// Mode as WxH@FPS[:FORMAT], e.g. 1920x1080@60:NV12 (default: Auto)
    #[arg(long, value_parser = mode::parse)]
    pub mode: Option<Mode>,

    /// Print rendered/dropped frames and pipeline latency every second, and log them to CSV
    #[arg(long)]
    pub stats: bool,
}
