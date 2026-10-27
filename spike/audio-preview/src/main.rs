//! Low-latency audio passthrough (research R7, T023): the capture card's audio played on a chosen
//! output, with the configured and reported latencies printed.
//!
//! Ctrl+C quits.

use std::time::{Duration, Instant};

use capture::{audio, cli::CommonArgs, devices};
use clap::Parser;
use gst::prelude::*;

#[derive(Parser, Debug)]
#[command(about = "Plays the capture card's audio on an output with the lowest delay")]
struct Args {
    #[command(flatten)]
    common: CommonArgs,

    /// Audio input: index from --list-audio or part of its name (default: paired with the video device)
    #[arg(long)]
    input: Option<String>,

    /// Audio output: index from --list-audio or part of its name (default: system default)
    #[arg(long)]
    output: Option<String>,

    /// Print audio inputs and outputs, then exit
    #[arg(long)]
    list_audio: bool,

    /// Play each block as soon as it arrives instead of on the pipeline clock
    #[arg(long)]
    free_run: bool,

    /// Output buffer in milliseconds (GStreamer's default is 200)
    #[arg(long)]
    buffer: Option<u64>,

    /// With --free-run, the most audio waiting between input and output, in milliseconds
    #[arg(long, default_value_t = 20)]
    max_queue: u64,

    /// Open input, output or both in WASAPI exclusive mode (Windows)
    #[arg(long)]
    exclusive: Option<Exclusive>,
}

#[derive(clap::ValueEnum, Clone, Copy, Debug, PartialEq)]
enum Exclusive {
    Input,
    Output,
    Both,
}

/// Microseconds from an integer property, if the element has it (WASAPI elements only).
fn micros(element: &gst::Element, name: &str) -> Option<i64> {
    element.find_property(name)?;
    let value = element.property_value(name);
    value.get::<i64>().ok().or_else(|| value.get::<i32>().ok().map(i64::from))
}

fn ms(micros: Option<i64>) -> String {
    micros.map_or("n/a".into(), |us| format!("{:.1} ms", us as f64 / 1000.0))
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    println!("{}", capture::init()?);

    let inputs = audio::audio_devices("Audio/Source")?;
    let outputs = audio::audio_devices("Audio/Sink")?;
    if args.list_audio {
        audio::print_list(&inputs, &outputs);
        return Ok(());
    }

    let input = match &args.input {
        Some(wanted) => audio::pick(inputs, wanted)?,
        None => {
            let video = devices::pick(devices::video_devices()?, args.common.device.as_deref())?;
            audio::paired_input(inputs, &video.name).ok_or_else(|| {
                anyhow::anyhow!("no audio input paired with {}; choose one with --input (see --list-audio)", video.name)
            })?
        }
    };
    let output = args.output.as_deref().map(|wanted| audio::pick(outputs, wanted)).transpose()?;
    println!("input:  {}", input.name);
    println!("output: {}", output.as_ref().map_or("system default", |d| d.name.as_str()));

    let options = audio::PreviewOptions {
        free_run: args.free_run,
        buffer_ms: args.buffer,
        max_queue_ms: args.max_queue,
        exclusive_input: matches!(args.exclusive, Some(Exclusive::Input | Exclusive::Both)),
        exclusive_output: matches!(args.exclusive, Some(Exclusive::Output | Exclusive::Both)),
    };
    let audio::Preview { pipeline, input: src, output: sink } = audio::build_preview(&input, output.as_ref(), &options)?;
    pipeline.set_state(gst::State::Playing)?;

    let bus = pipeline.bus().expect("pipeline has a bus");
    let mut reported = false;
    let mut last_stats = Instant::now();
    let stats = args.common.stats || args.common.record;
    println!("Playing. Ctrl+C quits.");
    loop {
        if let Some(msg) = bus.timed_pop(gst::ClockTime::from_mseconds(100)) {
            match msg.view() {
                gst::MessageView::Error(err) => {
                    anyhow::bail!("error from {:?}: {} ({:?})", err.src().map(|s| s.path_string()), err.error(), err.debug())
                }
                gst::MessageView::Eos(..) => break,
                _ => {}
            }
        }
        let playing = pipeline.current_state() == gst::State::Playing;
        if playing && !reported {
            reported = true;
            println!(
                "input  period {} buffer {} | output buffer {}",
                ms(micros(&src, "actual-latency-time")),
                ms(micros(&src, "actual-buffer-time")),
                ms(micros(&sink, "buffer-time")),
            );
        }
        if playing && stats && last_stats.elapsed() >= Duration::from_secs(1) {
            last_stats = Instant::now();
            let mut query = gst::query::Latency::new();
            if pipeline.query(&mut query) {
                let (live, min, _) = query.result();
                println!("[audio] pipeline latency {:.1} ms (live: {live})", min.nseconds() as f64 / 1e6);
            }
        }
    }
    pipeline.set_state(gst::State::Null)?;
    Ok(())
}
