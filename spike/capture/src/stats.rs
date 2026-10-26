//! `--stats`: rendered and dropped frames plus in-pipeline latency, once per second, printed and
//! appended to measurements/pipeline-stats.csv (contracts/measurements.md).

use gst::prelude::*;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const HEADER: &str = "run,path,variant,mode,platform,second,rendered,dropped,pipeline_latency_ms";

/// What is being measured; becomes the leading columns of every row.
pub struct RunInfo {
    pub path: String,
    pub variant: String,
    pub mode: String,
}

pub struct StatsReporter {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

fn csv_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../specs/20261026-154808-stack-spike/measurements/pipeline-stats.csv")
}

fn platform() -> &'static str {
    if cfg!(windows) { "windows" } else { "linux" }
}

impl StatsReporter {
    /// Starts reporting for `sink` (any base sink) inside `pipeline`.
    pub fn start(pipeline: &gst::Pipeline, sink: &gst::Element, info: RunInfo) -> anyhow::Result<Self> {
        let path = csv_path();
        let new_file = !path.exists();
        let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
        if new_file {
            writeln!(file, "{HEADER}")?;
        }
        let run = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        let stop = Arc::new(AtomicBool::new(false));
        let (pipeline, sink, flag) = (pipeline.clone(), sink.clone(), stop.clone());

        let thread = std::thread::spawn(move || {
            let mut second = 0u64;
            while !flag.load(Ordering::Relaxed) {
                std::thread::sleep(Duration::from_secs(1));
                second += 1;
                let stats = sink.property::<gst::Structure>("stats");
                let rendered = stats.get::<u64>("rendered").unwrap_or(0);
                let dropped = stats.get::<u64>("dropped").unwrap_or(0);
                let mut query = gst::query::Latency::new();
                let latency_ms = if pipeline.query(&mut query) {
                    query.result().1.mseconds().to_string()
                } else {
                    String::new()
                };
                println!("[stats] {second:>4}s rendered {rendered:>7} dropped {dropped:>5} pipeline latency {latency_ms} ms");
                let _ = writeln!(
                    file,
                    "{run},{},{},{},{},{second},{rendered},{dropped},{latency_ms}",
                    info.path,
                    info.variant,
                    info.mode,
                    platform()
                );
            }
        });
        Ok(Self { stop, thread: Some(thread) })
    }
}

impl Drop for StatsReporter {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
