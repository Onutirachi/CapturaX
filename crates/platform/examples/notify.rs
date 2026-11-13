//! Shows a system notification with an **Open** button and prints the action when it comes back
//! (research R15; the maintainer checks it at task T061: the name CapturaX, its icon, the button).
//!
//! `cargo run -p capturax-platform --example notify [-- <seconds> [<replace after>]]`: the first
//! notice for `<replace after>` seconds (8 by default), then the second for `<seconds>` (30 by
//! default); every action is printed.

use std::sync::mpsc;
use std::time::{Duration, Instant};

use platform::{Notice, NoticeIcon};

use capturax_platform as platform;

/// The icon's side in pixels; the app hands over the same size.
const ICON_SIZE: u32 = 64;
/// Seconds the first notice stays before the second replaces it.
const REPLACE_AFTER: u64 = 8;

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

/// `brand/X.svg` as straight RGBA, as app/build.rs rasterizes it.
fn icon() -> NoticeIcon {
    use resvg::{tiny_skia, usvg};
    let svg = concat!(env!("CARGO_MANIFEST_DIR"), "/../../brand/X.svg");
    let data = std::fs::read(svg).expect("brand/X.svg");
    let tree = usvg::Tree::from_data(&data, &usvg::Options::default()).expect("brand/X.svg must parse");
    let mut pixmap = tiny_skia::Pixmap::new(ICON_SIZE, ICON_SIZE).expect("icon size");
    let size = tree.size();
    let scale = (ICON_SIZE as f32 / size.width()).min(ICON_SIZE as f32 / size.height());
    let (dx, dy) = (
        (ICON_SIZE as f32 - size.width() * scale) / 2.0,
        (ICON_SIZE as f32 - size.height() * scale) / 2.0,
    );
    resvg::render(
        &tree,
        tiny_skia::Transform::from_row(scale, 0.0, 0.0, scale, dx, dy),
        &mut pixmap.as_mut(),
    );
    let rgba: Vec<u8> = pixmap
        .pixels()
        .iter()
        .flat_map(|p| {
            let c = p.demultiply();
            [c.red(), c.green(), c.blue(), c.alpha()]
        })
        .collect();
    NoticeIcon {
        size: ICON_SIZE,
        rgba: rgba.leak(),
    }
}

fn main() {
    let logger = Box::leak(Box::new(Stderr(Instant::now())));
    log::set_logger(logger).expect("no other logger");
    log::set_max_level(log::LevelFilter::Debug);

    let mut args = std::env::args().skip(1);
    let seconds: u64 = args.next().map(|s| s.parse().expect("seconds")).unwrap_or(30);
    let replace_after: u64 = args
        .next()
        .map(|s| s.parse().expect("replace after"))
        .unwrap_or(REPLACE_AFTER);

    let (tx, rx) = mpsc::channel();
    platform::init_notifications(icon(), move |tag| {
        let _ = tx.send(tag);
    });

    let first = Notice {
        title: "Example device connected".into(),
        action: Some("Open".into()),
        tag: "example:first".into(),
    };
    println!(
        "first shown: {} (Open now prints example:first)",
        platform::notify(&first)
    );
    print_actions(&rx, Duration::from_secs(replace_after));
    // Replaces the first one: only this should stay on screen, and its button reports its tag.
    let second = Notice {
        title: "Example device 2 connected".into(),
        tag: "example:second".into(),
        ..first
    };
    println!(
        "second shown: {} (replaces the first; Open prints example:second)",
        platform::notify(&second)
    );
    print_actions(&rx, Duration::from_secs(seconds));
    println!("done");
}

/// Prints every action that comes back for `wait`.
fn print_actions(rx: &mpsc::Receiver<String>, wait: Duration) {
    let end = Instant::now() + wait;
    while let Ok(tag) = rx.recv_timeout(end.saturating_duration_since(Instant::now())) {
        println!("action: {tag}");
    }
}
