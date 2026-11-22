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
use platform::Access;
use platform::audio::{AudioError, AudioEvent, Endpoint, Flow, SessionEvent, SessionRequest, Side, StreamMode};
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
    /// Both sides run, but this endpoint was marked exclusive and refused it, so it plays shared
    /// (Windows); exclusive is asked again at its next open.
    InUse(String),
    /// Auto and no input pairs with the video device.
    NoPair,
    /// The chosen or paired input is missing or left, by its last known name; it opens when it
    /// returns.
    Disconnected(String),
    /// Windows microphone privacy refuses the input; it opens when the switch is read on again.
    Blocked,
    /// Another app holds this endpoint alone (Windows); **Try again** reopens.
    Held(String),
    /// This endpoint failed to open or to play; **Try again** reopens.
    Failed(String),
}

/// The tag before the Audio select's value (FR-017, FR-022).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateTag {
    Disconnected,
    InUse,
    Blocked,
    Failed,
}

/// What the top of the Audio menu says (FR-017, FR-022): a title and a detail line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Notice {
    /// `No paired audio` / `Choose an input`.
    NoPair,
    /// `Device disconnected` / `{}` / `Waiting to reconnect`.
    Disconnected(String),
    /// `Access blocked` / `Desktop app permission is off`.
    Blocked,
    /// `Device in use` / `{}` / `Exclusive mode unavailable`: it plays shared.
    InUse(String),
    /// `Device in use` / `{}`, with `Try again`.
    Held(String),
    /// `Cannot open` / `{}`, with `Try again`; the name is empty for a system default output
    /// without a name.
    Failed(String),
    /// `Device disconnected` / `{}` / `Using the system default`: the chosen output.
    Fallback(String),
}

/// The action under the menu's notice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// `Try again`: a new session.
    TryAgain,
    /// `Open privacy settings`: the system's microphone privacy page.
    OpenPrivacy,
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
    /// The user changed this endpoint's exclusive box (FR-014); the settings already hold it.
    Exclusive(String),
    /// The user pressed **Try again** in the Audio menu.
    TryAgain,
    /// Every 3 s while [`AudioController::watches_access`]: microphone access is read, and once
    /// it was read off and then no longer is, the input opens again.
    AccessCheck,
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
            Self::Exclusive(id) => format!("{id} exclusive changed"),
            Self::TryAgain => "try again".into(),
            Self::AccessCheck => "microphone access read".into(),
        }
    }
}

/// What the app does for the preview's sound. Every command returns at once (research R9).
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    /// Open this session; a session still open is closed first by [`Command::Close`].
    Open(SessionRequest),
    /// Reopen the open session's input on this endpoint (the bridge restarts empty).
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
    /// A chosen (or Auto's paired) endpoint that is missing: shown with `Disconnected`.
    pub missing: bool,
    /// Its endpoint has the exclusive switch on (Auto: the pair; System default: the default):
    /// shown with the `Exclusive` tag (FR-022).
    pub exclusive: bool,
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
    /// The state's title and detail, the menu's first row; not picked.
    Notice(Notice),
    /// The state's action, the row after the notice.
    Action(Action),
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
    Action(Action),
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
    /// The sides that reported `Opened` since their last open, with the mode they run in.
    input: Option<StreamMode>,
    output: Option<StreamMode>,
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

/// The endpoint's exclusive box (FR-014); the platform ignores it on Linux.
fn exclusive(settings: &Settings, id: Option<&str>) -> bool {
    id.and_then(|id| settings.audio_device(id)).is_some_and(|d| d.exclusive)
}

/// One endpoint in the settings panel's Audio category (FR-014).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExclusiveRow {
    pub id: String,
    pub name: String,
    pub exclusive: bool,
    /// The preview plays on it now (the `Preview` tag).
    pub in_use: bool,
}

/// Reads the microphone privacy switch (`platform::access`; tests set it).
struct AccessReader(Box<dyn Fn() -> Access + Send>);

impl std::fmt::Debug for AccessReader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("AccessReader")
    }
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
    access: AccessReader,
    /// Microphone access was read off since the state became Blocked: the input opens once it no
    /// longer is.
    access_was_denied: bool,
}

impl Default for AudioController {
    fn default() -> Self {
        Self::with(platform::IS_WINDOWS, || {
            platform::access(platform::Capability::Microphone)
        })
    }
}

impl AudioController {
    pub fn new() -> Self {
        Self::default()
    }

