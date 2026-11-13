//! The present log (`--present-log <file>`, contracts/stats.md "Present log"; research R8): one
//! tab-separated line per acquire, present, window event and language switch stage, stamped with
//! `platform::qpc()` so the lines match PresentMon's `CPUStartQPC`. Lines go through a channel to a
//! writer thread: no file I/O on the video thread.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;
use std::sync::OnceLock;
use std::sync::mpsc::{self, Sender};

/// A window event the log records.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    Focus(bool),
    Fullscreen(bool),
}

/// A stage of a language switch (FR-024).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// Selecting the new catalog.
    Catalog,
    /// Handing the new texts to Slint and to the tray and window title.
    Property,
    /// Drawing the first UI image after the switch (main thread).
    Draw,
    /// Uploading that image (video thread).
    Upload,
    /// From the upload to the return of the present that shows it.
    Present,
}

impl Stage {
    fn name(self) -> &'static str {
        match self {
            Self::Catalog => "catalog",
            Self::Property => "property",
            Self::Draw => "draw",
            Self::Upload => "upload",
            Self::Present => "present",
        }
    }
}

/// One line of the log; every value is a raw `platform::qpc()` reading.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Line {
    /// Before asking the swap chain for an image (a blocking acquire shows up to the present).
    Acquire(i64),
    /// When the present returned; `frame` counts the presents of the video thread from 1.
    Present {
        qpc: i64,
        frame: u64,
    },
    Event(i64, Event),
    /// At the stage's end, with its length in ms.
    Stage(i64, Stage, f64),
}

impl Line {
    /// The line's text, without the line end.
    pub fn text(&self) -> String {
        match *self {
            Self::Acquire(qpc) => format!("acquire\t{qpc}"),
            Self::Present { qpc, frame } => format!("present\t{qpc}\tframe={frame}"),
            Self::Event(qpc, Event::Focus(on)) => format!("event\t{qpc}\tfocus={on}"),
            Self::Event(qpc, Event::Fullscreen(on)) => format!("event\t{qpc}\tfullscreen={on}"),
            Self::Stage(qpc, stage, ms) => format!("stage\t{qpc}\tlanguage={}\t{ms:.1}", stage.name()),
        }
    }
}

static LOG: OnceLock<Sender<Line>> = OnceLock::new();

/// Creates (or empties) `path` and starts the writer thread. Once per process.
pub fn start(path: &Path) -> std::io::Result<()> {
    let mut out = BufWriter::new(File::create(path)?);
    let (send, receive) = mpsc::channel::<Line>();
    std::thread::Builder::new().name("present-log".into()).spawn(move || {
        // Written as they come; flushed whenever the channel is empty, so a stall's lines are on
        // disk while it lasts and a killed app loses little.
        while let Ok(line) = receive.recv() {
            let mut result = writeln!(out, "{}", line.text());
            for line in receive.try_iter() {
                result = result.and_then(|()| writeln!(out, "{}", line.text()));
            }
            if let Err(err) = result.and_then(|()| out.flush()) {
                log::error!("present log: {err}; no more lines");
                return;
            }
        }
    })?;
    if LOG.set(send).is_err() {
        log::warn!("present log: already started");
    }
    Ok(())
}

/// Whether the log is on: callers skip their timing work when it is off.
pub fn enabled() -> bool {
    LOG.get().is_some()
}

/// Hands a line to the writer; nothing when the log is off.
pub fn write(line: Line) {
    if let Some(log) = LOG.get() {
        // The writer only goes on a write error, already logged.
        let _ = log.send(line);
    }
}

/// Logs an acquire now.
pub fn acquire() {
    if enabled() {
        write(Line::Acquire(platform::qpc()));
    }
}

/// Logs a present now.
pub fn present(frame: u64) {
    if enabled() {
        write(Line::Present {
            qpc: platform::qpc(),
            frame,
        });
    }
}

/// Logs a window event now.
pub fn event(event: Event) {
    if enabled() {
        write(Line::Event(platform::qpc(), event));
    }
}

/// Logs a language switch stage that ends now: to the present log when on, and as a stats line
/// with `--stats` (`stats`).
pub fn stage(stage: Stage, ms: f64, stats: bool) {
    if stats {
        crate::render::print_stats(&format!("[language] stage={} ms={ms:.1}", stage.name()));
    }
    if enabled() {
        write(Line::Stage(platform::qpc(), stage, ms));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_are_tab_separated() {
        assert_eq!(Line::Acquire(123_456_789_012).text(), "acquire\t123456789012");
        assert_eq!(
            Line::Present {
                qpc: 123_456_789_345,
                frame: 1
            }
            .text(),
            "present\t123456789345\tframe=1"
        );
        assert_eq!(Line::Event(5, Event::Focus(false)).text(), "event\t5\tfocus=false");
        assert_eq!(
            Line::Event(6, Event::Fullscreen(true)).text(),
            "event\t6\tfullscreen=true"
        );
        assert_eq!(
            Line::Stage(7, Stage::Catalog, 0.44).text(),
            "stage\t7\tlanguage=catalog\t0.4"
        );
        let names = [Stage::Property, Stage::Draw, Stage::Upload, Stage::Present].map(|s| s.name());
        assert_eq!(names, ["property", "draw", "upload", "present"]);
    }
}
