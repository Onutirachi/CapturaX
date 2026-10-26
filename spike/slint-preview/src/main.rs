//! Path B (video inside Slint) and the visual directions (research R5, R10).
//!
//! F11 or F toggles fullscreen on the current monitor; Esc or Q quits.

mod video_cpu;

use std::cell::RefCell;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use capture::{cli::CommonArgs, devices, stats, video};
use clap::{Parser, ValueEnum};
use gst::prelude::*;

slint::include_modules!();

/// Timing of the frame waiting to be drawn, and the samples of drawn frames (T046).
/// Both live on the UI thread only.
struct Pending {
    capture_to_sink_ms: Option<f64>,
    copy_ms: f64,
    wait_ms: f64,
    set_at: Instant,
}

thread_local! {
    static PENDING: RefCell<Option<Pending>> = const { RefCell::new(None) };
    /// capture→appsink, copy, wait for UI thread, UI update→render done (ms).
    static SAMPLES: RefCell<Vec<[f64; 4]>> = const { RefCell::new(Vec::new()) };
}

fn median(mut values: Vec<f64>) -> f64 {
    if values.is_empty() {
        return f64::NAN;
    }
    values.sort_by(f64::total_cmp);
    values[values.len() / 2]
}

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

    /// Slint renderer: femtovg, femtovg-wgpu, skia, skia-opengl, skia-wgpu, software (default: femtovg)
    #[arg(long)]
    renderer: Option<String>,
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

    let renderer = args.renderer.as_deref().unwrap_or("femtovg");
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .renderer_name(renderer.into())
        .select()
        .map_err(|e| anyhow::anyhow!("renderer {renderer:?} is not available: {e}"))?;
    println!("Slint renderer: {renderer}");

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
        move |pixels, timing: video_cpu::FrameTiming| {
            delivered.fetch_add(1, Ordering::Relaxed);
            let _ = app.upgrade_in_event_loop(move |app| {
                let now = Instant::now();
                app.set_video_frame(slint::Image::from_rgba8(pixels));
                PENDING.with_borrow_mut(|p| {
                    *p = Some(Pending {
                        capture_to_sink_ms: timing.capture_to_sink_ms,
                        copy_ms: timing.copy_ms,
                        wait_ms: (now - timing.ready).as_secs_f64() * 1e3,
                        set_at: now,
                    })
                });
            });
        }
    });

    let source = video::build(&device, mode.clone())?;
    let convert = video_cpu::converter()?;
    let pipeline = gst::Pipeline::with_name("slint-preview");
    pipeline.add_many([source.bin.upcast_ref::<gst::Element>(), convert.upcast_ref(), sink.upcast_ref()])?;
    gst::Element::link_many([source.bin.upcast_ref::<gst::Element>(), convert.upcast_ref(), sink.upcast_ref()])?;

    // Frames Slint actually drew: fewer than delivered means the UI fell behind and skipped some.
    let drawn = Arc::new(AtomicU64::new(0));
    let notifier = app.window().set_rendering_notifier({
        let drawn = drawn.clone();
        move |state, _| {
            if let slint::RenderingState::AfterRendering = state {
                drawn.fetch_add(1, Ordering::Relaxed);
                if let Some(p) = PENDING.with_borrow_mut(Option::take) {
                    let render_ms = p.set_at.elapsed().as_secs_f64() * 1e3;
                    let sample = [p.capture_to_sink_ms.unwrap_or(f64::NAN), p.copy_ms, p.wait_ms, render_ms];
                    SAMPLES.with_borrow_mut(|s| s.push(sample));
                }
            }
        }
    });
    if let Err(err) = notifier {
        // The software renderer has no rendering notifier: no draw counts or render timing then.
        println!("Note: {err}; draws and render timing are not measured with this renderer.");
    }

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
            let samples = SAMPLES.with_borrow_mut(std::mem::take);
            let col = |i: usize| median(samples.iter().map(|s| s[i]).filter(|v| v.is_finite()).collect());
            println!(
                "[time]  median ms: capture->appsink {:5.1} | copy {:4.1} | wait UI {:4.1} | UI->drawn {:4.1} | sum {:5.1}",
                col(0), col(1), col(2), col(3), col(0) + col(1) + col(2) + col(3)
            );
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
