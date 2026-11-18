//! The preview's sound: which audio session is open and what the Audio dropdown shows
//! (data-model.md "Audio session", "Audio state", "Audio choice per video device", "Output
//! choice"). Pure, as `preview.rs`: monitor events, session events, the user's picks and the
//! video session's open and close go in, commands for the app come out. No platform call, no
//! settings write: the choices are read from the settings, which the app writes before it sends
//! the pick.

use std::collections::HashMap;

use ::audio::choice::{self, AudioChoice, OutputChoice, Resolved};
use ::audio::pairing::{self, VideoKey};
use ::audio::volume::VolumeIcon;
use platform::audio::{AudioEvent, Endpoint, Flow, SessionEvent, SessionRequest, Side};
use settings::Settings;

/// What the Audio dropdown shows (data-model.md "Audio state"); never shown while no video device
/// is open (FR-018).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AudioState {
    /// No video device open, or nothing to play (No audio).
    Off,
    /// The session was asked for (or waits for the audio list); no sound yet.
    Opening,
    /// Both sides run.
    Playing,
    /// Auto and no input pairs with the video device.
    NoPair,
    /// The chosen input is missing, by its last known name; it opens when it returns.
    Disconnected(String),
}

/// The video device whose sound is wanted, as the app read it when its session opened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoDevice {
    pub id: String,
    /// The system's name, without the app's " (2)" numbering.
    pub base_name: String,
    /// `platform::audio::video_link` of the device; None when the system gives none.
    pub link: Option<String>,
}

/// What goes into [`AudioController::handle`].
#[derive(Debug, Clone)]
pub enum Event {
    Monitor(AudioEvent),
    Session(SessionEvent),
    /// A video session started (preview states Opening, Playing, No signal follow it).
    VideoOpened {
        session: u64,
        device: VideoDevice,
    },
    /// The video session ended.
    VideoClosed {
        session: u64,
    },
    /// The user set the preview's volume (0–100) or mute (FR-011); the settings already hold it.
    Volume {
        level: u8,
        muted: bool,
    },
    /// The user picked the open video device's input (FR-008); the settings already hold it.
    Input(AudioChoice),
    /// The user picked the output (FR-009); the settings already hold it.
    Output(OutputChoice),
}

impl Event {
    /// A short description for the log line of a state change (the event's reason).
    pub fn reason(&self) -> String {
        match self {
            Self::Monitor(AudioEvent::Listed { inputs, outputs, .. }) => {
                format!("audio list, {} inputs, {} outputs", inputs.len(), outputs.len())
            }
            Self::Monitor(AudioEvent::Added(endpoint)) => format!("{} added", endpoint.id),
            Self::Monitor(AudioEvent::Removed(id)) => format!("{id} removed"),
            Self::Monitor(AudioEvent::DefaultOutput(id)) => format!("default output {id:?}"),
            Self::Session(event) => format!("audio session: {event:?}"),
            Self::VideoOpened { session, device } => format!("video session {session} opened on {}", device.id),
            Self::VideoClosed { session } => format!("video session {session} closed"),
            Self::Volume { level, muted } => format!("volume {level} muted {muted}"),
            Self::Input(choice) => format!("input {choice} picked"),
            Self::Output(choice) => format!("output {choice} picked"),
        }
    }
}

/// What the app does for the preview's sound. Every command returns at once (research R9).
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    /// Open this session; a session still open is closed first by [`Command::Close`].
    Open(SessionRequest),
    /// Reopen the open session's input on this endpoint (the bridge restarts empty).
    #[expect(dead_code, reason = "emitted by the exclusive flag (T038)")]
    ReopenInput { input: String, exclusive: bool },
    /// Reopen the open session's output (None: the system default).
    ReopenOutput { output: Option<String>, exclusive: bool },
    /// Close the open session.
    Close,
    /// Set the open bridge's gain (`audio::volume::gain`), at once and without a reopen.
    SetGain(f32),
}

/// One row of the Audio dropdown's menu. Texts that need a catalog are named, not written, as
/// `preview::Item`'s.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub text: Label,
    /// The choice in use.
    pub current: bool,
    /// What picking the row does; None for headers.
    pub pick: Option<Pick>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Label {
    /// The header of the inputs, `Input`.
    Inputs,
    /// The header of the outputs, `Output`.
    Outputs,
    /// `Auto ({})` with the paired input's name, `Auto (none)` without.
    Auto(Option<String>),
    /// `No audio`.
    NoAudio,
    /// `System default ({})` with the default output's name; None when the system has none.
    SystemDefault(Option<String>),
    /// An endpoint's (numbered) name, or the id of a chosen one never seen.
    Plain(String),
}

