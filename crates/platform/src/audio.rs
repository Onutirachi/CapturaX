//! Audio devices and the preview's audio streams (preview audio contracts/platform.md "Audio
//! devices" and "Audio session"): Windows through WASAPI, Linux through PipeWire.
//!
//! Every call returns at once (research R9): opening, reopening and closing are requests to the
//! session's own threads (Windows) or the `pipewire` thread (Linux), and every outcome comes back
//! as an event through the callback given at start or open. No call waits for a device, so a
//! stuck device can never freeze the window or keep the app from quitting.
//!
//! The platform layer does not know the bridge (`crates/audio`): a session hands the input's
//! samples to [`Samples::on_input`] and asks [`Samples::fill`] for the output's, both on the
//! audio threads; the app connects them to its bridge.

use std::fmt;
use std::sync::Arc;

use crate::os;

/// Which way an endpoint carries sound.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Flow {
    Input,
    Output,
}

/// How a device stores one sample. The streams always hand over `f32`; this is what the device
/// itself runs in, for the stats and the exclusive probe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SampleFormat {
    F32,
    I32,
    /// 24 bits in the high bytes of a 32-bit container.
    I24In32,
    /// 24 bits packed in 3 bytes.
    I24,
    I16,
}

impl fmt::Display for SampleFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::F32 => "f32",
            Self::I32 => "i32",
            Self::I24In32 => "i24in32",
            Self::I24 => "i24",
            Self::I16 => "i16",
        })
    }
}

/// The format a device or a stream runs in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Format {
    /// Frames per second.
    pub rate: u32,
    /// Samples per frame, interleaved.
    pub channels: u16,
    pub sample: SampleFormat,
}

impl fmt::Display for Format {
    /// `rate/channels/sample`, as the `[audio-source]` stats line writes it (contracts/stats.md).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}/{}", self.rate, self.channels, self.sample)
    }
}

/// One audio input or output of the computer (data-model.md "Audio endpoint").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoint {
    /// Stable key: the Windows endpoint id (`{0.0.1.00000000}.{guid}`), the Linux `node.name`.
    pub id: String,
    /// The name shown: [`Endpoint::base_name`], numbered " (2)", " (3)" when repeated within its
    /// flow (as video devices, device preview FR-003).
    pub name: String,
    /// The system's name: the Windows friendly name, the Linux `node.description`.
    pub base_name: String,
    pub flow: Flow,
    /// The physical device's key for pairing (research R5): the Windows container id (lowercase,
    /// no braces), the Linux USB path without the interface suffix. None when the system gives
    /// none (virtual devices).
    pub link: Option<String>,
    /// What the device runs in; None when the system does not say.
    pub format: Option<Format>,
}

/// Numbers repeated names " (2)", " (3)" in list order, separately for inputs and outputs; the
/// first keeps its name. The monitor numbers its first list; the app numbers its own list again
/// after each add or remove.
pub fn numbered(endpoints: &mut [Endpoint]) {
    for i in 0..endpoints.len() {
        let earlier = endpoints[..i]
            .iter()
            .filter(|e| e.flow == endpoints[i].flow && e.base_name == endpoints[i].base_name)
            .count();
        endpoints[i].name = if earlier == 0 {
            endpoints[i].base_name.clone()
        } else {
            format!("{} ({})", endpoints[i].base_name, earlier + 1)
        };
    }
}

/// What the audio monitor reports (research R6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AudioEvent {
    /// The endpoints present at start and the system default output; sent once, before any
    /// other event.
    Listed {
        inputs: Vec<Endpoint>,
        outputs: Vec<Endpoint>,
        default_output: Option<String>,
    },
    /// An endpoint came (plugged in, enabled).
    Added(Endpoint),
    /// The id of an endpoint that left.
    Removed(String),
    /// The system default output changed; None when there is none.
    DefaultOutput(Option<String>),
}

/// Why an audio side could not open or stopped (research R7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AudioError {
    /// The device left.
    Gone,
    /// Another app holds the device in exclusive mode (Windows).
    InUse,
    /// The system's microphone privacy switch refuses the input (Windows).
    Blocked,
    /// The device accepts no format the app can use.
    Format,
    /// Anything else, with the system's text.
    Failed(String),
}

impl fmt::Display for AudioError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Gone => f.write_str("the device is gone"),
            Self::InUse => f.write_str("another app is using the device"),
            Self::Blocked => f.write_str("the microphone privacy switch blocks the device"),
            Self::Format => f.write_str("no usable format"),
            Self::Failed(text) => f.write_str(text),
        }
    }
}

impl std::error::Error for AudioError {}

/// The running audio monitor; stops when dropped.
pub struct Monitor {
    _os: os::audio::Monitor,
}

impl Monitor {
    /// Starts watching the system's audio endpoints. `on_event` runs on the monitor's thread and
    /// must not block; [`AudioEvent::Listed`] comes first. Errs when the system's device list
    /// cannot be read at all.
    pub fn start(on_event: impl Fn(AudioEvent) + Send + Sync + 'static) -> Result<Self, AudioError> {
        Ok(Self {
            _os: os::audio::Monitor::start(Arc::new(on_event))?,
        })
    }
}

/// The audio link of a video device, for pairing (FR-007, research R5): the Windows container id
/// of the Media Foundation device (`mf:` id), the Linux USB path from the V4L2 id's bus path
/// without the interface suffix. Formatted as [`Endpoint::link`]; None when the system gives
/// none. Reads the system's device tree; does not open the device.
pub fn video_link(video_id: &str) -> Option<String> {
    os::audio::video_link(video_id)
}

