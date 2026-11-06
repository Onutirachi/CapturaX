//! What the video area shows and which device is open (data-model.md "Preview state", FR-019).
//! Pure: monitor events, session events, user choices and window visibility go in, commands for
//! the app come out. No GStreamer call, no settings write.

use capture::mode::{self, ModeChoice};
use capture::session::SessionEvent;
use capture::{Device, Mode, MonitorEvent, RangeChoice};
use settings::{ColorRange, Settings};

/// What the video area shows; one at a time. The string is the device id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreviewState {
    /// No capture device is connected.
    NoDevices,
    Opening(String),
    Playing,
    Blocked(String),
    InUse(String),
    Disconnected(String),
    NoSignal(String),
    Failed(String),
    /// Devices exist but none is open (the shell's "No source"); not reached at start.
    NoSource,
}

/// What goes into [`Preview::handle`].
#[derive(Debug, Clone)]
pub enum Event {
    Monitor(MonitorEvent),
    /// A session's event, tagged with its session number.
    Session {
        session: u64,
        event: SessionEvent,
    },
    /// The user picked a device.
    ChooseDevice(String),
    /// The user pressed Try again.
    TryAgain,
    /// The window went to the tray: the device is closed.
    Hidden,
    /// The window came back from the tray.
    Shown,
}

/// What the app does for the preview.
#[derive(Debug, Clone)]
pub enum Command {
    /// Close the current session (if any), then open this one.
    Open {
        device: Device,
        mode: Mode,
        range: RangeChoice,
        session: u64,
    },
    /// Close the current session.
    Close,
    /// Remember the device the user chose: `preview.device`, `preview.recent` and its entry.
    SaveChoice { id: String, name: String },
    /// The state changed: show [`Preview::state`].
    ShowState,
}

/// The open (or opening) session.
#[derive(Debug, Clone)]
struct Open {
    device: String,
    session: u64,
    /// Opened because the user chose it (saved when it plays), not a fallback.
    chosen: bool,
}

#[derive(Debug, Default)]
pub struct Preview {
    state: Option<PreviewState>,
    devices: Vec<Device>,
    /// The monitor's first list arrived.
    listed: bool,
    open: Option<Open>,
    /// Session numbers handed out so far.
    sessions: u64,
    hidden: bool,
    /// The device to open again when the window is shown, and whether the user had chosen it.
    resume: Option<(String, bool)>,
}

impl Preview {
    pub fn new() -> Self {
        Self::default()
    }

    /// What the video area shows. Before the monitor's first list: opening nothing yet, shown as
    /// [`PreviewState::NoSource`].
    pub fn state(&self) -> PreviewState {
        self.state.clone().unwrap_or(PreviewState::NoSource)
    }

    /// The current session number, if a device is open or opening; frames of other sessions are
    /// dropped.
    pub fn session(&self) -> Option<u64> {
        self.open.as_ref().map(|o| o.session)
    }

    /// The device open or opening.
    pub fn device(&self) -> Option<&Device> {
        let id = &self.open.as_ref()?.device;
        capture::device::find(id, &self.devices)
    }

    /// The devices listed now.
    pub fn devices(&self) -> &[Device] {
        &self.devices
    }

    pub fn handle(&mut self, event: Event, settings: &Settings) -> Vec<Command> {
        let mut out = Vec::new();
        let before = self.state.clone();
        match event {
            Event::Monitor(MonitorEvent::Listed(devices)) => {
                self.devices = devices;
                self.listed = true;
                self.start(settings, &mut out);
            }
            Event::Monitor(MonitorEvent::Added(device)) => {
                self.devices.retain(|d| d.id != device.id);
                self.devices.push(device);
                capture::device::numbered(&mut self.devices);
                if self.listed && self.state == Some(PreviewState::NoDevices) {
                    self.start(settings, &mut out);
                }
            }
            Event::Monitor(MonitorEvent::Removed(id)) => {
                self.devices.retain(|d| d.id != id);
                capture::device::numbered(&mut self.devices);
                if self.open.as_ref().is_some_and(|o| o.device == id) {
                    self.open = None;
                    out.push(Command::Close);
                    self.state = Some(PreviewState::Disconnected(id));
                } else if self.resume.as_ref().is_some_and(|(r, _)| *r == id) {
                    self.resume = None;
                    self.state = Some(PreviewState::Disconnected(id));
                } else if self.devices.is_empty() && self.open.is_none() {
                    self.state = Some(PreviewState::NoDevices);
                }
            }
            Event::Session { session, event } => self.session_event(session, event, &mut out),
            Event::ChooseDevice(id) => {
                if self.hidden {
                    self.resume = Some((id, true));
                } else {
                    self.open_device(&id, true, settings, &mut out);
                }
            }
            Event::TryAgain => {
                if let Some(id) = self.failed_device()
                    && !self.hidden
                {
                    self.open_device(&id, false, settings, &mut out);
                }
            }
            Event::Hidden => {
                if !self.hidden {
                    self.hidden = true;
                    if let Some(open) = self.open.take() {
                        self.resume = Some((open.device, open.chosen));
                        out.push(Command::Close);
                    }
                }
            }
            Event::Shown => {
                if self.hidden {
                    self.hidden = false;
                    if let Some((id, chosen)) = self.resume.take() {
                        self.open_device(&id, chosen, settings, &mut out);
                    } else if self.listed && self.state == Some(PreviewState::NoDevices) {
                        self.start(settings, &mut out);
                    }
                }
            }
        }
        if self.state != before {
            out.push(Command::ShowState);
        }
        out
    }

