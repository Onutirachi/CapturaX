//! When a Windows output reopens to clear a delay that grows after the app (spec FR-003a, research
//! R16): at the first quiet moment once the stream has played for 5 minutes. A palliative until
//! the cause is known.

/// The stream plays this long before a quiet moment reopens it, in seconds.
const AFTER_SECONDS: u64 = 5 * 60;
/// A quiet moment lasts this long, in ms.
const QUIET_MS: u64 = 300;
/// Quiet: every sample's magnitude below this (−50 dBFS).
const QUIET_PEAK: f32 = 0.003_162_278;

/// Fed with each block the output writes; says when to reopen.
#[derive(Debug, Clone)]
pub struct Resync {
    after: u64,
    quiet_needed: u64,
    played: u64,
    quiet: u64,
}

impl Resync {
    /// A new count for a stream at `rate` frames per second, just opened.
    pub fn new(rate: u32) -> Self {
        let rate = u64::from(rate);
        Self {
            after: AFTER_SECONDS * rate,
            quiet_needed: QUIET_MS * rate / 1000,
            played: 0,
            quiet: 0,
        }
    }

    /// Counts a block of interleaved samples about to play; true when the stream should reopen
    /// now (5 minutes played and the last 300 ms quiet).
    pub fn feed(&mut self, samples: &[f32], channels: usize) -> bool {
        let frames = (samples.len() / channels.max(1)) as u64;
        self.played += frames;
        if samples.iter().all(|s| s.abs() < QUIET_PEAK) {
            self.quiet += frames;
        } else {
            self.quiet = 0;
        }
        self.played >= self.after && self.quiet >= self.quiet_needed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 48_000;
    /// 2.67 ms of stereo: the shared period measured on Windows.
    const BLOCK: usize = 128;

    fn block(level: f32) -> Vec<f32> {
        vec![level; BLOCK * 2]
    }

    /// Feeds `seconds` of blocks at `level`; the number of blocks that said to reopen.
    fn run(resync: &mut Resync, level: f32, seconds: f64) -> usize {
        let blocks = (seconds * f64::from(RATE) / BLOCK as f64).round() as usize;
        let data = block(level);
        (0..blocks).filter(|_| resync.feed(&data, 2)).count()
    }

    #[test]
    fn silence_before_five_minutes_does_not_reopen() {
        let mut resync = Resync::new(RATE);
        assert_eq!(run(&mut resync, 0.0, 299.0), 0);
    }

    #[test]
    fn quiet_after_five_minutes_reopens_after_300_ms() {
        let mut resync = Resync::new(RATE);
        assert_eq!(run(&mut resync, 0.5, 300.0), 0);
        assert_eq!(run(&mut resync, 0.0, 0.29), 0);
        assert!(run(&mut resync, 0.0, 0.02) > 0);
    }

    #[test]
    fn a_loud_block_starts_the_quiet_again() {
        let mut resync = Resync::new(RATE);
        run(&mut resync, 0.5, 300.0);
        run(&mut resync, 0.0, 0.25);
        assert!(!resync.feed(&block(0.01), 2));
        assert_eq!(run(&mut resync, 0.0, 0.29), 0);
        assert!(run(&mut resync, 0.0, 0.02) > 0);
    }

    #[test]
    fn continuous_sound_never_reopens() {
        let mut resync = Resync::new(RATE);
        assert_eq!(run(&mut resync, 0.01, 1800.0), 0);
    }

    #[test]
    fn below_minus_50_dbfs_is_quiet() {
        let mut resync = Resync::new(RATE);
        run(&mut resync, 0.5, 300.0);
        // −52 dBFS: background noise of a silent console menu.
        assert!(run(&mut resync, 0.0025, 0.31) > 0);
    }
}
