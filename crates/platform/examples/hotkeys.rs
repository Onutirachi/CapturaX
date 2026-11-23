//! Global hotkeys check (screenshot spec, research R6 and R9, quickstart "Hotkeys"): takes the
//! given keys through the platform's `Hotkeys` and prints every event, so a press can be checked
//! while another app (a fullscreen game) has the focus. Windows, Linux X11, and Wayland through
//! the GlobalShortcuts portal (the desktop asks once; its key wins over the one given here).
//!
//! `cargo run -p capturax-platform --example hotkeys -- [<key>…] [--suspend-after <s>]`
//!
//! - `<key>`: a hotkey in the settings form, default `Ctrl+Shift+F8`; each one is an action
//!   `key1`, `key2`… A key another app holds is printed as `Bound { shown: None }`.
//! - `--suspend-after <s>`: releases the keys after `s` seconds for 5 s, then takes them again
//!   (the key recording of FR-017).
//!
//! Ends after 60 s, or with Ctrl+C.

use std::time::Duration;

use capturax_platform::{Hotkey, Hotkeys};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut suspend_after = None;
    let mut keys = Vec::new();
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if arg == "--suspend-after" {
            suspend_after = iter.next().and_then(|s| s.parse::<u64>().ok());
        } else {
            match Hotkey::parse(arg) {
                Some(key) => keys.push(key),
                None => {
                    eprintln!("hotkeys: {arg:?} is not a hotkey");
                    std::process::exit(2);
                }
            }
        }
    }
    if keys.is_empty() {
        keys.push(Hotkey::parse("Ctrl+Shift+F8").expect("valid"));
    }
    let bindings: Vec<_> = keys
        .iter()
        .enumerate()
        .map(|(i, key)| (format!("key{}", i + 1), Some(*key), format!("Key {}", i + 1)))
        .collect();
    let start = std::time::Instant::now();
    let hotkeys = Hotkeys::start(move |event| {
        println!("{:>7.3} s  {event:?}", start.elapsed().as_secs_f64());
    });
    hotkeys.set(bindings);
    println!(
        "holding {}; press them with another app focused (60 s)",
        keys.iter().map(ToString::to_string).collect::<Vec<_>>().join(", ")
    );
    let mut left = Duration::from_secs(60);
    if let Some(after) = suspend_after {
        std::thread::sleep(Duration::from_secs(after));
        println!("suspended for 5 s");
        hotkeys.suspend(true);
        std::thread::sleep(Duration::from_secs(5));
        println!("taken again");
        hotkeys.suspend(false);
        left = left.saturating_sub(Duration::from_secs(after + 5));
    }
    std::thread::sleep(left);
    drop(hotkeys);
    println!("released");
}
