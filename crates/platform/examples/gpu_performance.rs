//! Holds the GPU performance request for a while and prints what happened (research R7; task
//! T025 checks the clocks with `nvidia-smi` meanwhile).
//!
//! `cargo run --release -p capturax-platform --example gpu_performance [-- <seconds> [<pci bus id>]]`

use std::time::{Duration, Instant};

struct Stderr(Instant);

impl log::Log for Stderr {
    fn enabled(&self, _: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        let at = self.0.elapsed().as_secs_f64();
        eprintln!("{at:8.3} {:5} {}", record.level(), record.args());
    }

    fn flush(&self) {}
}

fn main() {
    let logger = Box::leak(Box::new(Stderr(Instant::now())));
    log::set_logger(logger).expect("no other logger");
    log::set_max_level(log::LevelFilter::Debug);

    let mut args = std::env::args().skip(1);
    let seconds: u64 = args
        .next()
        .map_or(60, |text| text.parse().expect("seconds as a whole number"));
    let pci_bus_id = args.next();

    log::info!("requesting for {seconds} s");
    let guard = capturax_platform::gpu_performance(pci_bus_id.as_deref());
    std::thread::sleep(Duration::from_secs(seconds));
    log::info!("releasing");
    drop(guard);
    log::info!("done");
}