impl Label {
    pub fn is_header(&self) -> bool {
        matches!(self, Self::Inputs | Self::Outputs)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pick {
    Input(AudioChoice),
    Output(OutputChoice),
}

/// The volume button's icon in `main.slint` (`volume-icon`) for a level and mute.
pub fn icon_name(level: u8, muted: bool) -> &'static str {
    match ::audio::volume::icon(level, muted) {
        VolumeIcon::Off => "off",
        VolumeIcon::Low => "low",
        VolumeIcon::Medium => "medium",
        VolumeIcon::High => "high",
    }
}

/// Logical pixels of wheel move per notch, as Slint counts a mouse wheel line.
const WHEEL_NOTCH: f32 = 60.0;

/// Turns the wheel's moves over the volume button into notches, keeping the remainder so a
/// touchpad's small moves add up.
#[derive(Debug, Default)]
pub struct Wheel {
    rest: f32,
}

impl Wheel {
    /// Whole notches in this move plus what the last ones left; positive is up (louder).
    pub fn notches(&mut self, delta: f32) -> i32 {
        // A turn the other way drops what the last direction left.
        if delta * self.rest < 0.0 {
            self.rest = 0.0;
        }
        self.rest += delta / WHEEL_NOTCH;
        let notches = self.rest.trunc();
        self.rest -= notches;
        notches as i32
    }
}

/// The audio session asked for.
#[derive(Debug, Clone)]
struct Open {
    number: u64,
    /// The input asked for, as the last open said.
    input_id: String,
    /// The output asked for, as the last open or reopen said; None: the system default.
    output_id: Option<String>,
    /// The sides that reported `Opened` since their last open.
    input: bool,
    output: bool,
    /// The output side failed while playing on the system default: it reopens when
    /// `DefaultOutput` names an output (Linux after PipeWire restarts, research R7).
    output_failed: bool,
}

/// The open video device's choice, the output choice: [`choice`]'s types read from the settings.
fn input_choice(settings: &Settings, video: &VideoDevice) -> AudioChoice {
    settings
        .device(&video.id)
        .map_or(AudioChoice::Auto, |c| AudioChoice::parse(&c.audio))
}

fn output_choice(settings: &Settings) -> OutputChoice {
    OutputChoice::parse(&settings.audio.output)
}

#[derive(Debug)]
pub struct AudioController {
    state: Option<AudioState>,
    /// The monitor's first list arrived.
    listed: bool,
    inputs: Vec<Endpoint>,
    outputs: Vec<Endpoint>,
    default_output: Option<String>,
    /// The shown name of every endpoint seen since start, by id: a chosen endpoint that left is
    /// still named.
    names: HashMap<String, String>,
    /// The open video session and its device.
    video: Option<(u64, VideoDevice)>,
    open: Option<Open>,
    /// The chosen output plays on the system default because it is missing or failed
    /// (data-model.md "Output fallback"): its id, until the output opens on it again.
    fallback: Option<String>,
    /// A `DefaultOutput` change reopens an output that follows the default: Windows; on Linux
    /// PipeWire moves the stream itself (contracts/platform.md).
    reopens_on_default: bool,
    /// Audio session numbers handed out so far.
    sessions: u64,
}

impl Default for AudioController {
    fn default() -> Self {
        Self::with_default_reopen(platform::IS_WINDOWS)
    }
}

impl AudioController {
    pub fn new() -> Self {
        Self::default()
    }

    /// `reopens_on_default`: the system leaves an output on the old default (Windows).
    fn with_default_reopen(reopens_on_default: bool) -> Self {
        Self {
            state: None,
            listed: false,
            inputs: Vec::new(),
            outputs: Vec::new(),
            default_output: None,
            names: HashMap::new(),
            video: None,
            open: None,
            fallback: None,
            reopens_on_default,
            sessions: 0,
        }
    }

    /// What the Audio dropdown shows.
    pub fn state(&self) -> AudioState {
        self.state.clone().unwrap_or(AudioState::Off)
    }

    /// The number of the audio session open or opening; events and samples of others are dropped.
    pub fn session(&self) -> Option<u64> {
        self.open.as_ref().map(|o| o.number)
    }

    /// The open video device, whose input choice the Audio select shows and a pick stores.
    pub fn video(&self) -> Option<&VideoDevice> {
        self.video.as_ref().map(|(_, video)| video)
    }

    /// The system default output's id, as the monitor last said.
    #[cfg(test)]
    pub fn default_output(&self) -> Option<&str> {
        self.default_output.as_deref()
    }

    /// The shown name of a listed endpoint; `output` None names the system default output.
    pub fn endpoint_name(&self, id: Option<&str>) -> Option<&str> {
        let id = id.or(self.default_output.as_deref())?;
        self.inputs
            .iter()
            .chain(&self.outputs)
            .find(|e| e.id == id)
            .map(|e| e.name.as_str())
    }

    /// The name of an endpoint seen since start, else its id.
    fn name_of(&self, id: &str) -> String {
        self.names.get(id).cloned().unwrap_or_else(|| id.to_string())
    }

    /// The chosen output's name while the sound plays on the system default instead
    /// (`{} disconnected` / `Playing on the system default`); None otherwise.
    #[cfg_attr(not(test), expect(dead_code, reason = "shown by the Audio select (T041)"))]
    pub fn fallback(&self) -> Option<String> {
        self.open.as_ref()?;
        self.fallback.as_deref().map(|id| self.name_of(id))
    }

    /// The input paired with the open video device, among the present inputs.
    fn pair(&self, video: &VideoDevice) -> Option<String> {
        let endpoints: Vec<pairing::Endpoint> = self
            .inputs
            .iter()
            .map(|e| pairing::Endpoint {
                id: e.id.clone(),
                name: e.base_name.clone(),
                link: e.link.clone(),
            })
            .collect();
        let key = VideoKey {
            link: video.link.clone(),
            name: video.base_name.clone(),
        };
        pairing::pair(&key, &endpoints).map(str::to_string)
    }

    /// What the open video device's choice gives with the inputs present now.
    fn resolved(&self, video: &VideoDevice, settings: &Settings) -> Resolved {
        let pair = self.pair(video);
        let ids: Vec<&str> = self.inputs.iter().map(|e| e.id.as_str()).collect();
        choice::resolve(&input_choice(settings, video), pair.as_deref(), &ids)
    }

    /// The output to open for the output choice and the chosen output when it is missing.
    fn output_to_open(&self, settings: &Settings) -> (Option<String>, Option<String>) {
        let choice = output_choice(settings);
        let ids: Vec<&str> = self.outputs.iter().map(|e| e.id.as_str()).collect();
        let (output, fallback) = choice::output_for(&choice, &ids);
        let missing = match choice {
            OutputChoice::Device(id) if fallback => Some(id),
            _ => None,
        };
        (output, missing)
    }

    /// The Audio select's value: the open video device's input choice (FR-008); None while no
    /// video device is open.
    pub fn value(&self, settings: &Settings) -> Option<Label> {
        let (_, video) = self.video.as_ref()?;
        Some(match input_choice(settings, video) {
            AudioChoice::Auto => Label::Auto(self.pair(video).map(|id| self.name_of(&id))),
            AudioChoice::None => Label::NoAudio,
            AudioChoice::Input(id) => Label::Plain(self.name_of(&id)),
        })
    }