    /// `reopens_on_default`: the system leaves an output on the old default (Windows); `access`
    /// reads the microphone privacy switch.
    fn with(reopens_on_default: bool, access: impl Fn() -> Access + Send + 'static) -> Self {
        Self {
            access: AccessReader(Box::new(access)),
            access_was_denied: false,
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
    /// The name of an endpoint seen in this run or stored with a choice; None when unknown.
    pub fn known_name(&self, id: &str) -> Option<&str> {
        self.names.get(id).map(String::as_str)
    }

    fn name_of(&self, id: &str) -> String {
        self.names.get(id).cloned().unwrap_or_else(|| id.to_string())
    }

    /// The chosen output's name while the sound plays on the system default instead
    /// (`Device disconnected` / `{}` / `Using the system default`); None otherwise.
    pub fn fallback(&self) -> Option<String> {
        self.open.as_ref()?;
        self.fallback.as_deref().map(|id| self.name_of(id))
    }

    /// The tag before the Audio select's value; None while the sound plays as expected.
    pub fn tag(&self) -> Option<StateTag> {
        match self.state() {
            AudioState::Disconnected(_) => Some(StateTag::Disconnected),
            AudioState::InUse(_) | AudioState::Held(_) => Some(StateTag::InUse),
            AudioState::Blocked => Some(StateTag::Blocked),
            AudioState::Failed(_) => Some(StateTag::Failed),
            AudioState::Off | AudioState::Opening | AudioState::Playing | AudioState::NoPair => None,
        }
    }

    /// The settings panel's Audio category (FR-014): every input and every output with its
    /// exclusive switch, marked when the preview plays on it; those first, then the system's
    /// order.
    pub fn exclusive_rows(&self, settings: &Settings) -> (Vec<ExclusiveRow>, Vec<ExclusiveRow>) {
        let (input, output) = match &self.open {
            Some(open) => (
                open.input.is_some().then_some(open.input_id.as_str()),
                open.output
                    .and(open.output_id.as_deref().or(self.default_output.as_deref())),
            ),
            None => (None, None),
        };
        let rows = |list: &[Endpoint], used: Option<&str>| {
            let mut rows: Vec<ExclusiveRow> = list
                .iter()
                .map(|e| ExclusiveRow {
                    id: e.id.clone(),
                    name: e.name.clone(),
                    exclusive: exclusive(settings, Some(&e.id)),
                    in_use: used == Some(e.id.as_str()),
                })
                .collect();
            rows.sort_by_key(|r| !r.in_use);
            rows
        };
        (rows(&self.inputs, input), rows(&self.outputs, output))
    }

    /// The top of the Audio menu: the state's notice and its action, else the output fallback.
    fn notice(&self) -> Option<(Notice, Option<Action>)> {
        Some(match self.state() {
            AudioState::NoPair => (Notice::NoPair, None),
            AudioState::Disconnected(name) => (Notice::Disconnected(name), None),
            AudioState::Blocked => (Notice::Blocked, Some(Action::OpenPrivacy)),
            AudioState::InUse(name) => (Notice::InUse(name), None),
            AudioState::Held(name) => (Notice::Held(name), Some(Action::TryAgain)),
            AudioState::Failed(name) => (Notice::Failed(name), Some(Action::TryAgain)),
            AudioState::Playing => (Notice::Fallback(self.fallback()?), None),
            AudioState::Off | AudioState::Opening => return None,
        })
    }

    /// Microphone access is read every 3 s ([`Event::AccessCheck`]): the input is Blocked.
    pub fn watches_access(&self) -> bool {
        self.video.is_some() && self.state() == AudioState::Blocked
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
            AudioChoice::Auto => Label::Auto(self.auto_name(video)),
            AudioChoice::None => Label::NoAudio,
            AudioChoice::Input(id) => Label::Plain(self.name_of(&id)),
        })
    }

    /// The name Auto shows: the paired input, or the one that left while it is Disconnected.
    fn auto_name(&self, video: &VideoDevice) -> Option<String> {
        match (self.pair(video), self.state()) {
            (Some(id), _) => Some(self.name_of(&id)),
            (None, AudioState::Disconnected(name)) => Some(name),
            (None, _) => None,
        }
    }

