//! Presentation statistics (spec FR-005, contracts/stats.md): counted by the video thread, printed
//! once per second with `--stats`. The second's summary (presented frames, measured latency) is
//! kept always, for the bubbles (FR-016).

use std::fmt::Write as _;
use std::time::{Duration, Instant};

use capture::{Matrix, Mode, PixelFormat, Range};

/// The `[present]` header: printed when the video thread starts and when the presentation changes.
pub fn header(backend: &str, mode: &str, adapter: &str, surface: (u32, u32)) -> String {
    format!(
        "[present] backend={backend} mode={mode} adapter=\"{adapter}\" surface={}x{}",
        surface.0, surface.1
    )
}

/// The open source, for its `[source]` line.
#[derive(Debug, Clone)]
pub struct Source {
    pub device: String,
    pub id: String,
    pub mode: Mode,
    /// What the session plays, once the device accepted the format.
    pub playing: Option<(PixelFormat, Matrix)>,
}

impl Source {
    /// The `[source]` line with the range in use (`auto`: inferred), once the source plays.
    pub fn line(&self, range: Range, auto: bool) -> Option<String> {
        let (format, matrix) = self.playing?;
        Some(format!(
            "[source] device=\"{}\" id=\"{}\" mode={}x{}@{} format={} matrix={} range={}{}",
            self.device,
            self.id,
            self.mode.width,
            self.mode.height,
            capture::mode::rate_text(self.mode.fps),
            format.name(),
            match matrix {
                Matrix::Bt601 => "bt601",
                Matrix::Bt709 => "bt709",
            },
            match range {
                Range::Full => "full",
                Range::Limited => "limited",
            },
            if auto { "(auto)" } else { "" },
        ))
    }
}

/// One second of presented video, for the frame rate and latency bubbles (FR-016).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Summary {
    /// Presents with a new video frame.
    pub presented: u32,
    /// Median of captured → presented, in ms; `None` without frames (research R11).
    pub latency_ms: Option<f64>,
}

impl Summary {
    pub const EMPTY: Self = Self {
        presented: 0,
        latency_ms: None,
    };
}

/// What a second gives: its summary and, with `--stats`, its `[loop]` line.
#[derive(Debug, Clone, PartialEq)]
pub struct Report {
    pub summary: Summary,
    pub line: Option<String>,
}

/// Counts one second of the video thread's work: the summary always, the rest of the `[loop]`
/// line only with `--stats` (no work per frame for it otherwise, FR-015).
pub struct Stats {
    /// The `[loop]` line is produced (`--stats`).
    lines: bool,
    start: Instant,
    second_start: Instant,
    received: u32,
    presented: u32,
    replaced: u32,
    /// Frame ready → present returned, in ms, for the presents of this second.
    ready_ms: Vec<f64>,
    /// The last present with a new frame; kept across seconds so a gap spanning two is counted.
    last_present: Option<Instant>,
    gap_ms: Option<f64>,
    ui: u32,
    ui_ms: Vec<f64>,
    moves: u32,
    /// Captured → presented, in ms, for the presents of this second.
    latency_ms: Vec<f64>,
    /// Presents that repeated the last frame (Follow the monitor, T047).
    repeated: u32,
    /// The last summary had frames: one more report is due to tell they stopped.
    had_frames: bool,
}

impl Stats {
    pub fn new(now: Instant, lines: bool) -> Self {
        Self {
            lines,
            start: now,
            second_start: now,
            received: 0,
            presented: 0,
            replaced: 0,
            ready_ms: Vec::new(),
            last_present: None,
            gap_ms: None,
            ui: 0,
            ui_ms: Vec::new(),
            moves: 0,
            latency_ms: Vec::new(),
            repeated: 0,
            had_frames: false,
        }
    }

    /// Frames received from the source (`count`, since the video thread last looked).
    pub fn received(&mut self, count: u32) {
        self.received += count;
    }

    /// Frames replaced in the inbox before being presented.
    pub fn replaced(&mut self, count: u32) {
        self.replaced += count;
    }

    /// A present with a new video frame returned at `now`; the frame was captured at `captured`
    /// and ready at `ready`.
    pub fn presented(&mut self, captured: Instant, ready: Instant, now: Instant) {
        self.presented += 1;
        self.latency_ms.push(ms(now.saturating_duration_since(captured)));
        if !self.lines {
            return;
        }
        self.ready_ms.push(ms(now.saturating_duration_since(ready)));
        if let Some(last) = self.last_present.replace(now) {
            let gap = ms(now.saturating_duration_since(last));
            self.gap_ms = Some(self.gap_ms.map_or(gap, |g| g.max(gap)));
        }
    }

    /// A present that repeated the last frame (Follow the monitor, research R9).
    pub fn repeated(&mut self) {
        self.repeated += 1;
    }

