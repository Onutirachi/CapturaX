//! Path B (video inside Slint) and the visual directions (research R5, R10).
//!
//! F11 or F toggles fullscreen on the current monitor; Esc or Q quits.

mod video_cpu;

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use capture::{cli::CommonArgs, devices, stats, video};
use clap::{Parser, ValueEnum};
use gst::prelude::*;

slint::include_modules!();

#[derive(Clone, Copy, Debug, PartialEq, ValueEnum)]
enum Transfer {
    Cpu,
    Gpu,
}

#[derive(Parser, Debug)]
#[command(about = "Capture shown inside a Slint window")]
struct Args {
    #[command(flatten)]
    common: CommonArgs,

    /// Video only, no chrome (the measured path B)
    #[arg(long)]
    bare: bool,

    /// How frames reach Slint
    #[arg(long, value_enum, default_value = "cpu")]
    transfer: Transfer,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    println!("{}", capture::init()?);

    let all = devices::video_devices()?;
    if args.common.list {
        devices::print_list(&all);
        return Ok(());
    }
    if args.transfer == Transfer::Gpu {
        anyhow::bail!("--transfer gpu is not implemented yet (T014)");
    }
    if !args.bare {
        anyhow::bail!("only --bare exists so far; the visual directions come in T027–T032");
    }

    let app = App::new()?;
    let window = app.as_weak();
    app.on_toggle_fullscreen(move || {
        if let Some(app) = window.upgrade() {
            let w = app.window();
            w.set_fullscreen(!w.is_fullscreen());
        }
    });
    app.on_quit(|| {
        let _ = slint::quit_event_loop();
    });

    let device = match devices::pick(all, args.common.device.as_deref()) {
        Ok(device) => device,
        Err(err) => {
            // No device: say so in the window instead of showing an empty video area (spec edge case).
            app.set_message(format!("{err}").into());
            app.run()?;
            return Ok(());
        }
    };
    let mode = video::choose_mode(&device, args.common.mode.as_ref())?;
    println!("{}: {mode} -> slint ({:?})", device.name, args.transfer);

    // Frames arrive on a GStreamer thread; the newest one is handed to the UI thread.
    let delivered = Arc::new(AtomicU64::new(0));
    let sink = video_cpu::appsink({
        let app = app.as_weak();
        let delivered = delivered.clone();
        move |pixels| {
            delivered.fetch_add(1, Ordering::Relaxed);
            let _ = app.upgrade_in_event_loop(move |app| app.set_video_frame(slint::Image::from_rgba8(pixels)));
        }
    });

    let source = video::build(&device, mode.clone())?;
    let convert = video_cpu::converter()?;
    let pipeline = gst::Pipeline::with_name("slint-preview");
    pipeline.add_many([source.bin.upcast_ref::<gst::Element>(), convert.upcast_ref(), sink.upcast_ref()])?;
    gst::Element::link_many([source.bin.upcast_ref::<gst::Element>(), convert.upcast_ref(), sink.upcast_ref()])?;

    // Frames Slint actually drew: fewer than delivered means the UI fell behind and skipped some.
    let drawn = Arc::new(AtomicU64::new(0));
    app.window().set_rendering_notifier({
        let drawn = drawn.clone();
        move |state, _| {
            if let slint::RenderingState::AfterRendering = state {
                drawn.fetch_add(1, Ordering::Relaxed);
            }
        }
    })?;

    let _stats = if args.common.stats || args.common.record {
        let info = stats::RunInfo { path: "slint-cpu".into(), variant: "bare".into(), mode: mode.to_string() };
        let reporter = stats::StatsReporter::start(&pipeline, sink.upcast_ref(), info, args.common.record)?;
        Some(reporter)
    } else {
        None
    };
    let ui_timer = slint::Timer::default();
    if args.common.stats || args.common.record {
        let (mut last_delivered, mut last_drawn) = (0u64, 0u64);
        ui_timer.start(slint::TimerMode::Repeated, std::time::Duration::from_secs(1), move || {
            let (d, r) = (delivered.load(Ordering::Relaxed), drawn.load(Ordering::Relaxed));
            println!("[ui]    {:>4} fps delivered to the UI | {:>4} fps drawn", d - last_delivered, r - last_drawn);
            (last_delivered, last_drawn) = (d, r);
        });
    }

    // Slint runs its own event loop (not GLib's), so the bus is read on a thread.
    let bus = pipeline.bus().expect("pipeline has a bus");
    std::thread::spawn(move || {
        for msg in bus.iter_timed(gst::ClockTime::NONE) {
            if let gst::MessageView::Error(err) = msg.view() {
                eprintln!("Error from {:?}: {} ({:?})", err.src().map(|s| s.path_string()), err.error(), err.debug());
                let _ = slint::invoke_from_event_loop(|| {
                    let _ = slint::quit_event_loop();
                });
                break;
            }
        }
    });

    pipeline.set_state(gst::State::Playing)?;
    println!("F11 or F: fullscreen on the current monitor. Esc or Q: quit.");
    app.run()?;
    pipeline.set_state(gst::State::Null)?;
    Ok(())
}