    /// The Audio dropdown's menu: the state's notice and action when there is one, the `Input`
    /// group (Auto, every input, No audio) while a video
    /// device is open, then the `Output` group; the choices in use marked. A chosen endpoint that
    /// is missing stays in its group, marked and `missing`, until it returns or another is picked
    /// (Auto names its paired input that left the same way).
    pub fn menu(&self, settings: &Settings) -> Vec<Item> {
        let header = |text| Item {
            text,
            current: false,
            pick: None,
            missing: false,
            exclusive: false,
        };
        let mut items = Vec::new();
        if let Some((notice, action)) = self.notice() {
            items.push(header(Label::Notice(notice)));
            if let Some(action) = action {
                items.push(Item {
                    text: Label::Action(action),
                    current: false,
                    pick: Some(Pick::Action(action)),
                    missing: false,
                    exclusive: false,
                });
            }
        }
        if let Some((_, video)) = &self.video {
            let choice = input_choice(settings, video);
            items.push(header(Label::Inputs));
            let auto = choice == AudioChoice::Auto;
            items.push(Item {
                text: Label::Auto(self.auto_name(video)),
                current: auto,
                pick: Some(Pick::Input(AudioChoice::Auto)),
                missing: auto && self.pair(video).is_none() && matches!(self.state(), AudioState::Disconnected(_)),
                exclusive: exclusive(settings, self.pair(video).as_deref()),
            });
            for input in &self.inputs {
                let pick = AudioChoice::Input(input.id.clone());
                items.push(Item {
                    text: Label::Plain(input.name.clone()),
                    current: choice == pick,
                    pick: Some(Pick::Input(pick)),
                    missing: false,
                    exclusive: exclusive(settings, Some(&input.id)),
                });
            }
            if let AudioChoice::Input(id) = &choice
                && !self.inputs.iter().any(|e| &e.id == id)
            {
                items.push(Item {
                    text: Label::Plain(self.name_of(id)),
                    current: true,
                    pick: Some(Pick::Input(choice.clone())),
                    missing: true,
                    exclusive: exclusive(settings, Some(id)),
                });
            }
            items.push(Item {
                text: Label::NoAudio,
                current: choice == AudioChoice::None,
                pick: Some(Pick::Input(AudioChoice::None)),
                missing: false,
                exclusive: false,
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
        let missing = match &choice {
            OutputChoice::Device(id) if !self.outputs.iter().any(|e| &e.id == id) => Some(Item {
                text: Label::Plain(self.name_of(id)),
                current: true,
                pick: Some(Pick::Output(choice.clone())),
                missing: true,
                exclusive: exclusive(settings, Some(id)),
            }),
            _ => None,
        };
        std::iter::once(Item {
            text: Label::SystemDefault(default_name),
            current: choice == OutputChoice::Default,
            pick: Some(Pick::Output(OutputChoice::Default)),
            missing: false,
            exclusive: exclusive(settings, self.default_output.as_deref()),
        })
        .chain(self.outputs.iter().map(|output| {
            let pick = OutputChoice::Device(output.id.clone());
            Item {
                text: Label::Plain(output.name.clone()),
                current: choice == pick,
                pick: Some(Pick::Output(pick)),
                missing: false,
                exclusive: exclusive(settings, Some(&output.id)),
            }
        }))
        .chain(missing)
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
                // The last known names of chosen endpoints, for those missing now.
                for (id, device) in &settings.audio_devices {
                    if !device.name.is_empty() {
                        self.names.entry(id.clone()).or_insert_with(|| device.name.clone());
                    }
                }
                for endpoint in self.inputs.iter().chain(&self.outputs) {
                    self.names.insert(endpoint.id.clone(), endpoint.name.clone());
                }
                self.listed = true;
                if self.open.is_none() {
                    match self.state() {
                        // The first list, or nothing to play.
                        AudioState::Off | AudioState::Opening => self.resolve(settings, &mut out),
                        AudioState::NoPair | AudioState::Disconnected(_) => self.play_if_present(settings, &mut out),
                        // Errors wait for their action or for access.
                        AudioState::Playing
                        | AudioState::InUse(_)
                        | AudioState::Blocked
                        | AudioState::Held(_)
                        | AudioState::Failed(_) => {}
                    }
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
            Event::Monitor(AudioEvent::DefaultOutput(id)) => self.default_changed(id, settings, &mut out),
            Event::Session(event) => self.session_event(event, settings, &mut out),
            Event::VideoOpened { session, device } => {
                if self.keeps(&device, settings) {
                    log::info!("audio: video session {session} on the same device keeps the audio session");
                    self.video = Some((session, device));
                } else {
                    self.close(&mut out);
                    self.video = Some((session, device));
                    self.resolve(settings, &mut out);
                }
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
            Event::Exclusive(id) => self.exclusive_changed(&id, settings, &mut out),
            Event::TryAgain => {
                if matches!(self.state(), AudioState::Held(_) | AudioState::Failed(_)) {
                    self.close(&mut out);
                    self.resolve(settings, &mut out);
                }
            }
            Event::AccessCheck => {
                if self.watches_access() {
                    if (self.access.0)() == Access::Denied {
                        self.access_was_denied = true;
                    } else if self.access_was_denied {
                        log::info!("audio: microphone access is back, opening the input again");
                        self.resolve(settings, &mut out);
                    }
                }
            }
        }
        out
    }

    /// A new video session on the device already open (a new mode) keeps the audio session when it
    /// is playing or opening on the input and output the choice still resolves to, so the sound
    /// plays on (User Story 1 scenario 3). An error state opens again, as a new device does.
    fn keeps(&self, device: &VideoDevice, settings: &Settings) -> bool {
        let (Some((_, video)), Some(open)) = (&self.video, &self.open) else {
            return false;
        };
        if video.id != device.id || !matches!(self.state(), AudioState::Opening | AudioState::Playing) {
            return false;
        }
        let Resolved::Play(input) = self.resolved(device, settings) else {
            return false;
        };
        input == open.input_id && self.output_to_open(settings).0 == open.output_id
    }

    /// Opens the video device's sound when its input is present now; else the state stays (an
    /// input that is not the missing one came or left).
    fn play_if_present(&mut self, settings: &Settings, out: &mut Vec<Command>) {
        if let Some((_, video)) = &self.video
            && let Resolved::Play(_) = self.resolved(video, settings)
        {
            self.resolve(settings, out);
        }
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
            // Windows refuses every input while the switch is off: no open (research R6).
            Resolved::Play(_) if (self.access.0)() == Access::Denied => {
                self.state = Some(AudioState::Blocked);
                self.access_was_denied = true;
            }
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
            input: None,
            output: None,
            output_failed: false,
        });
        self.state = Some(AudioState::Opening);
        let input_exclusive = exclusive(settings, Some(&input));
        let output_exclusive = exclusive(settings, output.as_deref().or(self.default_output.as_deref()));
        out.push(Command::Open(SessionRequest {
            number,
            input: Some(input),
            output,
            input_exclusive,
            output_exclusive,
        }));
    }

    /// Closes the open audio session, if any.
    fn close(&mut self, out: &mut Vec<Command>) {
        if self.open.take().is_some() {
            out.push(Command::Close);
        }
        self.fallback = None;
    }

    /// Reopens the open session's output on `output` (None: the system default), exclusive when
    /// that endpoint's box is checked.
    fn reopen_output(&mut self, output: Option<String>, settings: &Settings, out: &mut Vec<Command>) {
        let exclusive = exclusive(settings, output.as_deref().or(self.default_output.as_deref()));
        if let Some(open) = &mut self.open {
            open.output_id.clone_from(&output);
            open.output = None;
            open.output_failed = false;
            // An output side that failed on the default opens again.
            if matches!(self.state, Some(AudioState::Held(_) | AudioState::Failed(_))) {
                self.state = Some(AudioState::Opening);
            }
            out.push(Command::ReopenOutput { output, exclusive });
        }
    }

    /// An exclusive box changed: the side playing on that endpoint reopens in the new mode, the
    /// other keeps playing (FR-005).
    fn exclusive_changed(&mut self, id: &str, settings: &Settings, out: &mut Vec<Command>) {
        let Some(open) = &mut self.open else { return };
        if open.input_id == id {
            open.input = None;
            out.push(Command::ReopenInput {
                input: id.to_string(),
                exclusive: exclusive(settings, Some(id)),
            });
        } else if open.output_id.as_deref().or(self.default_output.as_deref()) == Some(id) {
            let output = open.output_id.clone();
            self.reopen_output(output, settings, out);
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
            // An input that pairs, or the missing one, may have come.
            Flow::Input => {
                if self.open.is_none() && matches!(self.state, Some(AudioState::NoPair | AudioState::Disconnected(_))) {
                    self.play_if_present(settings, out);
                }
            }
            // The chosen output returned: the sound moves back to it.
            Flow::Output => {
                if self.fallback.as_deref() == Some(id.as_str())
                    && self.open.as_ref().is_some_and(|o| o.output_id.is_none())
                {
                    self.reopen_output(Some(id), settings, out);
                }
            }
        }
    }

    /// The system default output changed: an output on the default reopens on Windows, and on
    /// both systems when it failed (research R7).
    fn default_changed(&mut self, id: Option<String>, settings: &Settings, out: &mut Vec<Command>) {
        let changed = self.default_output != id;
        self.default_output = id;
        let Some(open) = &self.open else { return };
        if open.output_id.is_none()
            && self.default_output.is_some()
            && (open.output_failed || (changed && self.reopens_on_default))
        {
            self.reopen_output(None, settings, out);
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
            self.reopen_output(output, settings, out);
        }
    }

    fn session_event(&mut self, event: SessionEvent, settings: &Settings, out: &mut Vec<Command>) {
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
            SessionEvent::Opened { side, mode, .. } => {
                match side {
                    Side::Input => open.input = Some(mode),
                    Side::Output => {
                        open.output = Some(mode);
                        // The chosen output plays again: no fallback.
                        if open.output_id.is_some() {
                            self.fallback = None;
                        }
                    }
                }
                if let (Some(input), Some(output)) = (open.input, open.output) {
                    // An exclusive side refused plays shared and says so (User Story 4,
                    // scenario 3); the input's is named first.
                    let refused = if input == StreamMode::SharedAfterInUse {
                        Some(open.input_id.clone())
                    } else if output == StreamMode::SharedAfterInUse {
                        open.output_id.clone().or_else(|| self.default_output.clone())
                    } else {
                        None
                    };
                    self.state = Some(match refused {
                        Some(id) => AudioState::InUse(self.name_of(&id)),
                        None => AudioState::Playing,
                    });
                }
            }
            SessionEvent::Error {
                side: Side::Output,
                error,
                ..
            } => {
                log::warn!("audio session {number}: output side: {error}");
                match open.output_id.take() {
                    // The chosen output is gone, failed or held: the default plays, and says so.
                    Some(id) => {
                        self.fallback = Some(id);
                        self.reopen_output(None, settings, out);
                    }
                    // The default: it reopens when `DefaultOutput` names an output (research R7),
                    // or on Try again. A default that left waits without a state (Windows names
                    // the next default at once).
                    None => {
                        open.output = None;
                        open.output_failed = true;
                        let name = self
                            .default_output
                            .as_deref()
                            .map(|id| self.name_of(id))
                            .unwrap_or_default();
                        self.state = Some(match error {
                            AudioError::Gone => AudioState::Opening,
                            AudioError::InUse => AudioState::Held(name),
                            AudioError::Blocked | AudioError::Format | AudioError::Failed(_) => {
                                AudioState::Failed(name)
                            }
                        });
                    }
                }
            }
            SessionEvent::Error {
                side: Side::Input,
                error,
                ..
            } => {
                log::warn!("audio session {number}: input side: {error}");
                let id = open.input_id.clone();
                let name = self.name_of(&id);
                if error == AudioError::Gone {
                    // Listed until the monitor's `Removed`: not opened again before it returns.
                    self.inputs.retain(|e| e.id != id);
                    platform::audio::numbered(&mut self.inputs);
                }
                self.close(out);
                self.state = Some(match error {
                    AudioError::Gone => AudioState::Disconnected(name),
                    AudioError::Blocked => {
                        self.access_was_denied = (self.access.0)() == Access::Denied;
                        AudioState::Blocked
                    }
                    AudioError::InUse => AudioState::Held(name),
                    AudioError::Format | AudioError::Failed(_) => AudioState::Failed(name),
                });
            }
            SessionEvent::Closed { .. } => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

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
            input: Some(input.into()),
            output: None,
            input_exclusive: false,
            output_exclusive: false,
        })
    }

