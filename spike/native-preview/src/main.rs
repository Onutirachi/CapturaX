//! Path A: capture → native video sink (research R4).
//!
//! The sink opens its own window on the primary monitor. Drag it to the preview monitor, then press
//! F11 (or F) to toggle fullscreen there. Esc or Q quits.

use capture::{cli::CommonArgs, devices, stats, video};
use clap::{Parser, ValueEnum};
use gst::prelude::*;

#[derive(Clone, Copy, Debug, ValueEnum)]
enum Sink {
    D3d12,
    D3d11,
    Gl,
    Wayland,
    Vulkan,
}

#[derive(Parser, Debug)]
#[command(about = "Capture straight to a native video window")]
struct Args {
    #[command(flatten)]
    common: CommonArgs,

    /// Video sink (default: d3d12 on Windows, gl on Linux)
    #[arg(long, value_enum)]
    sink: Option<Sink>,

    /// Start fullscreen (on the monitor where the window opens, usually the primary one)
    #[arg(long)]
    fullscreen: bool,
}

/// The sink chain as a bin with a ghost sink pad, plus the actual sink element (for stats).
fn sink_bin(kind: Sink) -> anyhow::Result<(gst::Bin, gst::Element)> {
    let names: &[&str] = match kind {
        Sink::D3d12 => &["d3d12videosink"],
        Sink::D3d11 => &["d3d11videosink"],
        Sink::Gl => &["glimagesink"],
        Sink::Wayland => &["videoconvert", "waylandsink"],
        Sink::Vulkan => &["vulkanupload", "vulkancolorconvert", "vulkansink"],
    };
    let bin = gst::Bin::with_name("display");
    let elements = names
        .iter()
        .map(|name| gst::ElementFactory::make(name).build())
        .collect::<Result<Vec<_>, _>>()?;
    bin.add_many(&elements)?;
    gst::Element::link_many(&elements)?;

    let sink = elements.last().unwrap().clone();
    // Show every frame as soon as it arrives instead of waiting for its timestamp.
    sink.set_property("sync", false);
    if matches!(kind, Sink::D3d11) {
        sink.set_property_from_str("fullscreen-toggle-mode", "property");
    }
    let pad = elements[0].static_pad("sink").expect("first element has a sink pad");
    bin.add_pad(&gst::GhostPad::with_target(&pad)?)?;
    // glimagesink is itself a bin; its inner sink element holds the stats.
    let sink = match sink.downcast_ref::<gst::Bin>() {
        Some(inner) => inner.iterate_sinks().into_iter().flatten().next().unwrap_or(sink),
        None => sink,
    };
    Ok((bin, sink))
}

fn toggle_fullscreen(sink: &gst::Element) {
    if sink.has_property("fullscreen") {
        let on = sink.property::<bool>("fullscreen");
        sink.set_property("fullscreen", !on);
    }
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
    let kind = args.sink.unwrap_or(if cfg!(windows) { Sink::D3d12 } else { Sink::Gl });
    println!("{}: {mode} -> {kind:?}", device.name);

    let source = video::build(&device, mode.clone(), args.common.range)?;
    let (display, sink) = sink_bin(kind)?;
    let pipeline = gst::Pipeline::with_name("native-preview");
    pipeline.add_many([source.bin.upcast_ref::<gst::Element>(), display.upcast_ref()])?;
    source.bin.link(&display)?;

    if args.fullscreen && sink.has_property("fullscreen") {
        sink.set_property("fullscreen", true);
    }
    let _stats = if args.common.stats || args.common.record {
        let info = stats::RunInfo {
            path: "native".into(),
            variant: format!("{kind:?}").to_lowercase(),
            mode: mode.to_string(),
        };
        Some(stats::StatsReporter::start(&pipeline, &sink, info, args.common.record)?)
    } else {
        None
    };

    pipeline.set_state(gst::State::Playing)?;
    println!("F11 or F: fullscreen on the current monitor. Esc or Q: quit.");

    let bus = pipeline.bus().expect("pipeline has a bus");
    for msg in bus.iter_timed(gst::ClockTime::NONE) {
        match msg.view() {
            gst::MessageView::Eos(..) => break,
            gst::MessageView::Error(err) => {
                eprintln!("Error from {:?}: {} ({:?})", err.src().map(|s| s.path_string()), err.error(), err.debug());
                break;
            }
            gst::MessageView::Element(..) => {
                let Ok(gst_video::NavigationMessage::Event(nav)) = gst_video::NavigationMessage::parse(&msg) else {
                    continue;
                };
                let Ok(gst_video::NavigationEvent::KeyPress { key, .. }) = gst_video::NavigationEvent::parse(&nav.event)
                else {
                    continue;
                };
                match key.as_str() {
                    "F11" | "f" | "F" => toggle_fullscreen(&sink),
                    "Escape" | "q" | "Q" => break,
                    _ => {}
                }
            }
            _ => {}
        }
    }
    pipeline.set_state(gst::State::Null)?;
    Ok(())
}