    /// A UI image handed to the video thread, drawn and copied in `draw_ms`.
    pub fn ui_image(&mut self, draw_ms: f64) {
        if self.lines {
            self.ui += 1;
            self.ui_ms.push(draw_ms);
        }
    }

    /// Window move events (`count`, since the video thread last looked).
    pub fn window_moved(&mut self, count: u32) {
        self.moves += count;
    }

    /// When the next report is due, if one is wanted: always with `--stats`, else while frames
    /// are presented and once after they stop.
    pub fn next_report(&self) -> Option<Instant> {
        (self.lines || self.presented > 0 || self.had_frames).then(|| self.second_start + Duration::from_secs(1))
    }

    /// The second's report once a second has passed since the last one, then starts the next
    /// second.
    pub fn report(&mut self, now: Instant) -> Option<Report> {
        if now < self.second_start + Duration::from_secs(1) {
            return None;
        }
        let latency = median_max(&mut self.latency_ms);
        let summary = Summary {
            presented: self.presented,
            latency_ms: latency.map(|(median, _)| median),
        };
        self.had_frames = summary.presented > 0;
        let line = self.lines.then(|| self.line(now, latency));
        self.second_start = now;
        self.received = 0;
        self.presented = 0;
        self.replaced = 0;
        self.ready_ms.clear();
        self.gap_ms = None;
        self.ui = 0;
        self.ui_ms.clear();
        self.moves = 0;
        self.latency_ms.clear();
        self.repeated = 0;
        Some(Report { summary, line })
    }

    fn line(&mut self, now: Instant, latency: Option<(f64, f64)>) -> String {
        let mut line = String::with_capacity(112);
        let _ = write!(
            line,
            "[loop] t={} recv={} pres={} repl={} ready_ms={} gap_ms={} ui={} ui_ms={} moves={} \
             lat_ms={} rep={}",
            now.saturating_duration_since(self.start).as_secs(),
            self.received,
            self.presented,
            self.replaced,
            median_max_text(median_max(&mut self.ready_ms)),
            self.gap_ms.map_or("-".to_string(), |g| format!("{g:.1}")),
            self.ui,
            median_max_text(median_max(&mut self.ui_ms)),
            self.moves,
            median_max_text(latency),
            self.repeated,
        );
        line
    }
}

fn ms(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1e3
}

/// The median and the maximum, `None` without samples.
fn median_max(samples: &mut [f64]) -> Option<(f64, f64)> {
    if samples.is_empty() {
        return None;
    }
    samples.sort_by(f64::total_cmp);
    let n = samples.len();
    let median = if n % 2 == 1 {
        samples[n / 2]
    } else {
        (samples[n / 2 - 1] + samples[n / 2]) / 2.0
    };
    Some((median, samples[n - 1]))
}