    /// The Audio dropdown's menu: the `Input` group (Auto, every input, No audio) while a video
    /// device is open, then the `Output` group; the choices in use marked. A chosen endpoint that
    /// is missing is not offered and nothing of its group is marked (the state says why).
    pub fn menu(&self, settings: &Settings) -> Vec<Item> {
        let header = |text| Item {
            text,
            current: false,
            pick: None,
        };
        let mut items = Vec::new();
        if let Some((_, video)) = &self.video {
            let choice = input_choice(settings, video);
            items.push(header(Label::Inputs));
            items.push(Item {
                text: Label::Auto(self.pair(video).map(|id| self.name_of(&id))),
                current: choice == AudioChoice::Auto,
                pick: Some(Pick::Input(AudioChoice::Auto)),
            });
            for input in &self.inputs {
                let pick = AudioChoice::Input(input.id.clone());
                items.push(Item {
                    text: Label::Plain(input.name.clone()),
                    current: choice == pick,
                    pick: Some(Pick::Input(pick)),
                });
            }
            items.push(Item {
                text: Label::NoAudio,
                current: choice == AudioChoice::None,
                pick: Some(Pick::Input(AudioChoice::None)),
            });
        }
        items.push(header(Label::Outputs));
        items.extend(self.output_items(settings));
        items
    }

    /// The output choices (System default, every output), the one in use marked: the `Output`
    /// group of the menu.
    fn output_items(&self, settings: &Settings) -> Vec<Item> {
        let choice = output_choice(settings);
        let default_name = self.default_output.as_deref().map(|id| self.name_of(id));
        std::iter::once(Item {
            text: Label::SystemDefault(default_name),
            current: choice == OutputChoice::Default,
            pick: Some(Pick::Output(OutputChoice::Default)),
        })
        .chain(self.outputs.iter().map(|output| {
            let pick = OutputChoice::Device(output.id.clone());
            Item {
                text: Label::Plain(output.name.clone()),
                current: choice == pick,
                pick: Some(Pick::Output(pick)),
            }
        }))
        .collect()
    }

    pub fn handle(&mut self, event: Event, settings: &Settings) -> Vec<Command> {
        let mut out = Vec::new();
        match event {
            Event::Monitor(AudioEvent::Listed {
                inputs,
                outputs,
                default_output,
            }) => {
                self.inputs = inputs;
                self.outputs = outputs;
                self.default_output = default_output;
                for endpoint in self.inputs.iter().chain(&self.outputs) {
                    self.names.insert(endpoint.id.clone(), endpoint.name.clone());
                }
                self.listed = true;
                if self.open.is_none() {
                    self.resolve(settings, &mut out);
                }
            }
            Event::Monitor(AudioEvent::Added(endpoint)) => self.added(endpoint, settings, &mut out),
            Event::Monitor(AudioEvent::Removed(id)) => {
                // The open side's own `Error` says what happens to the sound.
                self.inputs.retain(|e| e.id != id);
                self.outputs.retain(|e| e.id != id);
                platform::audio::numbered(&mut self.inputs);
                platform::audio::numbered(&mut self.outputs);
            }
            Event::Monitor(AudioEvent::DefaultOutput(id)) => self.default_changed(id, &mut out),
            Event::Session(event) => self.session_event(event, &mut out),
            Event::VideoOpened { session, device } => {
                self.close(&mut out);
                self.video = Some((session, device));
                self.resolve(settings, &mut out);
            }
            Event::VideoClosed { session } => {
                if self.video.as_ref().is_some_and(|(open, _)| *open == session) {
                    self.video = None;
                    self.close(&mut out);
                    self.state = Some(AudioState::Off);
                }
            }
            // A session opened later takes its gain from the settings.
            Event::Volume { level, muted } => {
                if self.open.is_some() {
                    out.push(Command::SetGain(::audio::volume::gain(level, muted)));
                }
            }
            Event::Input(_) => self.input_picked(settings, &mut out),
            Event::Output(_) => self.output_picked(settings, &mut out),
        }
        out
    }

    /// Opens the video device's sound, or shows why there is none.
    fn resolve(&mut self, settings: &Settings, out: &mut Vec<Command>) {
        let Some((_, video)) = &self.video else {
            self.state = Some(AudioState::Off);
            return;
        };
        if !self.listed {
            // The first list resolves it.
            self.state = Some(AudioState::Opening);
            return;
        }
        match self.resolved(video, settings) {
            Resolved::Play(input) => self.open_session(input, settings, out),
            Resolved::NoPair => self.state = Some(AudioState::NoPair),
            Resolved::Missing(id) => self.state = Some(AudioState::Disconnected(self.name_of(&id))),
            Resolved::Off => self.state = Some(AudioState::Off),
        }
    }

    /// Opens a session on `input` and the output of the output choice (the default, with the
    /// fallback, when the chosen one is missing).
    fn open_session(&mut self, input: String, settings: &Settings, out: &mut Vec<Command>) {
        let (output, missing) = self.output_to_open(settings);
        self.fallback = missing;
        self.sessions += 1;
        let number = self.sessions;
        self.open = Some(Open {
            number,
            input_id: input.clone(),
            output_id: output.clone(),
            input: false,
            output: false,
            output_failed: false,
        });
        self.state = Some(AudioState::Opening);
        out.push(Command::Open(SessionRequest {
            number,
            input,
            output,
            input_exclusive: false,
            output_exclusive: false,
        }));
    }

    /// Closes the open audio session, if any.
    fn close(&mut self, out: &mut Vec<Command>) {
        if self.open.take().is_some() {
            out.push(Command::Close);
        }
        self.fallback = None;
    }

    /// Reopens the open session's output on `output` (None: the system default).
    fn reopen_output(&mut self, output: Option<String>, out: &mut Vec<Command>) {
        if let Some(open) = &mut self.open {
            open.output_id.clone_from(&output);
            open.output = false;
            open.output_failed = false;
            out.push(Command::ReopenOutput {
                output,
                exclusive: false,
            });
        }
    }

    /// An endpoint came: kept in its list; a missing chosen input or output it is opens.
    fn added(&mut self, endpoint: Endpoint, settings: &Settings, out: &mut Vec<Command>) {
        let id = endpoint.id.clone();
        let flow = endpoint.flow;
        let list = match flow {
            Flow::Input => &mut self.inputs,
            Flow::Output => &mut self.outputs,
        };
        list.retain(|e| e.id != id);
        list.push(endpoint);
        platform::audio::numbered(list);
        for endpoint in self.inputs.iter().chain(&self.outputs) {
            self.names.insert(endpoint.id.clone(), endpoint.name.clone());
        }
        match flow {
            // An input that pairs, or the chosen one, may have come.
            Flow::Input => {
                if self.open.is_none() && matches!(self.state, Some(AudioState::NoPair | AudioState::Disconnected(_))) {
                    self.resolve(settings, out);
                }
            }
            // The chosen output returned: the sound moves back to it.
            Flow::Output => {
                if self.fallback.as_deref() == Some(id.as_str())
                    && self.open.as_ref().is_some_and(|o| o.output_id.is_none())
                {
                    self.reopen_output(Some(id), out);
                }
            }
        }
    }