    /// Opens the start device (FR-005), or shows that there is none.
    fn start(&mut self, settings: &Settings, out: &mut Vec<Command>) {
        let chosen = capture::device::start_order(
            &self.devices,
            settings.preview.device.as_deref(),
            &settings.preview.recent,
        )
        .map(|d| d.id.clone());
        match chosen {
            None => self.state = Some(PreviewState::NoDevices),
            Some(id) if self.hidden => self.resume = Some((id, false)),
            Some(id) => self.open_device(&id, false, settings, out),
        }
    }

    fn open_device(&mut self, id: &str, chosen: bool, settings: &Settings, out: &mut Vec<Command>) {
        let Some(device) = capture::device::find(id, &self.devices).cloned() else {
            log::warn!("preview: device {id} is not listed");
            return;
        };
        self.sessions += 1;
        let session = self.sessions;
        self.open = Some(Open {
            device: device.id.clone(),
            session,
            chosen,
        });
        let stored = settings.device(&device.id);
        let Some(mode) = mode_to_open(&device, stored.map(|c| c.mode.as_str())) else {
            log::error!("preview: {} ({}) offers no mode", device.name, device.id);
            self.open = None;
            self.state = Some(PreviewState::Failed(device.id));
            return;
        };
        let range = range_choice(stored.map(|c| c.color_range).unwrap_or_default());
        self.state = Some(PreviewState::Opening(device.id.clone()));
        out.push(Command::Open {
            device,
            mode,
            range,
            session,
        });
    }

    fn session_event(&mut self, session: u64, event: SessionEvent, out: &mut Vec<Command>) {
        let Some(open) = self.open.as_mut().filter(|o| o.session == session) else {
            log::debug!("preview: event of stale session {session} ignored: {event:?}");
            return;
        };
        match event {
            SessionEvent::FirstFrame => {
                self.state = Some(PreviewState::Playing);
                if open.chosen {
                    open.chosen = false;
                    if let Some(device) = capture::device::find(&open.device, &self.devices) {
                        out.push(Command::SaveChoice {
                            id: device.id.clone(),
                            name: device.base_name.clone(),
                        });
                    }
                }
            }
            SessionEvent::Error(error) => {
                let id = open.device.clone();
                log::warn!("preview: session {session} on {id} failed: {error:?}");
                self.open = None;
                out.push(Command::Close);
                // Every error is Failed for now; US4 maps each to its own state.
                self.state = Some(PreviewState::Failed(id));
            }
            SessionEvent::Playing { .. } | SessionEvent::Range(_) => {}
        }
    }

    /// The device of a state that offers Try again.
    fn failed_device(&self) -> Option<String> {
        match self.state.as_ref()? {
            PreviewState::Blocked(id)
            | PreviewState::InUse(id)
            | PreviewState::NoSignal(id)
            | PreviewState::Failed(id) => Some(id.clone()),
            _ => None,
        }
    }
}

/// The mode a device opens in: its stored choice resolved, else the default automatic mode
/// (unknown, missing or no longer offered; the stored text is not replaced, contracts
/// settings-file.md rule 2).
pub fn mode_to_open(device: &Device, stored: Option<&str>) -> Option<Mode> {
    let signal = device.signal;
    stored
        .and_then(ModeChoice::parse)
        .and_then(|choice| mode::resolve(&choice, &device.modes, signal))
        .or_else(|| mode::resolve(&mode::default_choice(signal), &device.modes, signal))
}

