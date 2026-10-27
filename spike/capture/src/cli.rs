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

    /// Override the color range the device declares: full (0-255) or limited (16-235)
    #[arg(long)]
    pub range: Option<crate::video::Range>,

    /// Print frames per second and dropped frames every second (nothing is written)
    #[arg(long)]
    pub stats: bool,

    /// Like --stats, and append the rows to measurements/pipeline-stats.csv (official runs only)
    #[arg(long)]
    pub record: bool,
}