    /// The system default output changed: an output on the default reopens on Windows, and on
    /// both systems when it failed (research R7).
    fn default_changed(&mut self, id: Option<String>, out: &mut Vec<Command>) {
        let changed = self.default_output != id;
        self.default_output = id;
        let Some(open) = &self.open else { return };
        if open.output_id.is_none()
            && self.default_output.is_some()
            && (open.output_failed || (changed && self.reopens_on_default))
        {
            self.reopen_output(None, out);
        }
    }

    /// The user picked the open video device's input: a session on another input opens (both
    /// sides, the bridge empty); the same input keeps playing.
    fn input_picked(&mut self, settings: &Settings, out: &mut Vec<Command>) {
        let Some((_, video)) = &self.video else { return };
        if self.listed
            && let (Some(open), Resolved::Play(input)) = (&self.open, self.resolved(video, settings))
            && open.input_id == input
        {
            return;
        }
        self.close(out);
        self.resolve(settings, out);
    }

    /// The user picked the output: only the output side reopens, when it plays elsewhere.
    fn output_picked(&mut self, settings: &Settings, out: &mut Vec<Command>) {
        if self.open.is_none() {
            return;
        }
        let (output, missing) = self.output_to_open(settings);
        self.fallback = missing;
        if self.open.as_ref().is_some_and(|o| o.output_id != output) {
            self.reopen_output(output, out);
        }
    }