/// `median/max` with one decimal, `-/-` without samples.
fn median_max_text(value: Option<(f64, f64)>) -> String {
    value.map_or_else(|| "-/-".into(), |(median, max)| format!("{median:.1}/{max:.1}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(start: Instant, ms: u64) -> Instant {
        start + Duration::from_millis(ms)
    }

    #[test]
    fn header_format() {
        assert_eq!(
            header("dx12", "mailbox", "NVIDIA GeForce RTX 5070 Ti", (1920, 1080)),
            "[present] backend=dx12 mode=mailbox adapter=\"NVIDIA GeForce RTX 5070 Ti\" surface=1920x1080"
        );
    }

    #[test]
    fn source_line() {
        let mut source = Source {
            device: "ezcap Live Gamer Ultra".into(),
            id: "mf:abc".into(),
            mode: Mode {
                width: 1920,
                height: 1080,
                fps: (60, 1),
                format: capture::mode::Format::Nv12,
            },
            playing: None,
        };
        assert_eq!(source.line(Range::Full, true), None);
        source.playing = Some((PixelFormat::Nv12, Matrix::Bt709));
        assert_eq!(
            source.line(Range::Full, true).unwrap(),
            "[source] device=\"ezcap Live Gamer Ultra\" id=\"mf:abc\" mode=1920x1080@60 format=NV12 matrix=bt709 range=full(auto)"
        );
        source.mode.fps = (60000, 1001);
        source.playing = Some((PixelFormat::Yuy2, Matrix::Bt601));
        assert_eq!(
            source.line(Range::Limited, false).unwrap(),
            "[source] device=\"ezcap Live Gamer Ultra\" id=\"mf:abc\" mode=1920x1080@59.94 format=YUY2 matrix=bt601 range=limited"
        );
    }

    /// The `[loop]` line of a report.
    fn line(report: Option<Report>) -> String {
        report.expect("a report").line.expect("a line")
    }

    #[test]
    fn empty_second() {
        let start = Instant::now();
        let mut stats = Stats::new(start, true);
        let report = stats.report(at(start, 1000)).unwrap();
        assert_eq!(report.summary, Summary::EMPTY);
        assert_eq!(
            report.line.unwrap(),
            "[loop] t=1 recv=0 pres=0 repl=0 ready_ms=-/- gap_ms=- ui=0 ui_ms=-/- moves=0 lat_ms=-/- rep=0"
        );
    }

    #[test]
    fn repeats_count_apart_from_new_frames_for_one_second() {
        let start = Instant::now();
        let mut stats = Stats::new(start, true);
        stats.presented(at(start, 10), at(start, 11), at(start, 12));
        stats.repeated();
        stats.repeated();
        let first = line(stats.report(at(start, 1000)));
        assert!(first.contains(" pres=1 "), "{first}");
        assert!(first.ends_with(" rep=2"), "{first}");
        assert!(line(stats.report(at(start, 2000))).ends_with(" rep=0"));
    }

    #[test]
    fn no_line_before_a_second() {
        let start = Instant::now();
        let mut stats = Stats::new(start, true);
        assert!(stats.report(at(start, 999)).is_none());
        assert!(stats.report(at(start, 1000)).is_some());
        assert!(stats.report(at(start, 1500)).is_none());
        assert_eq!(stats.next_report(), Some(at(start, 2000)));
    }

    #[test]
    fn full_line() {
        let start = Instant::now();
        let mut stats = Stats::new(start, true);
        stats.received(4);
        stats.replaced(1);
        // Captured → present: 4, 10, 6 ms; ready → present: 1, 3, 2 ms; presents at 10, 30,
        // 47 ms → longest gap 20 ms.
        stats.presented(at(start, 6), at(start, 9), at(start, 10));
        stats.presented(at(start, 20), at(start, 27), at(start, 30));
        stats.presented(at(start, 41), at(start, 45), at(start, 47));
        stats.ui_image(0.5);
        stats.ui_image(1.5);
        stats.window_moved(3);
        let report = stats.report(at(start, 1000)).unwrap();
        assert_eq!(
            report.summary,
            Summary {
                presented: 3,
                latency_ms: Some(6.0)
            }
        );
        assert_eq!(
            report.line.unwrap(),
            "[loop] t=1 recv=4 pres=3 repl=1 ready_ms=2.0/3.0 gap_ms=20.0 ui=2 ui_ms=1.0/1.5 moves=3 lat_ms=6.0/10.0 rep=0"
        );
    }

    #[test]
    fn median_of_even_and_odd_counts() {
        assert_eq!(median_max(&mut [4.0, 1.0, 3.0, 2.0]), Some((2.5, 4.0)));
        assert_eq!(median_max(&mut [5.0, 1.0, 9.0]), Some((5.0, 9.0)));
        assert_eq!(median_max(&mut []), None);
        assert_eq!(median_max_text(Some((0.25, 0.25))), "0.2/0.2");
        assert_eq!(median_max_text(None), "-/-");
    }

    #[test]
    fn gap_spans_seconds_and_counters_reset() {
        let start = Instant::now();
        let mut stats = Stats::new(start, true);
        stats.received(1);
        stats.presented(at(start, 980), at(start, 990), at(start, 990));
        stats.report(at(start, 1000)).unwrap();
        stats.presented(at(start, 1037), at(start, 1040), at(start, 1040));
        assert_eq!(
            line(stats.report(at(start, 2000))),
            "[loop] t=2 recv=0 pres=1 repl=0 ready_ms=0.0/0.0 gap_ms=50.0 ui=0 ui_ms=-/- moves=0 lat_ms=3.0/3.0 rep=0"
        );
    }

    #[test]
    fn without_stats_only_the_summary_is_kept() {
        let start = Instant::now();
        let mut stats = Stats::new(start, false);
        // Idle: no report is wanted.
        assert_eq!(stats.next_report(), None);
        stats.received(2);
        stats.presented(at(start, 2), at(start, 8), at(start, 10));
        stats.presented(at(start, 20), at(start, 22), at(start, 24));
        assert_eq!(stats.next_report(), Some(at(start, 1000)));
        // Nothing recorded for the line.
        assert!(stats.ready_ms.is_empty() && stats.last_present.is_none());
        let report = stats.report(at(start, 1000)).unwrap();
        assert_eq!(report.line, None);
        assert_eq!(
            report.summary,
            Summary {
                presented: 2,
                latency_ms: Some(6.0)
            }
        );
        // One more report tells the frames stopped, then none is wanted.
        assert_eq!(stats.next_report(), Some(at(start, 2000)));
        assert_eq!(stats.report(at(start, 2000)).unwrap().summary, Summary::EMPTY);
        assert_eq!(stats.next_report(), None);
    }
}