pub fn range_choice(stored: ColorRange) -> RangeChoice {
    match stored {
        ColorRange::Auto => RangeChoice::Auto,
        ColorRange::Full => RangeChoice::Full,
        ColorRange::Limited => RangeChoice::Limited,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use capture::mode::Format;
    use capture::session::DeviceError;

    fn mode(width: u32, height: u32, fps: i32) -> Mode {
        Mode {
            width,
            height,
            fps: (fps, 1),
            format: Format::Nv12,
        }
    }

    fn card() -> Device {
        Device::described(
            "card",
            "ezcap Live GAMER ULTRA",
            vec![mode(1920, 1080, 120), mode(1920, 1080, 60), mode(1280, 720, 60)],
        )
    }

    fn webcam() -> Device {
        Device::described(
            "cam",
            "HD Pro Webcam C920",
            vec![mode(1920, 1080, 30), mode(640, 480, 30)],
        )
    }

    fn listed(devices: Vec<Device>) -> Event {
        Event::Monitor(MonitorEvent::Listed(devices))
    }

    fn first_frame(session: u64) -> Event {
        Event::Session {
            session,
            event: SessionEvent::FirstFrame,
        }
    }

    /// The (device id, mode, session) of the commands' Open, if any.
    fn opened(commands: &[Command]) -> Option<(String, Mode, u64)> {
        commands.iter().find_map(|c| match c {
            Command::Open {
                device, mode, session, ..
            } => Some((device.id.clone(), mode.clone(), *session)),
            _ => None,
        })
    }

    fn has(commands: &[Command], f: impl Fn(&Command) -> bool) -> bool {
        commands.iter().any(f)
    }

    fn closes(commands: &[Command]) -> bool {
        has(commands, |c| matches!(c, Command::Close))
    }

    fn saves(commands: &[Command]) -> bool {
        has(commands, |c| matches!(c, Command::SaveChoice { .. }))
    }

    #[test]
    fn start_without_devices_then_one_is_added() {
        let settings = Settings::default();
        let mut p = Preview::new();
        assert_eq!(p.state(), PreviewState::NoSource);
        let out = p.handle(listed(vec![]), &settings);
        assert_eq!(p.state(), PreviewState::NoDevices);
        assert!(has(&out, |c| matches!(c, Command::ShowState)));
        assert!(opened(&out).is_none());

        let out = p.handle(Event::Monitor(MonitorEvent::Added(card())), &settings);
        assert_eq!(opened(&out).map(|o| o.0).as_deref(), Some("card"));
        assert_eq!(p.state(), PreviewState::Opening("card".into()));
    }

    #[test]
    fn start_opens_the_start_order_device_in_the_default_mode() {
        let settings = Settings::default();
        let mut p = Preview::new();
        let out = p.handle(listed(vec![webcam(), card()]), &settings);
        let (id, mode, session) = opened(&out).unwrap();
        assert_eq!(id, "card");
        // No stored choice: Auto max FPS.
        assert_eq!(mode, self::mode(1920, 1080, 120));
        assert_eq!(p.session(), Some(session));
        assert_eq!(p.state(), PreviewState::Opening("card".into()));
    }

    #[test]
    fn start_uses_the_last_device_and_its_stored_mode_and_range() {
        let mut settings = Settings::default();
        settings.preview.device = Some("cam".into());
        let entry = settings.touch_device("cam", "HD Pro Webcam C920");
        entry.mode = "640x480@30".into();
        entry.color_range = ColorRange::Full;
        let mut p = Preview::new();
        let out = p.handle(listed(vec![card(), webcam()]), &settings);
        let Some(Command::Open {
            device, mode, range, ..
        }) = out.iter().find(|c| matches!(c, Command::Open { .. }))
        else {
            panic!("no open: {out:?}");
        };
        assert_eq!(device.id, "cam");
        assert_eq!(*mode, self::mode(640, 480, 30));
        assert_eq!(*range, RangeChoice::Full);
    }

    #[test]
    fn unknown_or_missing_stored_mode_opens_the_default() {
        let device = card();
        let default = Some(mode(1920, 1080, 120));
        assert_eq!(mode_to_open(&device, None), default);
        assert_eq!(mode_to_open(&device, Some("rubbish")), default);
        assert_eq!(mode_to_open(&device, Some("3840x2160@60")), default);
        assert_eq!(mode_to_open(&device, Some("1280x720@60")), Some(mode(1280, 720, 60)));
        assert_eq!(
            mode_to_open(&device, Some("auto-max-resolution")),
            Some(mode(1920, 1080, 60))
        );
        assert_eq!(mode_to_open(&Device::described("x", "Empty", vec![]), None), None);
    }

    #[test]
    fn first_frame_plays_and_a_fallback_is_not_saved() {
        let mut settings = Settings::default();
        settings.preview.device = Some("gone".into());
        let mut p = Preview::new();
        let out = p.handle(listed(vec![card()]), &settings);
        let session = opened(&out).unwrap().2;
        let out = p.handle(first_frame(session), &settings);
        assert_eq!(p.state(), PreviewState::Playing);
        assert!(!saves(&out));
    }

    #[test]
    fn a_user_choice_is_saved_when_it_plays() {
        let settings = Settings::default();
        let mut p = Preview::new();
        p.handle(listed(vec![card(), webcam()]), &settings);
        let out = p.handle(Event::ChooseDevice("cam".into()), &settings);
        let (id, _, session) = opened(&out).unwrap();
        assert_eq!(id, "cam");
        assert!(!saves(&out));
        let out = p.handle(first_frame(session), &settings);
        let saved = out.iter().find_map(|c| match c {
            Command::SaveChoice { id, name } => Some((id.clone(), name.clone())),
            _ => None,
        });
        assert_eq!(saved, Some(("cam".into(), "HD Pro Webcam C920".into())));
        // Only once.
        let again = p.handle(first_frame(session), &settings);
        assert!(!saves(&again));
    }

    #[test]
    fn a_session_error_fails_and_try_again_reopens() {
        let settings = Settings::default();
        let mut p = Preview::new();
        let out = p.handle(listed(vec![card()]), &settings);
        let session = opened(&out).unwrap().2;
        let out = p.handle(
            Event::Session {
                session,
                event: SessionEvent::Error(DeviceError::Failed("boom".into())),
            },
            &settings,
        );
        assert!(closes(&out));
        assert_eq!(p.state(), PreviewState::Failed("card".into()));
        assert_eq!(p.session(), None);

        let out = p.handle(Event::TryAgain, &settings);
        let (id, _, new_session) = opened(&out).unwrap();
        assert_eq!(id, "card");
        assert!(new_session > session);
        assert_eq!(p.state(), PreviewState::Opening("card".into()));
    }

    #[test]
    fn stale_sessions_are_ignored() {
        let settings = Settings::default();
        let mut p = Preview::new();
        let first = opened(&p.handle(listed(vec![card(), webcam()]), &settings)).unwrap().2;
        let second = opened(&p.handle(Event::ChooseDevice("cam".into()), &settings))
            .unwrap()
            .2;
        assert_ne!(first, second);
        let out = p.handle(first_frame(first), &settings);
        assert!(out.is_empty());
        assert_eq!(p.state(), PreviewState::Opening("cam".into()));
        let out = p.handle(
            Event::Session {
                session: first,
                event: SessionEvent::Error(DeviceError::Gone),
            },
            &settings,
        );
        assert!(out.is_empty());
        assert_eq!(p.state(), PreviewState::Opening("cam".into()));
    }

    #[test]
    fn hidden_to_tray_closes_and_shown_opens_again() {
        let settings = Settings::default();
        let mut p = Preview::new();
        let session = opened(&p.handle(listed(vec![card()]), &settings)).unwrap().2;
        p.handle(first_frame(session), &settings);

        let out = p.handle(Event::Hidden, &settings);
        assert!(closes(&out));
        assert_eq!(p.session(), None);
        // Hidden twice does nothing more.
        assert!(p.handle(Event::Hidden, &settings).is_empty());

        let out = p.handle(Event::Shown, &settings);
        let (id, _, new_session) = opened(&out).unwrap();
        assert_eq!(id, "card");
        assert!(new_session > session);
        assert_eq!(p.state(), PreviewState::Opening("card".into()));
        assert!(p.handle(Event::Shown, &settings).is_empty());
    }

    #[test]
    fn a_user_choice_survives_the_tray() {
        let settings = Settings::default();
        let mut p = Preview::new();
        p.handle(listed(vec![card(), webcam()]), &settings);
        let session = opened(&p.handle(Event::ChooseDevice("cam".into()), &settings))
            .unwrap()
            .2;
        // Hidden before it played: the choice is still saved once it plays after showing.
        p.handle(Event::Hidden, &settings);
        assert!(p.handle(first_frame(session), &settings).is_empty());
        let session = opened(&p.handle(Event::Shown, &settings)).unwrap().2;
        assert!(saves(&p.handle(first_frame(session), &settings)));
    }

    #[test]
    fn removing_the_open_device_disconnects() {
        let settings = Settings::default();
        let mut p = Preview::new();
        p.handle(listed(vec![card(), webcam()]), &settings);
        let out = p.handle(
            Event::Monitor(MonitorEvent::Removed("webcam-not-open".into())),
            &settings,
        );
        assert!(out.is_empty());
        let out = p.handle(Event::Monitor(MonitorEvent::Removed("card".into())), &settings);
        assert!(closes(&out));
        assert_eq!(p.state(), PreviewState::Disconnected("card".into()));
        assert_eq!(p.devices().len(), 1);
    }
}