/// One side of an audio session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Side {
    Input,
    Output,
}

/// How a side opened.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StreamMode {
    /// Through the system's mixer.
    Shared,
    /// The device alone (Windows).
    Exclusive,
    /// Exclusive was asked and refused (in use, or no format), so the side opened shared; not
    /// asked again until the next open.
    SharedAfterInUse,
}

impl fmt::Display for StreamMode {
    /// As the stats write it (contracts/stats.md).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Shared => "shared",
            Self::Exclusive => "exclusive",
            Self::SharedAfterInUse => "shared-in-use",
        })
    }
}

/// What an audio session opens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionRequest {
    /// Carried by every event of the session, so the app can ignore an older session's events.
    pub number: u64,
    /// The input endpoint's id.
    pub input: String,
    /// The output endpoint's id; None follows the system default output.
    pub output: Option<String>,
    /// Try the device alone first (Windows; ignored on Linux).
    pub input_exclusive: bool,
    pub output_exclusive: bool,
}

/// What an audio session reports.
#[derive(Debug, Clone, PartialEq)]
pub enum SessionEvent {
    /// A side opened (at open or after a reopen); samples flow.
    Opened {
        number: u64,
        side: Side,
        mode: StreamMode,
        /// What the device runs in.
        format: Format,
        /// The period the side opened with.
        period_ms: f64,
        /// The latency the stream reports (Windows `GetStreamLatency`, Linux the
        /// `pw_stream_get_time` delay); None when it does not say.
        reported_latency_ms: Option<f64>,
    },
    /// A side could not open or stopped; the other side keeps running.
    Error { number: u64, side: Side, error: AudioError },
    /// Both sides closed and released their devices, after [`Session::close`].
    Closed { number: u64 },
}

/// Where a session's samples go and come from; called on the audio threads, so each call must
/// return quickly and never block.
pub trait Samples: Send + Sync {
    /// The input's samples, `f32` interleaved in `format`'s rate and channels.
    fn on_input(&self, samples: &[f32], format: Format);
    /// Fills the output's samples, `f32` interleaved in `format`'s rate and channels; silence is
    /// zeros.
    fn fill(&self, out: &mut [f32], format: Format);
}

/// A running audio session: an input and an output (research R2, R3). Closes when dropped.
pub struct Session {
    os: os::audio::Session,
    closed: bool,
}

impl Session {
    /// Asks for both sides to open and returns at once; each side reports `Opened` or `Error`
    /// through `on_event`, from the audio threads.
    pub fn open(
        request: SessionRequest,
        samples: Arc<dyn Samples>,
        on_event: impl Fn(SessionEvent) + Send + Sync + 'static,
    ) -> Self {
        Self {
            os: os::audio::Session::open(request, samples, Arc::new(on_event)),
            closed: false,
        }
    }

    /// Closes the input and opens `input` in its place; reports as at open.
    pub fn reopen_input(&self, input: String, exclusive: bool) {
        self.os.reopen_input(input, exclusive);
    }

    /// Closes the output and opens `output` (None: the system default) in its place; reports as
    /// at open.
    pub fn reopen_output(&self, output: Option<String>, exclusive: bool) {
        self.os.reopen_output(output, exclusive);
    }

    /// Asks both sides to stop and release their devices; `Closed` follows. Returns at once.
    pub fn close(&mut self) {
        if !self.closed {
            self.closed = true;
            self.os.close();
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.close();
    }
}

/// The event callback shared by a session's threads.
pub(crate) type OnSessionEvent = Arc<dyn Fn(SessionEvent) + Send + Sync>;
/// The monitor's event callback.
pub(crate) type OnAudioEvent = Arc<dyn Fn(AudioEvent) + Send + Sync>;

#[cfg(test)]
mod tests {
    use super::*;

    fn endpoint(id: &str, name: &str, flow: Flow) -> Endpoint {
        Endpoint {
            id: id.into(),
            name: String::new(),
            base_name: name.into(),
            flow,
            link: None,
            format: None,
        }
    }

    #[test]
    fn repeated_names_are_numbered_per_flow() {
        let mut endpoints = vec![
            endpoint("a", "USB Audio", Flow::Input),
            endpoint("b", "USB Audio", Flow::Output),
            endpoint("c", "USB Audio", Flow::Input),
            endpoint("d", "Speakers", Flow::Output),
            endpoint("e", "USB Audio", Flow::Input),
        ];
        numbered(&mut endpoints);
        let names: Vec<_> = endpoints.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(
            names,
            ["USB Audio", "USB Audio", "USB Audio (2)", "Speakers", "USB Audio (3)"]
        );
        // After the first leaves, the next one takes the plain name.
        endpoints.remove(0);
        numbered(&mut endpoints);
        assert_eq!(endpoints[1].name, "USB Audio");
        assert_eq!(endpoints[3].name, "USB Audio (2)");
    }

    #[test]
    fn formats_and_modes_print_as_the_stats_write_them() {
        let format = Format {
            rate: 48000,
            channels: 2,
            sample: SampleFormat::I24In32,
        };
        assert_eq!(format.to_string(), "48000/2/i24in32");
        assert_eq!(StreamMode::SharedAfterInUse.to_string(), "shared-in-use");
        assert_eq!(StreamMode::Exclusive.to_string(), "exclusive");
    }
}