    struct Run {
        audio: AudioController,
        settings: Settings,
        /// What the microphone privacy switch reads.
        access: Arc<Mutex<Access>>,
    }

    impl Run {
        fn new() -> Self {
            Self::on(platform::IS_WINDOWS)
        }

        /// `windows`: an output on the default reopens when the default changes.
        fn on(windows: bool) -> Self {
            let access = Arc::new(Mutex::new(Access::Unknown));
            let reader = Arc::clone(&access);
            Self {
                audio: AudioController::with(windows, move || *reader.lock().unwrap()),
                settings: Settings::default(),
                access,
            }
        }

        fn set_access(&self, access: Access) {
            *self.access.lock().unwrap() = access;
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
        assert_eq!(run.send(volume(50, false)), [Command::SetGain(0.25)]);
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
        run.settings.touch_device("mf:camera", "NVIDIA Broadcast").audio = MIC_INPUT.into();
        run.send(listed());
        run.send(Event::VideoOpened {
            session: 1,
            device: card(),
        });
        let commands = run.send(Event::VideoOpened {
            session: 2,
            device: lonely_camera(),
        });
        assert_eq!(commands, [Command::Close, request(2, MIC_INPUT)]);
        // The old video session's close comes late: the new one stays.
        assert!(run.send(Event::VideoClosed { session: 1 }).is_empty());
        assert_eq!(run.audio.session(), Some(2));
    }

    #[test]
    fn a_new_mode_of_the_same_device_keeps_the_sound() {
        let mut run = Run::new();
        run.playing();
        // The app sends no close for a reopen of the same device.
        assert!(
            run.send(Event::VideoOpened {
                session: 2,
                device: card(),
            })
            .is_empty()
        );
        assert_eq!(run.audio.state(), AudioState::Playing);
        assert_eq!(run.audio.session(), Some(1));
        // The audio session now belongs to video session 2.
        assert!(run.send(Event::VideoClosed { session: 1 }).is_empty());
        assert_eq!(run.send(Event::VideoClosed { session: 2 }), [Command::Close]);
    }

    #[test]
    fn a_new_mode_while_opening_keeps_the_session_too() {
        let mut run = Run::new();
        run.send(listed());
        run.send(Event::VideoOpened {
            session: 1,
            device: card(),
        });
        assert!(
            run.send(Event::VideoOpened {
                session: 2,
                device: card(),
            })
            .is_empty()
        );
        assert_eq!(run.audio.state(), AudioState::Opening);
        assert_eq!(run.audio.session(), Some(1));
    }

    #[test]
    fn a_new_mode_opens_again_when_the_input_resolves_elsewhere() {
        let mut run = Run::new();
        run.playing();
        run.settings.touch_device(CARD_VIDEO, "ezcap Game Link").audio = MIC_INPUT.into();
        let commands = run.send(Event::VideoOpened {
            session: 2,
            device: card(),
        });
        assert_eq!(commands, [Command::Close, request(2, MIC_INPUT)]);
    }

