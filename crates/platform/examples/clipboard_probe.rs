//! Clipboard probe for the screenshot (spec 20261121-134830-screenshot, research R4, quickstart
//! "Probes" 1): puts a test image on the clipboard through `arboard` while another app has the
//! focus, and prints how long the call took.
//!
//! `cargo run -p capturax-platform --example clipboard_probe -- <width> <height>`
//!
//! Focus another app within 5 s of the start; the image is then set; paste it in GIMP or
//! Krita, Paint (Windows), Dolphin (Linux) and Discord in the browser. The probe keeps running
//! until Enter: on Linux the clipboard owner serves the image while it runs (on Wayland through
//! the data-control protocol, on X11 through the selection).
//!
//! The image: eight vertical color bars (white, yellow, cyan, green, magenta, red, blue, black)
//! over the top two thirds, and a left-to-right gray gradient below.

use std::io::BufRead;
use std::time::{Duration, Instant};

const BARS: [[u8; 3]; 8] = [
    [255, 255, 255],
    [255, 255, 0],
    [0, 255, 255],
    [0, 255, 0],
    [255, 0, 255],
    [255, 0, 0],
    [0, 0, 255],
    [0, 0, 0],
];

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let size = match args.as_slice() {
        [w, h] => w.parse::<usize>().ok().zip(h.parse::<usize>().ok()),
        _ => None,
    };
    let Some((width, height)) = size.filter(|&(w, h)| w > 0 && h > 0) else {
        eprintln!("usage: clipboard_probe <width> <height>");
        std::process::exit(2);
    };

    let rgba = pattern(width, height);
    println!("{width}x{height} test image ready; focus another app now (5 s)");
    std::thread::sleep(Duration::from_secs(5));

    let mut clipboard = match arboard::Clipboard::new() {
        Ok(clipboard) => clipboard,
        Err(error) => {
            eprintln!("no clipboard: {error}");
            std::process::exit(1);
        }
    };
    let start = Instant::now();
    let result = clipboard.set_image(arboard::ImageData {
        width,
        height,
        bytes: rgba.into(),
    });
    let took = start.elapsed();
    match result {
        Ok(()) => println!("set_image ok in {:.1} ms", took.as_secs_f64() * 1000.0),
        Err(error) => println!("set_image failed in {:.1} ms: {error}", took.as_secs_f64() * 1000.0),
    }

    println!("paste it now; press Enter to quit (the clipboard is served while this runs)");
    let _ = std::io::stdin().lock().lines().next();
}

/// The test image as RGBA rows.
fn pattern(width: usize, height: usize) -> Vec<u8> {
    let bars_end = height * 2 / 3;
    let mut rgba = Vec::with_capacity(width * height * 4);
    for y in 0..height {
        for x in 0..width {
            let rgb = if y < bars_end {
                BARS[x * BARS.len() / width]
            } else {
                let gray = (x * 255 / (width - 1).max(1)) as u8;
                [gray; 3]
            };
            rgba.extend_from_slice(&rgb);
            rgba.push(255);
        }
    }
    rgba
}
