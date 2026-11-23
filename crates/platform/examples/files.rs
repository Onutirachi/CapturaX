//! The file manager calls of the screenshot (spec 20261121-134830-screenshot, research R12), to
//! check by hand on each system:
//!
//! ```sh
//! cargo run -p capturax-platform --example files                  # prints the default folder
//! cargo run -p capturax-platform --example files -- show <file>   # folder opened, file selected
//! cargo run -p capturax-platform --example files -- open <folder> # folder opened
//! ```

use std::path::Path;
use std::time::Duration;

use capturax_platform as platform;

fn main() {
    env_logger_lite();
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [] => println!("{}", platform::default_screenshots_folder().display()),
        [verb, path] if verb == "show" => platform::show_file(Path::new(path)),
        [verb, path] if verb == "open" => platform::open_folder(Path::new(path)),
        _ => {
            eprintln!("usage: files [show <file> | open <folder>]");
            std::process::exit(2);
        }
    }
    // The calls run on their own threads.
    std::thread::sleep(Duration::from_secs(3));
}

/// The log's warnings to stderr, without a logger crate.
fn env_logger_lite() {
    struct Stderr;
    impl log::Log for Stderr {
        fn enabled(&self, metadata: &log::Metadata) -> bool {
            metadata.level() <= log::Level::Info
        }
        fn log(&self, record: &log::Record) {
            if self.enabled(record.metadata()) {
                eprintln!("{} {}", record.level(), record.args());
            }
        }
        fn flush(&self) {}
    }
    static LOGGER: Stderr = Stderr;
    let _ = log::set_logger(&LOGGER);
    log::set_max_level(log::LevelFilter::Info);
}