    #[test]
    fn a_new_mode_opens_again_after_an_input_refused_exclusive() {
        let mut run = Run::new();
        run.playing();
        run.send(Event::Session(SessionEvent::Opened {
            number: 1,
            side: Side::Input,
            mode: StreamMode::SharedAfterInUse,
            format: Format {
                rate: 48000,
                channels: 2,
                sample: SampleFormat::F32,
            },
            period_ms: 10.0,
            reported_latency_ms: None,
        }));
        assert!(matches!(run.audio.state(), AudioState::InUse(_)));
        let commands = run.send(Event::VideoOpened {
            session: 2,
            device: card(),
        });
        assert_eq!(commands, [Command::Close, request(2, CARD_INPUT)]);
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
            input: Some(input.into()),
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
    fn a_chosen_output_missing_at_start_shows_its_stored_name() {
        let mut run = Run::new();
        run.settings.audio.output = HEADSET.into();
        run.settings.touch_audio_device(HEADSET, "Headset (G533)");
        run.send(listed());
        run.send(Event::VideoOpened {
            session: 1,
            device: card(),
        });
        assert_eq!(run.audio.fallback().as_deref(), Some("Headset (G533)"));
        let outputs = run.audio.output_items(&run.settings);
        let missing = outputs.iter().find(|i| i.missing).unwrap();
        assert_eq!(missing.text, Label::Plain("Headset (G533)".into()));
        // Never seen and not stored: its id.
        let mut run = Run::new();
        run.settings.audio.output = HEADSET.into();
        run.send(listed());
        let outputs = run.audio.output_items(&run.settings);
        assert_eq!(outputs.last().unwrap().text, Label::Plain(HEADSET.into()));
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
        // The notice first: no paired input.
        assert_eq!(run.audio.menu(&run.settings)[0].text, Label::Notice(Notice::NoPair));
        let names: Vec<Label> = run.audio.menu(&run.settings)[3..6]
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
        assert_eq!(run.audio.menu(&run.settings).len(), 9);
        // A chosen output that is missing stays last in its group, marked and missing.
        run.settings.audio.output = HEADSET.into();
        let outputs = run.audio.output_items(&run.settings);
        let last = outputs.last().unwrap();
        assert_eq!(current(&outputs), std::slice::from_ref(&last.text));
        assert!(last.missing);
        assert_eq!(last.pick, Some(Pick::Output(OutputChoice::Device(HEADSET.into()))));
        assert_eq!(outputs.iter().filter(|i| i.missing).count(), 1);
    }

    fn input_error(number: u64, error: AudioError) -> Event {
        Event::Session(SessionEvent::Error {
            number,
            side: Side::Input,
            error,
        })
    }

    /// The menu's first rows: the notice and its action, if any.
    fn top(run: &Run) -> (Option<Label>, Option<Pick>) {
        let menu = run.audio.menu(&run.settings);
        let notice = menu
            .first()
            .map(|i| i.text.clone())
            .filter(|t| matches!(t, Label::Notice(_)));
        let action = menu.get(1).and_then(|i| match i.pick {
            Some(Pick::Action(_)) => i.pick.clone(),
            _ => None,
        });
        (notice, action)
    }

    /// The first list without one input.
    fn listed_without(id: &str) -> Event {
        let Event::Monitor(AudioEvent::Listed {
            mut inputs,
            outputs,
            default_output,
        }) = listed()
        else {
            unreachable!()
        };
        inputs.retain(|e| e.id != id);
        Event::Monitor(AudioEvent::Listed {
            inputs,
            outputs,
            default_output,
        })
    }

    #[test]
    fn a_chosen_input_unplugged_is_disconnected_until_it_returns() {
        let mut run = Run::new();
        run.playing();
        run.pick_input(AudioChoice::Input(MIC_INPUT.into()));
        run.send(opened(2, Side::Input));
        run.send(opened(2, Side::Output));
        // Unplugged: the side's error, then (or before) the monitor's remove.
        assert_eq!(run.send(input_error(2, AudioError::Gone)), [Command::Close]);
        let name = "Microphone (Realtek(R) Audio)".to_string();
        assert_eq!(run.audio.state(), AudioState::Disconnected(name.clone()));
        assert_eq!(run.audio.tag(), Some(StateTag::Disconnected));
        assert_eq!(
            top(&run),
            (Some(Label::Notice(Notice::Disconnected(name.clone()))), None)
        );
        assert_eq!(run.audio.session(), None);
        assert!(
            run.send(Event::Monitor(AudioEvent::Removed(MIC_INPUT.into())))
                .is_empty()
        );
        // Gone from the list, it stays in the menu, marked and missing, and in the value.
        let menu = run.audio.menu(&run.settings);
        let missing: Vec<&Item> = menu.iter().filter(|i| i.missing).collect();
        assert_eq!(missing.len(), 1);
        assert_eq!(missing[0].text, Label::Plain(name.clone()));
        assert!(missing[0].current);
        assert_eq!(current(&menu)[0], Label::Plain(name.clone()));
        assert_eq!(run.audio.value(&run.settings), Some(Label::Plain(name)));
        // Another input, or the monitor's list read again: still waiting.
        assert!(
            run.send(Event::Monitor(AudioEvent::Added(endpoint(
                "{0.0.1.00000000}.{other}",
                "Other",
                Flow::Input,
                None,
            ))))
            .is_empty()
        );
        assert!(run.send(listed_without(MIC_INPUT)).is_empty());
        assert_eq!(run.audio.tag(), Some(StateTag::Disconnected));
        // It returns: a new session on it.
        let commands = run.send(Event::Monitor(AudioEvent::Added(endpoint(
            MIC_INPUT,
            "Microphone (Realtek(R) Audio)",
            Flow::Input,
            None,
        ))));
        assert_eq!(commands, [request(3, MIC_INPUT)]);
        assert_eq!(run.audio.state(), AudioState::Opening);
        assert_eq!(run.audio.tag(), None);
    }

    #[test]
    fn a_paired_input_that_leaves_is_disconnected_not_unpaired() {
        let mut run = Run::new();
        run.playing();
        assert_eq!(run.send(input_error(1, AudioError::Gone)), [Command::Close]);
        assert_eq!(
            run.audio.state(),
            AudioState::Disconnected("Digital Audio Interface (ezcap)".into())
        );
        // The monitor's remove comes after: the state stays (not No paired audio).
        run.send(Event::Monitor(AudioEvent::Removed(CARD_INPUT.into())));
        assert!(matches!(run.audio.state(), AudioState::Disconnected(_)));
        // Auto keeps the name of the input that left, marked and missing.
        let auto = Label::Auto(Some("Digital Audio Interface (ezcap)".into()));
        assert_eq!(run.audio.value(&run.settings), Some(auto.clone()));
        let menu = run.audio.menu(&run.settings);
        let row = menu.iter().find(|i| i.text == auto).unwrap();
        assert!(row.current && row.missing);
        let commands = run.send(Event::Monitor(AudioEvent::Added(endpoint(
            CARD_INPUT,
            "Digital Audio Interface (ezcap)",
            Flow::Input,
            Some(CARD_LINK),
        ))));
        assert_eq!(commands, [request(2, CARD_INPUT)]);
    }

    #[test]
    fn a_blocked_input_opens_when_access_is_back() {
        let mut run = Run::new();
        run.playing();
        run.set_access(Access::Denied);
        assert_eq!(run.send(input_error(1, AudioError::Blocked)), [Command::Close]);
        assert_eq!(run.audio.state(), AudioState::Blocked);
        assert_eq!(run.audio.tag(), Some(StateTag::Blocked));
        assert_eq!(
            top(&run),
            (
                Some(Label::Notice(Notice::Blocked)),
                Some(Pick::Action(Action::OpenPrivacy))
            )
        );
        assert!(run.audio.watches_access());
        assert!(run.send(Event::AccessCheck).is_empty(), "still off");
        // Try again is not its action.
        assert!(run.send(Event::TryAgain).is_empty());
        run.set_access(Access::Allowed);
        assert_eq!(run.send(Event::AccessCheck), [request(2, CARD_INPUT)]);
        assert!(!run.audio.watches_access());
        assert!(run.send(Event::AccessCheck).is_empty());
    }

    #[test]
    fn access_denied_before_opening_opens_nothing() {
        let mut run = Run::new();
        run.set_access(Access::Denied);
        run.send(listed());
        assert!(
            run.send(Event::VideoOpened {
                session: 1,
                device: card(),
            })
            .is_empty()
        );
        assert_eq!(run.audio.state(), AudioState::Blocked);
        run.send(Event::AccessCheck);
        run.set_access(Access::Allowed);
        assert_eq!(run.send(Event::AccessCheck), [request(1, CARD_INPUT)]);
        // Nothing to play needs no access: No audio stays Off.
        run.set_access(Access::Denied);
        run.pick_input(AudioChoice::None);
        assert_eq!(run.audio.state(), AudioState::Off);
        assert!(!run.audio.watches_access());
    }

    #[test]
    fn a_blocked_input_without_a_switch_read_off_waits() {
        // Blocked by the session while the switch reads on (the error and the switch disagree):
        // only a read that goes off and then on again opens it.
        let mut run = Run::new();
        run.playing();
        run.send(input_error(1, AudioError::Blocked));
        run.set_access(Access::Allowed);
        assert!(run.send(Event::AccessCheck).is_empty());
        run.set_access(Access::Denied);
        run.send(Event::AccessCheck);
        run.set_access(Access::Unknown);
        assert_eq!(run.send(Event::AccessCheck), [request(2, CARD_INPUT)]);
    }

    #[test]
    fn a_held_input_opens_again_on_try_again() {
        let mut run = Run::new();
        run.playing();
        assert_eq!(run.send(input_error(1, AudioError::InUse)), [Command::Close]);
        assert_eq!(
            run.audio.state(),
            AudioState::Held("Digital Audio Interface (ezcap)".into())
        );
        assert_eq!(run.audio.tag(), Some(StateTag::InUse));
        assert_eq!(
            top(&run),
            (
                Some(Label::Notice(Notice::Held("Digital Audio Interface (ezcap)".into()))),
                Some(Pick::Action(Action::TryAgain))
            )
        );
        // No automatic retry: the monitor's events change nothing.
        assert!(run.send(listed()).is_empty());
        assert_eq!(run.send(Event::TryAgain), [request(2, CARD_INPUT)]);
        assert_eq!(run.audio.state(), AudioState::Opening);
    }

    #[test]
    fn a_failed_input_opens_again_on_try_again() {
        for error in [AudioError::Format, AudioError::Failed("0x88890004".into())] {
            let mut run = Run::new();
            run.playing();
            assert_eq!(run.send(input_error(1, error)), [Command::Close]);
            let name = "Digital Audio Interface (ezcap)".to_string();
            assert_eq!(run.audio.state(), AudioState::Failed(name.clone()));
            assert_eq!(run.audio.tag(), Some(StateTag::Failed));
            assert_eq!(
                top(&run),
                (
                    Some(Label::Notice(Notice::Failed(name))),
                    Some(Pick::Action(Action::TryAgain))
                )
            );
            assert_eq!(run.send(Event::TryAgain), [request(2, CARD_INPUT)]);
            // Try again while it plays does nothing.
            run.send(opened(2, Side::Input));
            run.send(opened(2, Side::Output));
            assert!(run.send(Event::TryAgain).is_empty());
        }
    }

    #[test]
    fn an_input_error_of_an_opening_session_shows_its_state() {
        let mut run = Run::new();
        run.send(listed());
        run.send(Event::VideoOpened {
            session: 1,
            device: card(),
        });
        run.send(opened(1, Side::Output));
        assert_eq!(run.send(input_error(1, AudioError::InUse)), [Command::Close]);
        assert!(matches!(run.audio.state(), AudioState::Held(_)));
        // A late `Opened` of the closed session changes nothing.
        run.send(opened(1, Side::Input));
        assert!(matches!(run.audio.state(), AudioState::Held(_)));
    }

    #[test]
    fn a_failed_default_output_keeps_the_input_and_reopens_on_a_default() {
        let mut run = Run::on(false);
        run.playing();
        assert!(
            run.send(output_error(1, AudioError::Failed("stream error".into())))
                .is_empty(),
            "the input keeps its side"
        );
        let speakers = "Speakers (Realtek(R) Audio)".to_string();
        assert_eq!(run.audio.state(), AudioState::Failed(speakers.clone()));
        assert_eq!(
            top(&run),
            (
                Some(Label::Notice(Notice::Failed(speakers))),
                Some(Pick::Action(Action::TryAgain))
            )
        );
        assert_eq!(run.audio.session(), Some(1));
        // A default named again reopens it (research R7).
        assert_eq!(
            run.send(Event::Monitor(AudioEvent::DefaultOutput(Some(SPEAKERS.into())))),
            [reopen_output(None)]
        );
        assert_eq!(run.audio.state(), AudioState::Opening);
        run.send(opened(1, Side::Output));
        assert_eq!(run.audio.state(), AudioState::Playing);
    }

    #[test]
    fn a_held_default_output_opens_a_new_session_on_try_again() {
        let mut run = Run::new();
        run.playing();
        assert!(run.send(output_error(1, AudioError::InUse)).is_empty());
        assert_eq!(
            run.audio.state(),
            AudioState::Held("Speakers (Realtek(R) Audio)".into())
        );
        assert_eq!(run.audio.tag(), Some(StateTag::InUse));
        assert_eq!(run.send(Event::TryAgain), [Command::Close, request(2, CARD_INPUT)]);
    }

    #[test]
    fn a_default_output_that_leaves_waits_for_the_next_default() {
        let mut run = Run::on(true);
        run.playing();
        assert!(run.send(output_error(1, AudioError::Gone)).is_empty());
        assert_eq!(run.audio.state(), AudioState::Opening);
        assert_eq!(run.audio.tag(), None);
        assert_eq!(
            run.send(Event::Monitor(AudioEvent::DefaultOutput(Some(HEADSET.into())))),
            [reopen_output(None)]
        );
        run.send(opened(1, Side::Output));
        assert_eq!(run.audio.state(), AudioState::Playing);
    }

    #[test]
    fn a_held_chosen_output_falls_back_and_says_so() {
        let mut run = Run::new();
        run.settings.audio.output = HEADSET.into();
        run.playing();
        assert_eq!(run.send(output_error(1, AudioError::InUse)), [reopen_output(None)]);
        run.send(opened(1, Side::Output));
        assert_eq!(run.audio.state(), AudioState::Playing);
        assert_eq!(run.audio.tag(), None, "the fallback has no tag");
        assert_eq!(
            top(&run),
            (Some(Label::Notice(Notice::Fallback("Headset (G533)".into()))), None)
        );
    }

    #[test]
    fn video_states_win_over_audio_states() {
        let mut run = Run::new();
        run.playing();
        run.send(input_error(1, AudioError::Failed("x".into())));
        assert!(run.audio.tag().is_some());
        // The video session ends (its own state shows): no audio state, no notice.
        assert!(run.send(Event::VideoClosed { session: 1 }).is_empty());
        assert_eq!(run.audio.state(), AudioState::Off);
        assert_eq!(run.audio.tag(), None);
        assert_eq!(top(&run), (None, None));
        assert!(run.send(Event::TryAgain).is_empty());
        // Blocked waits no more once the video is gone.
        run.set_access(Access::Denied);
        run.send(Event::VideoOpened {
            session: 2,
            device: card(),
        });
        assert!(run.audio.watches_access());
        run.send(Event::VideoClosed { session: 2 });
        assert!(!run.audio.watches_access());
    }

    fn opened_in(number: u64, side: Side, mode: StreamMode) -> Event {
        match opened(number, side) {
            Event::Session(SessionEvent::Opened {
                number,
                side,
                format,
                period_ms,
                reported_latency_ms,
                ..
            }) => Event::Session(SessionEvent::Opened {
                number,
                side,
                mode,
                format,
                period_ms,
                reported_latency_ms,
            }),
            _ => unreachable!(),
        }
    }

    impl Run {
        /// Stores an endpoint's exclusive box and sends the change, as the app does.
        fn set_exclusive(&mut self, id: &str, on: bool) -> Vec<Command> {
            self.settings.touch_audio_device(id, "").exclusive = on;
            self.send(Event::Exclusive(id.into()))
        }
    }

    #[test]
    fn the_session_asks_exclusive_for_the_checked_endpoints() {
        let mut run = Run::new();
        run.settings.touch_audio_device(CARD_INPUT, "").exclusive = true;
        // The system default output is asked exclusive by its endpoint's box.
        run.settings.touch_audio_device(SPEAKERS, "").exclusive = true;
        run.send(listed());
        let commands = run.send(Event::VideoOpened {
            session: 1,
            device: card(),
        });
        assert_eq!(
            commands,
            [Command::Open(SessionRequest {
                number: 1,
                input: Some(CARD_INPUT.into()),
                output: None,
                input_exclusive: true,
                output_exclusive: true,
            })]
        );
    }

    #[test]
    fn a_box_change_reopens_only_the_side_on_that_endpoint() {
        let mut run = Run::new();
        run.playing();
        assert_eq!(
            run.set_exclusive(CARD_INPUT, true),
            [Command::ReopenInput {
                input: CARD_INPUT.into(),
                exclusive: true,
            }]
        );
        run.send(opened_in(1, Side::Input, StreamMode::Exclusive));
        assert_eq!(run.audio.state(), AudioState::Playing);
        assert_eq!(
            run.audio.tag(),
            None,
            "exclusive is no problem: the menu row's tag says it"
        );
        // The output follows the default: the default's box reopens it.
        assert_eq!(
            run.set_exclusive(SPEAKERS, true),
            [Command::ReopenOutput {
                output: None,
                exclusive: true,
            }]
        );
        // An endpoint the preview does not use changes nothing.
        assert!(run.set_exclusive(MIC_INPUT, true).is_empty());
        assert!(run.set_exclusive(HEADSET, true).is_empty());
        // Unchecked: back to shared, the tag goes with it.
        run.send(opened_in(1, Side::Output, StreamMode::Shared));
        assert_eq!(
            run.set_exclusive(CARD_INPUT, false),
            [Command::ReopenInput {
                input: CARD_INPUT.into(),
                exclusive: false,
            }]
        );
        run.send(opened_in(1, Side::Input, StreamMode::Shared));
        assert_eq!(run.audio.tag(), None);
        // A chosen output opens with its own box, and its box reopens it.
        assert_eq!(
            run.pick_output(OutputChoice::Device(HEADSET.into())),
            [Command::ReopenOutput {
                output: Some(HEADSET.into()),
                exclusive: true,
            }]
        );
        run.send(opened(1, Side::Output));
        assert_eq!(
            run.set_exclusive(HEADSET, false),
            [Command::ReopenOutput {
                output: Some(HEADSET.into()),
                exclusive: false,
            }]
        );
    }

    #[test]
    fn no_box_change_reaches_a_closed_session() {
        let mut run = Run::new();
        run.send(listed());
        assert!(run.set_exclusive(CARD_INPUT, true).is_empty());
    }

    #[test]
    fn an_exclusive_side_refused_plays_shared_and_says_so() {
        let mut run = Run::new();
        run.settings.touch_audio_device(CARD_INPUT, "").exclusive = true;
        run.send(listed());
        run.send(Event::VideoOpened {
            session: 1,
            device: card(),
        });
        run.send(opened_in(1, Side::Input, StreamMode::SharedAfterInUse));
        assert_eq!(run.audio.state(), AudioState::Opening);
        run.send(opened(1, Side::Output));
        assert_eq!(
            run.audio.state(),
            AudioState::InUse("Digital Audio Interface (ezcap)".into())
        );
        assert_eq!(run.audio.tag(), Some(StateTag::InUse));
        assert_eq!(
            top(&run),
            (
                Some(Label::Notice(Notice::InUse("Digital Audio Interface (ezcap)".into()))),
                None
            ),
            "no action: exclusive is asked again at the next open"
        );
        // The next open asks exclusive again, and it is granted.
        assert_eq!(
            run.set_exclusive(CARD_INPUT, true),
            [Command::ReopenInput {
                input: CARD_INPUT.into(),
                exclusive: true,
            }]
        );
        run.send(opened_in(1, Side::Input, StreamMode::Exclusive));
        assert_eq!(run.audio.state(), AudioState::Playing);
        assert_eq!(
            run.audio.tag(),
            None,
            "exclusive is no problem: the menu row's tag says it"
        );
    }

    #[test]
    fn a_refused_output_names_the_default_it_plays_on() {
        let mut run = Run::new();
        run.settings.touch_audio_device(SPEAKERS, "").exclusive = true;
        run.send(listed());
        run.send(Event::VideoOpened {
            session: 1,
            device: card(),
        });
        run.send(opened(1, Side::Input));
        run.send(opened_in(1, Side::Output, StreamMode::SharedAfterInUse));
        assert_eq!(
            run.audio.state(),
            AudioState::InUse("Speakers (Realtek(R) Audio)".into())
        );
    }

    #[test]
    fn the_menu_tags_the_exclusive_endpoints() {
        let mut run = Run::new();
        run.settings.touch_audio_device(CARD_INPUT, "").exclusive = true;
        run.settings.touch_audio_device(SPEAKERS, "").exclusive = true;
        run.send(listed());
        run.send(Event::VideoOpened {
            session: 1,
            device: card(),
        });
        let menu = run.audio.menu(&run.settings);
        let tagged = |text: &Label| menu.iter().find(|i| &i.text == text).map(|i| i.exclusive);
        assert_eq!(
            tagged(&Label::Plain("Digital Audio Interface (ezcap)".into())),
            Some(true)
        );
        assert_eq!(
            tagged(&Label::Plain("Microphone (Realtek(R) Audio)".into())),
            Some(false)
        );
        assert_eq!(
            menu.iter()
                .find(|i| matches!(i.text, Label::Auto(_)))
                .map(|i| i.exclusive),
            Some(true),
            "Auto names the card"
        );
        assert_eq!(tagged(&Label::NoAudio), Some(false));
        assert_eq!(
            menu.iter()
                .find(|i| matches!(i.text, Label::SystemDefault(_)))
                .map(|i| i.exclusive),
            Some(true),
            "the default output is the speakers"
        );
        assert_eq!(tagged(&Label::Plain("Speakers (Realtek(R) Audio)".into())), Some(true));
    }

    #[test]
    fn the_settings_rows_mark_the_endpoints_in_use() {
        let mut run = Run::new();
        run.settings.touch_audio_device(MIC_INPUT, "").exclusive = true;
        run.send(listed());
        let row = |id: &str, name: &str, exclusive, in_use| ExclusiveRow {
            id: id.into(),
            name: name.into(),
            exclusive,
            in_use,
        };
        let (inputs, outputs) = run.audio.exclusive_rows(&run.settings);
        assert_eq!(
            inputs,
            [
                row(MIC_INPUT, "Microphone (Realtek(R) Audio)", true, false),
                row(CARD_INPUT, "Digital Audio Interface (ezcap)", false, false),
            ]
        );
        assert_eq!(outputs, [row(SPEAKERS, "Speakers (Realtek(R) Audio)", false, false)]);
        run.send(Event::VideoOpened {
            session: 1,
            device: card(),
        });
        run.send(opened(1, Side::Input));
        run.send(opened(1, Side::Output));
        let (inputs, outputs) = run.audio.exclusive_rows(&run.settings);
        assert_eq!(
            inputs,
            [
                row(CARD_INPUT, "Digital Audio Interface (ezcap)", false, true),
                row(MIC_INPUT, "Microphone (Realtek(R) Audio)", true, false),
            ],
            "the input in use comes first"
        );
        assert!(outputs[0].in_use, "the default output the preview plays on");
        run.send(Event::VideoClosed { session: 1 });
        let (inputs, outputs) = run.audio.exclusive_rows(&run.settings);
        assert!(inputs.iter().chain(&outputs).all(|r| !r.in_use));
        assert_eq!(inputs[0].id, MIC_INPUT, "the system's order again");
    }
}