    fn session_event(&mut self, event: SessionEvent, out: &mut Vec<Command>) {
        let number = match &event {
            SessionEvent::Opened { number, .. }
            | SessionEvent::Error { number, .. }
            | SessionEvent::Closed { number } => *number,
        };
        let Some(open) = self.open.as_mut().filter(|o| o.number == number) else {
            // An older session's: no sound plays after its video is gone (research R9).
            return;
        };
        match event {
            SessionEvent::Opened { side, .. } => {
                match side {
                    Side::Input => open.input = true,
                    Side::Output => {
                        open.output = true;
                        // The chosen output plays again: no fallback.
                        if open.output_id.is_some() {
                            self.fallback = None;
                        }
                    }
                }
                if open.input && open.output {
                    self.state = Some(AudioState::Playing);
                }
            }
            SessionEvent::Error {
                side: Side::Output,
                error,
                ..
            } => {
                log::warn!("audio session {number}: output side: {error}");
                match open.output_id.take() {
                    // The chosen output is gone or failed: the default plays, and says so.
                    Some(id) => {
                        self.fallback = Some(id);
                        self.reopen_output(None, out);
                    }
                    // Waits for the default output to return (or Try again, T041).
                    None => open.output_failed = true,
                }
            }
            SessionEvent::Error { side, error, .. } => {
                // The input's error states come with T040.
                log::warn!("audio session {number}: {side:?} side: {error}");
            }
            SessionEvent::Closed { .. } => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use platform::audio::{AudioError, Flow, Format, SampleFormat, StreamMode};

    use super::*;

    const CARD_VIDEO: &str = "mf:ezcap";
    const CARD_INPUT: &str = "{0.0.1.00000000}.{card}";
    const MIC_INPUT: &str = "{0.0.1.00000000}.{mic}";
    const SPEAKERS: &str = "{0.0.0.00000000}.{speakers}";
    const CARD_LINK: &str = "6c1b7c3a-0d4e-4a7f-9b1e-2f3c4d5e6f70";

    fn endpoint(id: &str, name: &str, flow: Flow, link: Option<&str>) -> Endpoint {
        Endpoint {
            id: id.into(),
            name: name.into(),
            base_name: name.into(),
            flow,
            link: link.map(Into::into),
            format: None,
        }
    }

    fn listed() -> Event {
        Event::Monitor(AudioEvent::Listed {
            inputs: vec![
                endpoint(MIC_INPUT, "Microphone (Realtek(R) Audio)", Flow::Input, None),
                endpoint(
                    CARD_INPUT,
                    "Digital Audio Interface (ezcap)",
                    Flow::Input,
                    Some(CARD_LINK),
                ),
            ],
            outputs: vec![endpoint(SPEAKERS, "Speakers (Realtek(R) Audio)", Flow::Output, None)],
            default_output: Some(SPEAKERS.into()),
        })
    }

    fn card() -> VideoDevice {
        VideoDevice {
            id: CARD_VIDEO.into(),
            base_name: "ezcap Game Link".into(),
            link: Some(CARD_LINK.into()),
        }
    }

    /// A webcam with no link and no input of its name.
    fn lonely_camera() -> VideoDevice {
        VideoDevice {
            id: "mf:camera".into(),
            base_name: "NVIDIA Broadcast".into(),
            link: None,
        }
    }

    fn opened(number: u64, side: Side) -> Event {
        Event::Session(SessionEvent::Opened {
            number,
            side,
            mode: StreamMode::Shared,
            format: Format {
                rate: 48000,
                channels: 2,
                sample: SampleFormat::I16,
            },
            period_ms: 10.0,
            reported_latency_ms: None,
        })
    }

    fn request(number: u64, input: &str) -> Command {
        Command::Open(SessionRequest {
            number,
            input: input.into(),
            output: None,
            input_exclusive: false,
            output_exclusive: false,
        })
    }

    struct Run {
        audio: AudioController,
        settings: Settings,
    }

    impl Run {
        fn new() -> Self {
            Self {
                audio: AudioController::new(),
                settings: Settings::default(),
            }
        }

        fn send(&mut self, event: Event) -> Vec<Command> {
            self.audio.handle(event, &self.settings)
        }
    }

    #[test]
    fn opens_with_the_video_and_closes_with_it() {
        let mut run = Run::new();
        assert!(run.send(listed()).is_empty());
        assert_eq!(run.audio.state(), AudioState::Off);

        let commands = run.send(Event::VideoOpened {
            session: 1,
            device: card(),
        });
        assert_eq!(commands, [request(1, CARD_INPUT)]);
        assert_eq!(run.audio.state(), AudioState::Opening);
        assert_eq!(run.audio.session(), Some(1));

        assert!(run.send(opened(1, Side::Output)).is_empty());
        assert_eq!(run.audio.state(), AudioState::Opening, "one side is not enough");
        run.send(opened(1, Side::Input));
        assert_eq!(run.audio.state(), AudioState::Playing);

        assert_eq!(run.send(Event::VideoClosed { session: 1 }), [Command::Close]);
        assert_eq!(run.audio.state(), AudioState::Off);
        assert_eq!(run.audio.session(), None);
        // Its acknowledgment changes nothing.
        assert!(run.send(Event::Session(SessionEvent::Closed { number: 1 })).is_empty());
        assert_eq!(run.audio.state(), AudioState::Off);
    }

    #[test]
    fn wheel_counts_notches_and_keeps_the_rest() {
        let mut wheel = Wheel::default();
        assert_eq!(wheel.notches(60.0), 1);
        assert_eq!(wheel.notches(-120.0), -2);
        // A touchpad: small moves add up to a notch.
        assert_eq!(wheel.notches(25.0), 0);
        assert_eq!(wheel.notches(25.0), 0);
        assert_eq!(wheel.notches(25.0), 1);
        // Turning back drops the rest of the other way.
        assert_eq!(wheel.notches(-50.0), 0);
        assert_eq!(wheel.notches(-20.0), -1);
    }

    #[test]
    fn icon_names_follow_the_level() {
        assert_eq!(icon_name(80, true), "off");
        assert_eq!(icon_name(0, false), "off");
        assert_eq!(icon_name(20, false), "low");
        assert_eq!(icon_name(50, false), "medium");
        assert_eq!(icon_name(80, false), "high");
    }

    #[test]
    fn the_volume_reaches_only_an_open_session() {
        let mut run = Run::new();
        run.send(listed());
        let volume = |level, muted| Event::Volume { level, muted };
        assert!(run.send(volume(50, false)).is_empty(), "no session");
        run.send(Event::VideoOpened {
            session: 1,
            device: card(),
        });
        // Opening already: the bridge takes it as soon as it is made.
        assert_eq!(run.send(volume(50, false)), [Command::SetGain(0.125)]);
        assert_eq!(run.send(volume(50, true)), [Command::SetGain(0.0)]);
        assert_eq!(run.audio.state(), AudioState::Opening, "no reopen");
        run.send(Event::VideoClosed { session: 1 });
        assert!(run.send(volume(80, false)).is_empty());
    }

    #[test]
    fn a_video_opened_before_the_audio_list_waits_for_it() {
        let mut run = Run::new();
        assert!(
            run.send(Event::VideoOpened {
                session: 1,
                device: card(),
            })
            .is_empty()
        );
        assert_eq!(run.audio.state(), AudioState::Opening);
        assert_eq!(run.send(listed()), [request(1, CARD_INPUT)]);
    }

    #[test]
    fn a_video_switch_while_the_audio_opens_closes_it_and_opens_the_next() {
        let mut run = Run::new();
        run.send(listed());
        run.send(Event::VideoOpened {
            session: 1,
            device: card(),
        });
        // The preview closes the old video session and opens the next one.
        assert_eq!(run.send(Event::VideoClosed { session: 1 }), [Command::Close]);
        assert_eq!(
            run.send(Event::VideoOpened {
                session: 2,
                device: card(),
            }),
            [request(2, CARD_INPUT)]
        );
        // The first session's late events are ignored.
        run.send(opened(1, Side::Input));
        run.send(opened(1, Side::Output));
        assert_eq!(run.audio.state(), AudioState::Opening);
        run.send(opened(2, Side::Input));
        run.send(opened(2, Side::Output));
        assert_eq!(run.audio.state(), AudioState::Playing);
    }

    #[test]
    fn a_video_opened_without_a_close_replaces_the_session() {
        let mut run = Run::new();
        run.send(listed());
        run.send(Event::VideoOpened {
            session: 1,
            device: card(),
        });
        let commands = run.send(Event::VideoOpened {
            session: 2,
            device: card(),
        });
        assert_eq!(commands, [Command::Close, request(2, CARD_INPUT)]);
        // The old video session's close comes late: the new one stays.
        assert!(run.send(Event::VideoClosed { session: 1 }).is_empty());
        assert_eq!(run.audio.session(), Some(2));
    }

    #[test]
    fn no_paired_input_opens_nothing_until_one_comes() {
        let mut run = Run::new();
        run.send(listed());
        let camera = lonely_camera();
        assert!(
            run.send(Event::VideoOpened {
                session: 1,
                device: camera.clone(),
            })
            .is_empty()
        );
        assert_eq!(run.audio.state(), AudioState::NoPair);
        // An unrelated output changes nothing; an input with the camera's name pairs.
        assert!(
            run.send(Event::Monitor(AudioEvent::Added(endpoint(
                "{0.0.0.00000000}.{hdmi}",
                "NVIDIA Broadcast",
                Flow::Output,
                None,
            ))))
            .is_empty()
        );
        let mic = "{0.0.1.00000000}.{broadcast}";
        let commands = run.send(Event::Monitor(AudioEvent::Added(endpoint(
            mic,
            "Microphone (NVIDIA Broadcast)",
            Flow::Input,
            None,
        ))));
        assert_eq!(commands, [request(1, mic)]);
        assert_eq!(run.audio.state(), AudioState::Opening);
        // No pair after the video closes is Off.
        run.send(Event::VideoClosed { session: 1 });
        assert_eq!(run.audio.state(), AudioState::Off);
    }

    #[test]
    fn a_late_opened_of_an_old_session_is_ignored() {
        let mut run = Run::new();
        run.send(listed());
        run.send(Event::VideoOpened {
            session: 1,
            device: card(),
        });
        run.send(Event::VideoClosed { session: 1 });
        run.send(Event::VideoOpened {
            session: 2,
            device: lonely_camera(),
        });
        assert_eq!(run.audio.state(), AudioState::NoPair);
        run.send(opened(1, Side::Input));
        run.send(opened(1, Side::Output));
        run.send(Event::Session(SessionEvent::Error {
            number: 1,
            side: Side::Input,
            error: AudioError::Gone,
        }));
        assert_eq!(run.audio.state(), AudioState::NoPair);
        assert_eq!(run.audio.session(), None);
    }

    #[test]
    fn the_monitor_keeps_its_lists_and_the_default_output() {
        let mut run = Run::new();
        run.send(listed());
        assert_eq!(run.audio.default_output(), Some(SPEAKERS));
        assert_eq!(
            run.audio.endpoint_name(Some(CARD_INPUT)),
            Some("Digital Audio Interface (ezcap)")
        );
        assert_eq!(run.audio.endpoint_name(None), Some("Speakers (Realtek(R) Audio)"));
        assert_eq!(run.audio.endpoint_name(Some("gone")), None);
        run.send(Event::Monitor(AudioEvent::DefaultOutput(None)));
        assert_eq!(run.audio.default_output(), None);
        assert_eq!(run.audio.endpoint_name(None), None);
        // The card's input leaves before its video opens: nothing pairs by link.
        run.send(Event::Monitor(AudioEvent::Removed(CARD_INPUT.into())));
        run.send(Event::VideoOpened {
            session: 1,
            device: card(),
        });
        assert_eq!(run.audio.state(), AudioState::NoPair);
    }

    const HEADSET: &str = "{0.0.0.00000000}.{headset}";

    fn headset() -> Endpoint {
        endpoint(HEADSET, "Headset (G533)", Flow::Output, None)
    }

    fn open_request(number: u64, input: &str, output: Option<&str>) -> Command {
        Command::Open(SessionRequest {
            number,
            input: input.into(),
            output: output.map(Into::into),
            input_exclusive: false,
            output_exclusive: false,
        })
    }

    fn reopen_output(output: Option<&str>) -> Command {
        Command::ReopenOutput {
            output: output.map(Into::into),
            exclusive: false,
        }
    }

    fn output_error(number: u64, error: AudioError) -> Event {
        Event::Session(SessionEvent::Error {
            number,
            side: Side::Output,
            error,
        })
    }

    impl Run {
        fn on(windows: bool) -> Self {
            Self {
                audio: AudioController::with_default_reopen(windows),
                settings: Settings::default(),
            }
        }

        /// The card's video playing with both sides open, as session 1; the headset is listed.
        fn playing(&mut self) {
            self.send(listed());
            self.send(Event::Monitor(AudioEvent::Added(headset())));
            self.send(Event::VideoOpened {
                session: 1,
                device: card(),
            });
            self.send(opened(1, Side::Input));
            self.send(opened(1, Side::Output));
            assert_eq!(self.audio.state(), AudioState::Playing);
        }

        /// Stores the card's input choice and sends the pick, as the app does.
        fn pick_input(&mut self, choice: AudioChoice) -> Vec<Command> {
            self.settings.touch_device(CARD_VIDEO, "ezcap Game Link").audio = choice.to_string();
            self.send(Event::Input(choice))
        }

        fn pick_output(&mut self, choice: OutputChoice) -> Vec<Command> {
            self.settings.audio.output = choice.to_string();
            self.send(Event::Output(choice))
        }
    }

    fn current(items: &[Item]) -> Vec<Label> {
        items.iter().filter(|i| i.current).map(|i| i.text.clone()).collect()
    }

    #[test]
    fn a_stored_input_plays_for_its_device_only() {
        let mut run = Run::new();
        run.settings.touch_device(CARD_VIDEO, "ezcap Game Link").audio = MIC_INPUT.into();
        run.send(listed());
        let commands = run.send(Event::VideoOpened {
            session: 1,
            device: card(),
        });
        assert_eq!(commands, [request(1, MIC_INPUT)]);
        // Another device keeps Auto: the camera has no pair.
        run.send(Event::VideoOpened {
            session: 2,
            device: lonely_camera(),
        });
        assert_eq!(run.audio.state(), AudioState::NoPair);
    }

    #[test]
    fn picking_another_input_opens_a_new_session() {
        let mut run = Run::new();
        run.playing();
        assert_eq!(
            run.pick_input(AudioChoice::Input(MIC_INPUT.into())),
            [Command::Close, request(2, MIC_INPUT)]
        );
        assert_eq!(run.audio.state(), AudioState::Opening);
        // Auto pairs the card again: another session.
        assert_eq!(
            run.pick_input(AudioChoice::Auto),
            [Command::Close, request(3, CARD_INPUT)]
        );
    }

    #[test]
    fn picking_the_input_already_playing_changes_nothing() {
        let mut run = Run::new();
        run.playing();
        assert!(run.pick_input(AudioChoice::Input(CARD_INPUT.into())).is_empty());
        assert_eq!(run.audio.state(), AudioState::Playing);
        assert_eq!(run.audio.session(), Some(1));
    }

    #[test]
    fn no_audio_closes_the_session_and_auto_opens_it_again() {
        let mut run = Run::new();
        run.playing();
        assert_eq!(run.pick_input(AudioChoice::None), [Command::Close]);
        assert_eq!(run.audio.state(), AudioState::Off);
        assert_eq!(run.audio.session(), None);
        // Remembered: the device opens again without sound.
        run.send(Event::VideoClosed { session: 1 });
        assert!(
            run.send(Event::VideoOpened {
                session: 2,
                device: card(),
            })
            .is_empty()
        );
        assert_eq!(run.audio.state(), AudioState::Off);
        assert_eq!(run.pick_input(AudioChoice::Auto), [request(2, CARD_INPUT)]);
    }

    #[test]
    fn a_pick_from_no_pair_opens_the_input() {
        let mut run = Run::new();
        run.send(listed());
        run.send(Event::VideoOpened {
            session: 1,
            device: lonely_camera(),
        });
        assert_eq!(run.audio.state(), AudioState::NoPair);
        run.settings.touch_device("mf:camera", "NVIDIA Broadcast").audio = MIC_INPUT.into();
        assert_eq!(
            run.send(Event::Input(AudioChoice::Input(MIC_INPUT.into()))),
            [request(1, MIC_INPUT)]
        );
    }

    #[test]
    fn a_missing_input_is_disconnected_until_it_returns() {
        let mut run = Run::new();
        run.send(listed());
        let usb = "{0.0.1.00000000}.{usb}";
        run.send(Event::Monitor(AudioEvent::Added(endpoint(
            usb,
            "Line In (USB Audio)",
            Flow::Input,
            None,
        ))));
        run.send(Event::Monitor(AudioEvent::Removed(usb.into())));
        run.settings.touch_device(CARD_VIDEO, "ezcap Game Link").audio = usb.into();
        assert!(
            run.send(Event::VideoOpened {
                session: 1,
                device: card(),
            })
            .is_empty()
        );
        // Named as when it was last seen.
        assert_eq!(
            run.audio.state(),
            AudioState::Disconnected("Line In (USB Audio)".into())
        );
        // Another input changes nothing; the chosen one opens.
        assert!(
            run.send(Event::Monitor(AudioEvent::Added(endpoint(
                "{0.0.1.00000000}.{other}",
                "Other",
                Flow::Input,
                None,
            ))))
            .is_empty()
        );
        let commands = run.send(Event::Monitor(AudioEvent::Added(endpoint(
            usb,
            "Line In (USB Audio)",
            Flow::Input,
            None,
        ))));
        assert_eq!(commands, [request(1, usb)]);
        assert_eq!(run.audio.state(), AudioState::Opening);
    }

    #[test]
    fn an_input_never_seen_is_named_by_its_id() {
        let mut run = Run::new();
        run.settings.touch_device(CARD_VIDEO, "ezcap Game Link").audio = "gone".into();
        run.send(listed());
        run.send(Event::VideoOpened {
            session: 1,
            device: card(),
        });
        assert_eq!(run.audio.state(), AudioState::Disconnected("gone".into()));
        assert_eq!(run.audio.value(&run.settings), Some(Label::Plain("gone".into())));
    }

    #[test]
    fn the_stored_output_opens_with_the_session() {
        let mut run = Run::new();
        run.settings.audio.output = HEADSET.into();
        run.send(listed());
        run.send(Event::Monitor(AudioEvent::Added(headset())));
        let commands = run.send(Event::VideoOpened {
            session: 1,
            device: card(),
        });
        assert_eq!(commands, [open_request(1, CARD_INPUT, Some(HEADSET))]);
        assert_eq!(run.audio.fallback(), None);
    }

    #[test]
    fn an_output_pick_reopens_only_the_output() {
        let mut run = Run::new();
        run.playing();
        assert_eq!(
            run.pick_output(OutputChoice::Device(HEADSET.into())),
            [reopen_output(Some(HEADSET))]
        );
        assert_eq!(run.audio.state(), AudioState::Playing, "the input keeps playing");
        assert_eq!(run.audio.session(), Some(1));
        assert_eq!(run.pick_output(OutputChoice::Default), [reopen_output(None)]);
        // The same output again: nothing.
        assert!(run.pick_output(OutputChoice::Default).is_empty());
    }

    #[test]
    fn an_output_pick_without_a_session_waits_for_the_next_open() {
        let mut run = Run::new();
        run.send(listed());
        run.send(Event::Monitor(AudioEvent::Added(headset())));
        assert!(run.pick_output(OutputChoice::Device(HEADSET.into())).is_empty());
        assert_eq!(
            run.send(Event::VideoOpened {
                session: 1,
                device: card(),
            }),
            [open_request(1, CARD_INPUT, Some(HEADSET))]
        );
    }

    #[test]
    fn a_missing_output_plays_on_the_default_until_it_returns() {
        let mut run = Run::new();
        run.send(Event::Monitor(AudioEvent::Added(headset())));
        run.send(Event::Monitor(AudioEvent::Removed(HEADSET.into())));
        run.settings.audio.output = HEADSET.into();
        run.send(listed());
        let commands = run.send(Event::VideoOpened {
            session: 1,
            device: card(),
        });
        assert_eq!(commands, [open_request(1, CARD_INPUT, None)]);
        run.send(opened(1, Side::Input));
        run.send(opened(1, Side::Output));
        assert_eq!(run.audio.state(), AudioState::Playing);
        assert_eq!(run.audio.fallback().as_deref(), Some("Headset (G533)"));
        // It returns: the output moves back; the flag clears when it opens there.
        assert_eq!(
            run.send(Event::Monitor(AudioEvent::Added(headset()))),
            [reopen_output(Some(HEADSET))]
        );
        assert!(run.audio.fallback().is_some());
        run.send(opened(1, Side::Output));
        assert_eq!(run.audio.fallback(), None);
    }

    #[test]
    fn a_chosen_output_that_fails_falls_back_to_the_default() {
        let mut run = Run::new();
        run.settings.audio.output = HEADSET.into();
        run.playing();
        // Unplugged: the monitor's remove, then the side's error.
        assert!(run.send(Event::Monitor(AudioEvent::Removed(HEADSET.into()))).is_empty());
        assert_eq!(run.send(output_error(1, AudioError::Gone)), [reopen_output(None)]);
        assert_eq!(run.audio.fallback().as_deref(), Some("Headset (G533)"));
        assert_eq!(run.audio.session(), Some(1), "the input keeps playing");
        // Picking the default clears the flag without a reopen.
        assert!(run.pick_output(OutputChoice::Default).is_empty());
        assert_eq!(run.audio.fallback(), None);
    }

    #[test]
    fn the_fallback_ends_with_the_session() {
        let mut run = Run::new();
        run.settings.audio.output = HEADSET.into();
        run.send(listed());
        run.send(Event::VideoOpened {
            session: 1,
            device: card(),
        });
        assert!(run.audio.fallback().is_some());
        run.send(Event::VideoClosed { session: 1 });
        assert_eq!(run.audio.fallback(), None);
    }

    #[test]
    fn windows_reopens_an_output_on_the_default_when_the_default_changes() {
        let mut run = Run::on(true);
        run.playing();
        assert_eq!(
            run.send(Event::Monitor(AudioEvent::DefaultOutput(Some(HEADSET.into())))),
            [reopen_output(None)]
        );
        // The same default again, or none: nothing.
        assert!(
            run.send(Event::Monitor(AudioEvent::DefaultOutput(Some(HEADSET.into()))))
                .is_empty()
        );
        assert!(run.send(Event::Monitor(AudioEvent::DefaultOutput(None))).is_empty());
        // A chosen output stays where it is.
        run.pick_output(OutputChoice::Device(HEADSET.into()));
        assert!(
            run.send(Event::Monitor(AudioEvent::DefaultOutput(Some(SPEAKERS.into()))))
                .is_empty()
        );
    }

    #[test]
    fn linux_leaves_the_default_move_to_pipewire() {
        let mut run = Run::on(false);
        run.playing();
        assert!(
            run.send(Event::Monitor(AudioEvent::DefaultOutput(Some(HEADSET.into()))))
                .is_empty()
        );
        assert_eq!(run.audio.default_output(), Some(HEADSET));
        assert_eq!(
            run.audio.endpoint_name(None),
            Some("Headset (G533)"),
            "only the name changes"
        );
    }

    #[test]
    fn a_failed_output_on_the_default_reopens_when_a_default_returns() {
        for windows in [false, true] {
            let mut run = Run::on(windows);
            run.playing();
            // PipeWire restarts: the side on the default fails, every endpoint leaves and comes
            // back, then the default is named again (research R7).
            assert!(
                run.send(output_error(1, AudioError::Failed("stream error".into())))
                    .is_empty()
            );
            run.send(Event::Monitor(AudioEvent::Removed(SPEAKERS.into())));
            assert!(run.send(Event::Monitor(AudioEvent::DefaultOutput(None))).is_empty());
            run.send(Event::Monitor(AudioEvent::Added(endpoint(
                SPEAKERS,
                "Speakers (Realtek(R) Audio)",
                Flow::Output,
                None,
            ))));
            assert_eq!(
                run.send(Event::Monitor(AudioEvent::DefaultOutput(Some(SPEAKERS.into())))),
                [reopen_output(None)],
                "windows {windows}"
            );
            run.send(opened(1, Side::Output));
            // Reopened once: the next default change follows the system's rule.
            let again = run.send(Event::Monitor(AudioEvent::DefaultOutput(Some(SPEAKERS.into()))));
            assert!(again.is_empty(), "windows {windows}");
        }
    }

    #[test]
    fn the_menu_lists_inputs_and_outputs_with_the_choices_marked() {
        let mut run = Run::new();
        run.send(listed());
        run.send(Event::Monitor(AudioEvent::Added(headset())));
        // No video device: the output group only.
        let menu = run.audio.menu(&run.settings);
        assert_eq!(menu[0].text, Label::Outputs);
        assert_eq!(menu.len(), 4);

        run.send(Event::VideoOpened {
            session: 1,
            device: card(),
        });
        let menu = run.audio.menu(&run.settings);
        let texts: Vec<Label> = menu.iter().map(|i| i.text.clone()).collect();
        assert_eq!(
            texts,
            [
                Label::Inputs,
                Label::Auto(Some("Digital Audio Interface (ezcap)".into())),
                Label::Plain("Microphone (Realtek(R) Audio)".into()),
                Label::Plain("Digital Audio Interface (ezcap)".into()),
                Label::NoAudio,
                Label::Outputs,
                Label::SystemDefault(Some("Speakers (Realtek(R) Audio)".into())),
                Label::Plain("Speakers (Realtek(R) Audio)".into()),
                Label::Plain("Headset (G533)".into()),
            ]
        );
        assert!(menu.iter().filter(|i| i.text.is_header()).all(|i| i.pick.is_none()));
        assert_eq!(
            current(&menu),
            [
                Label::Auto(Some("Digital Audio Interface (ezcap)".into())),
                Label::SystemDefault(Some("Speakers (Realtek(R) Audio)".into())),
            ]
        );
        assert_eq!(menu[4].pick, Some(Pick::Input(AudioChoice::None)));
        assert_eq!(menu[8].pick, Some(Pick::Output(OutputChoice::Device(HEADSET.into()))));

        run.pick_input(AudioChoice::Input(MIC_INPUT.into()));
        run.pick_output(OutputChoice::Device(HEADSET.into()));
        assert_eq!(
            current(&run.audio.menu(&run.settings)),
            [
                Label::Plain("Microphone (Realtek(R) Audio)".into()),
                Label::Plain("Headset (G533)".into()),
            ]
        );
        assert_eq!(
            run.audio.value(&run.settings),
            Some(Label::Plain("Microphone (Realtek(R) Audio)".into()))
        );
    }

    #[test]
    fn the_menu_follows_the_lists_and_names_a_missing_pair_none() {
        let mut run = Run::new();
        run.send(listed());
        run.send(Event::VideoOpened {
            session: 1,
            device: lonely_camera(),
        });
        assert_eq!(run.audio.value(&run.settings), Some(Label::Auto(None)));
        let menu = run.audio.menu(&run.settings);
        assert_eq!(current(&menu)[0], Label::Auto(None));
        // Two inputs of one name are numbered; one that leaves is gone from the menu.
        let twin = "{0.0.1.00000000}.{mic2}";
        run.send(Event::Monitor(AudioEvent::Added(endpoint(
            twin,
            "Microphone (Realtek(R) Audio)",
            Flow::Input,
            None,
        ))));
        let names: Vec<Label> = run.audio.menu(&run.settings)[2..5]
            .iter()
            .map(|i| i.text.clone())
            .collect();
        assert_eq!(
            names,
            [
                Label::Plain("Microphone (Realtek(R) Audio)".into()),
                Label::Plain("Digital Audio Interface (ezcap)".into()),
                Label::Plain("Microphone (Realtek(R) Audio) (2)".into()),
            ]
        );
        run.send(Event::Monitor(AudioEvent::Removed(CARD_INPUT.into())));
        assert_eq!(run.audio.menu(&run.settings).len(), 8);
        // A chosen output that is missing: nothing marked in the output group.
        run.settings.audio.output = HEADSET.into();
        assert!(current(&run.audio.output_items(&run.settings)).is_empty());
    }
}
