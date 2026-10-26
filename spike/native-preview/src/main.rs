//! Path A: capture → native video sink (research R4).

use capture::{cli::CommonArgs, devices, video};
use clap::Parser;

#[derive(Parser, Debug)]
#[command(about = "Capture straight to a native video window")]
struct Args {
    #[command(flatten)]
    common: CommonArgs,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    println!("{}", capture::init()?);

    let all = devices::video_devices()?;
    if args.common.list {
        devices::print_list(&all);
        return Ok(());
    }
    let device = devices::pick(all, args.common.device.as_deref())?;
    let mode = video::choose_mode(&device, args.common.mode.as_ref())?;
    println!("{}: {mode}", device.name);
    Ok(())
}
