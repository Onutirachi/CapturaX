//! What the video area shows and which device is open (data-model.md "Preview state", FR-019).
//! Pure: monitor events, session events, user choices and window visibility go in, commands for
//! the app come out. No GStreamer call, no settings write.

use capture::mode::{self, ModeChoice, Tags};
use capture::session::{DeviceError, SessionEvent};
use capture::{Access, Device, Mode, MonitorEvent, Range, RangeChoice};
use settings::{ColorRange, Settings};

/// What the video area shows; one at a time. The string is the device id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreviewState {
    /// No capture device is connected.
    NoDevices,
    Opening(String),
    Playing,
    /// The system blocks access: a device that cannot be opened, or none listed while camera
    /// access is denied (Windows, research R6); on Linux the node that could not be opened.
    Blocked {
        device: Option<String>,
        node: Option<String>,
    },
    InUse(String),
    Disconnected(String),
    NoSignal(String),
    Failed(String),
    /// Devices exist but none is open (the shell's "No source"); not reached at start.
    NoSource,
}

/// What goes into [`Preview::handle`].
#[allow(dead_code)] // TryAgain comes with T045.
#[derive(Debug, Clone)]
pub enum Event {
    Monitor(MonitorEvent),
    /// The devices listed now, read again ([`Command::Relist`], or the 3 s re-read while
    /// [`Preview::waiting`], research R1, with `timer` set): what the monitor missed is added or
    /// removed.
    Relisted {
        devices: Vec<Device>,
        timer: bool,
    },
    /// The system woke from sleep: the open device opens again as after a reconnect (FR-014), and
    /// so does the device of an error state (its error came before the wake was noticed).
    Resumed,
    /// A session's event, tagged with its session number.
    Session {
        session: u64,
        event: SessionEvent,
    },
    /// The user picked a device.
    ChooseDevice(String),
    /// The user picked a mode for the open device.
    ChooseMode(ModeChoice),
    /// The user picked a color range for the open device.
    ChooseRange(RangeChoice),
    /// The user pressed Try again.
    TryAgain,
    /// The window went to the tray: the device is closed.
    Hidden,
    /// The window came back from the tray.
    Shown,
}

impl Event {
    /// A short description for the log line of a state change (the event's reason).
    pub fn reason(&self) -> String {
        match self {
            Self::Monitor(MonitorEvent::Listed(devices)) => format!("first list, {} devices", devices.len()),
            Self::Monitor(MonitorEvent::Added(device)) => format!("{} added", device.id),
            Self::Monitor(MonitorEvent::Removed(id)) => format!("{id} removed"),
            Self::Relisted { devices, timer } => format!(
                "list read again{}, {} devices",
                if *timer { " (3 s timer)" } else { "" },
                devices.len()
            ),
            Self::Resumed => "woke from sleep".into(),
            Self::Session { session, event } => format!("session {session}: {event:?}"),
            Self::ChooseDevice(id) => format!("{id} chosen"),
            Self::ChooseMode(choice) => format!("mode {choice} chosen"),
            Self::ChooseRange(range) => format!("color range {range:?} chosen"),
            Self::TryAgain => "try again".into(),
            Self::Hidden => "window hidden".into(),
            Self::Shown => "window shown".into(),
        }
    }
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
    /// Read the device list again and hand it back as [`Event::Relisted`] (after a session error:
    /// the running monitor may still list a device that left).
    Relist,
    /// Remember the device the user chose: `preview.device`, `preview.recent` and its entry.
    SaveChoice { id: String, name: String },
    /// Store a device's mode choice (`devices."<id>".mode`, FR-009).
    SaveMode {
        id: String,
        name: String,
        choice: ModeChoice,
    },
    /// Apply a color range choice to the open session (no restart, FR-010).
    SetRange(RangeChoice),
    /// Store a device's color range choice (`devices."<id>".color_range`).
    SaveRange {
        id: String,
        name: String,
        range: RangeChoice,
    },
    /// The color range choice or the range in use changed: show [`Preview::range`].
    ShowRange,
    /// The state changed: show [`Preview::state`].
    ShowState,
}

/// A device to open again in the mode it had: after the tray, Try again or a disconnect.
#[derive(Debug, Clone)]
struct Again {
    device: String,
    /// `None` when unknown (opened again in its stored choice).
    mode: Option<Mode>,
    /// Chosen by the user: saved once it plays.
    chosen: bool,
}

impl Again {
    fn of(open: &Open) -> Self {
        Self {
            device: open.device.clone(),
            mode: Some(open.mode.clone()),
            chosen: open.chosen,
        }
    }
}

/// Reads whether the system lets the app use cameras (`platform::camera_access()` in the app).
struct AccessReader(Box<dyn Fn() -> Access + Send>);

impl Default for AccessReader {
    fn default() -> Self {
        Self(Box::new(|| Access::Unknown))
    }
}

impl std::fmt::Debug for AccessReader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("AccessReader")
    }
}

/// The open (or opening) session.
#[derive(Debug, Clone)]
struct Open {
    device: String,
    session: u64,
    /// The mode it was opened in.
    mode: Mode,
    /// Opened because the user chose it (saved when it plays), not a fallback.
    chosen: bool,
    range: RangeChoice,
    /// The range frames are converted with, once the session reports it.
    in_use: Option<Range>,
}

#[derive(Debug, Default)]
pub struct Preview {
    state: Option<PreviewState>,
    devices: Vec<Device>,
    /// The name of every device seen in this run, by id: a state names its device also after it
    /// left.
    names: Vec<(String, String)>,
    /// The monitor's first list arrived.
    listed: bool,
    open: Option<Open>,
    /// Session numbers handed out so far.
    sessions: u64,
    hidden: bool,
    /// The device to open again when the window is shown.
    resume: Option<Again>,
    /// The device of an error or disconnected state: Try again, or its return, opens it again.
    again: Option<Again>,
    /// The device was opened again after a wake: until it plays, its errors are a disconnect.
    woke: bool,
    access: AccessReader,
}

impl Preview {
    #[cfg(test)]
    pub fn new() -> Self {
        Self::default()
    }

    /// A preview that reads camera access with `access` (only when a state depends on it).
    pub fn with_access(access: impl Fn() -> Access + Send + 'static) -> Self {
        Self {
            access: AccessReader(Box::new(access)),
            ..Self::default()
        }
    }

    /// Waiting for a device to come (no devices, the open one disconnected, or blocked while it
    /// is not listed): the app reads the list again every 3 s meanwhile (research R1).
    pub fn waiting(&self) -> bool {
        match self.state.as_ref() {
            Some(PreviewState::NoDevices | PreviewState::Disconnected(_)) => true,
            Some(PreviewState::Blocked { device, .. }) => device
                .as_ref()
                .is_none_or(|id| capture::device::find(id, &self.devices).is_none()),
            _ => false,
        }
    }

    /// What the video area shows. Before the monitor's first list: opening nothing yet, shown as
    /// [`PreviewState::NoSource`].
    pub fn state(&self) -> PreviewState {
        self.state.clone().unwrap_or(PreviewState::NoSource)
    }

    /// The current session number, if a device is open or opening; frames of other sessions are
    /// dropped.
    #[cfg(test)]
    pub fn session(&self) -> Option<u64> {
        self.open.as_ref().map(|o| o.session)
    }

    /// The device open or opening.
    pub fn device(&self) -> Option<&Device> {
        let id = &self.open.as_ref()?.device;
        capture::device::find(id, &self.devices)
    }

    /// The name shown for a device seen in this run; its id when never seen.
    pub fn name(&self, id: &str) -> String {
        self.names
            .iter()
            .find(|(known, _)| known == id)
            .map_or_else(|| id.to_string(), |(_, name)| name.clone())
    }

    /// Keeps the names of the devices listed now.
    fn learn_names(&mut self) {
        for device in &self.devices {
            match self.names.iter_mut().find(|(id, _)| *id == device.id) {
                Some(entry) => entry.1 = device.name.clone(),
                None => self.names.push((device.id.clone(), device.name.clone())),
            }
        }
    }

    /// The open device's color range choice and the range in use (once known); Auto with no
    /// device.
    pub fn range(&self) -> (RangeChoice, Option<Range>) {
        self.open
            .as_ref()
            .map_or((RangeChoice::Auto, None), |o| (o.range, o.in_use))
    }

    /// The devices listed now.
    #[cfg(test)]
    pub fn devices(&self) -> &[Device] {
        &self.devices
    }

    /// The device menu and the open device's mode menu (empty with no open device).
    pub fn menus(&self, settings: &Settings) -> (Vec<Item>, Vec<Item>) {
        let open = self.device();
        let devices = device_items(&self.devices, open.map(|d| d.id.as_str()), settings);
        let modes = open.map_or_else(Vec::new, |device| {
            let stored = settings.device(&device.id).map(|c| c.mode.as_str());
            mode_items(device, &mode_choice(device, stored))
        });
        (devices, modes)
    }

    pub fn handle(&mut self, event: Event, settings: &Settings) -> Vec<Command> {
        let mut out = Vec::new();
        let before = self.state.clone();
        if let Event::Monitor(_) | Event::Relisted { .. } = &event {
            // Learnt after the change too (renumbering); before it for a device about to leave.
            self.learn_names();
        }
        match event {
            Event::Monitor(MonitorEvent::Listed(devices)) => {
                self.devices = devices;
                self.listed = true;
                self.start(settings, &mut out);
            }
            Event::Monitor(MonitorEvent::Added(device)) => {
                let id = device.id.clone();
                self.devices.retain(|d| d.id != id);
                self.devices.push(device);
                capture::device::numbered(&mut self.devices);
                self.came(&id, settings, &mut out);
            }
            Event::Monitor(MonitorEvent::Removed(id)) => {
                self.devices.retain(|d| d.id != id);
                capture::device::numbered(&mut self.devices);
                self.left(&id, &mut out);
            }
            Event::Relisted { devices, timer } => self.relisted(devices, timer, settings, &mut out),
            Event::Resumed => {
                if let Some(open) = &self.open {
                    log::info!("preview: woke from sleep, opening {} again", open.device);
                    self.again = Some(Again::of(open));
                    self.woke = true;
                    self.reopen(settings, &mut out);
                } else if !self.hidden
                    && matches!(
                        self.state,
                        Some(PreviewState::InUse(_) | PreviewState::NoSignal(_) | PreviewState::Failed(_))
                    )
                    && let Some(again) = &self.again
                {
                    // The session failed in the second before the wake was noticed: an error
                    // after the wake, as above.
                    log::info!(
                        "preview: woke from sleep, opening {} again after its error",
                        again.device
                    );
                    self.woke = true;
                    self.reopen(settings, &mut out);
                }
            }
            Event::Session { session, event } => self.session_event(session, event, &mut out),
            Event::ChooseDevice(id) => {
                if self.open.as_ref().is_some_and(|o| o.device == id) {
                    log::info!("preview: {id} is already open");
                } else if self.hidden {
                    // No longer waiting for another device either.
                    self.again = None;
                    self.resume = Some(Again {
                        device: id,
                        mode: None,
                        chosen: true,
                    });
                } else {
                    self.open_device(&id, true, None, settings, &mut out);
                }
            }
            Event::ChooseMode(choice) => self.choose_mode(choice, settings, &mut out),
            Event::ChooseRange(range) => {
                if let Some(open) = self.open.as_mut()
                    && open.range != range
                {
                    open.range = range;
                    out.push(Command::SetRange(range));
                    out.push(Command::SaveRange {
                        id: open.device.clone(),
                        name: capture::device::find(&open.device, &self.devices)
                            .map_or_else(String::new, |d| d.base_name.clone()),
                        range,
                    });
                    out.push(Command::ShowRange);
                }
            }
            Event::TryAgain => self.try_again(settings, &mut out),
            Event::Hidden => {
                if !self.hidden {
                    self.hidden = true;
                    if let Some(open) = self.open.take() {
                        self.resume = Some(Again::of(&open));
                        out.push(Command::Close);
                    }
                }
            }
            Event::Shown => {
                if self.hidden {
                    self.hidden = false;
                    if let Some(again) = self.resume.take() {
                        let mode = self.offered(&again);
                        self.open_device(&again.device, again.chosen, mode, settings, &mut out);
                    } else if self.listed && self.none_listed_state() {
                        self.start(settings, &mut out);
                    }
                }
            }
        }
        self.learn_names();
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
            None => self.none_listed(),
            Some(id) if self.hidden => {
                self.resume = Some(Again {
                    device: id,
                    mode: None,
                    chosen: false,
                });
            }
            Some(id) => self.open_device(&id, false, None, settings, out),
        }
    }

    /// No device is listed: "No devices", or Blocked when camera access is denied (on Windows a
    /// denied device is not listed at all, research R6).
    fn none_listed(&mut self) {
        self.state = Some(if (self.access.0)() == Access::Denied {
            PreviewState::Blocked {
                device: None,
                node: None,
            }
        } else {
            PreviewState::NoDevices
        });
    }

    /// The state says that no device is listed (No devices, or Blocked without a device).
    fn none_listed_state(&self) -> bool {
        matches!(
            self.state,
            Some(PreviewState::NoDevices | PreviewState::Blocked { device: None, .. })
        )
    }

    /// The device an error or disconnected state names.
    fn state_device(&self) -> Option<&str> {
        match self.state.as_ref()? {
            PreviewState::Blocked { device, .. } => device.as_deref(),
            PreviewState::InUse(id)
            | PreviewState::Disconnected(id)
            | PreviewState::NoSignal(id)
            | PreviewState::Failed(id) => Some(id),
            _ => None,
        }
    }

    /// A device came (from the monitor or a re-read): the device a state waits for opens again
    /// in the same mode (F4); with none listed before, the start device opens; any other only
    /// appears in the menu (spec edge case).
    fn came(&mut self, id: &str, settings: &Settings, out: &mut Vec<Command>) {
        if !self.listed {
            return;
        }
        let waited = matches!(
            &self.state,
            Some(PreviewState::Disconnected(w) | PreviewState::Blocked { device: Some(w), .. }) if w == id
        );
        if waited {
            log::info!("preview: {id} is back");
            self.reopen(settings, out);
        } else if self.none_listed_state() {
            self.start(settings, out);
        }
    }

    /// A device left: the open one (or the one to open when shown) disconnects, or shows Blocked
    /// when camera access is now denied (spec edge case); so does the device of an error state;
    /// with none left and nothing waited for, "No devices".
    fn left(&mut self, id: &str, out: &mut Vec<Command>) {
        if let Some(open) = self.open.take_if(|o| o.device == id) {
            out.push(Command::Close);
            self.again = Some(Again::of(&open));
            self.state = Some(self.lost(id));
        } else if let Some(again) = self.resume.take_if(|r| r.device == id) {
            self.again = Some(again);
            self.state = Some(self.lost(id));
        } else if self.state_device() == Some(id) {
            if !matches!(self.state, Some(PreviewState::Disconnected(_))) {
                self.state = Some(self.lost(id));
            }
        } else if self.devices.is_empty() && self.open.is_none() && self.state_device().is_none() {
            self.none_listed();
        }
    }

    /// The state of a device that left: Blocked when camera access is denied, else Disconnected.
    fn lost(&self, id: &str) -> PreviewState {
        if (self.access.0)() == Access::Denied {
            PreviewState::Blocked {
                device: Some(id.to_string()),
                node: None,
            }
        } else {
            PreviewState::Disconnected(id.to_string())
        }
    }

    /// The list read again: what the monitor missed is added or removed; on the timer, a
    /// disconnected device still (or again) listed opens again (after a wake it may never have
    /// left); with none listed, camera access is read again.
    fn relisted(&mut self, devices: Vec<Device>, timer: bool, settings: &Settings, out: &mut Vec<Command>) {
        let gone: Vec<String> = self
            .devices
            .iter()
            .filter(|d| capture::device::find(&d.id, &devices).is_none())
            .map(|d| d.id.clone())
            .collect();
        let new: Vec<String> = devices
            .iter()
            .filter(|d| capture::device::find(&d.id, &self.devices).is_none())
            .map(|d| d.id.clone())
            .collect();
        self.devices = devices;
        self.learn_names();
        for id in &gone {
            log::info!("preview: {id} is no longer listed");
            self.left(id, out);
        }
        for id in &new {
            log::info!("preview: {id} is listed now");
            self.came(id, settings, out);
        }
        if timer
            && let Some(PreviewState::Disconnected(id)) = &self.state
            && capture::device::find(id, &self.devices).is_some()
        {
            log::info!("preview: {id} is listed, opening it again");
            self.reopen(settings, out);
        }
        if self.listed && self.devices.is_empty() && self.none_listed_state() {
            self.none_listed();
        }
    }

    /// Opens the device of `again` in its mode (its stored choice when it no longer offers that
    /// mode); while hidden, it opens when the window is shown.
    fn reopen(&mut self, settings: &Settings, out: &mut Vec<Command>) {
        let Some(again) = self.again.clone() else {
            return;
        };
        if self.hidden {
            self.resume = Some(again);
            return;
        }
        let mode = self.offered(&again);
        self.open_device(&again.device, again.chosen, mode, settings, out);
    }

    /// The mode of `again` when its device still offers it.
    fn offered(&self, again: &Again) -> Option<Mode> {
        let device = capture::device::find(&again.device, &self.devices)?;
        again.mode.clone().filter(|m| device.modes.contains(m))
    }

    /// Try again: the device of an error state opens again in the same mode; one no longer
    /// listed disconnects (or stays Blocked while access is denied); Blocked without a device
    /// goes through the start order again (list and access read again).
    fn try_again(&mut self, settings: &Settings, out: &mut Vec<Command>) {
        if self.hidden {
            return;
        }
        let id = match self.state.as_ref() {
            Some(PreviewState::Blocked { device: None, .. }) => {
                self.start(settings, out);
                return;
            }
            Some(
                PreviewState::Blocked { .. }
                | PreviewState::InUse(_)
                | PreviewState::NoSignal(_)
                | PreviewState::Failed(_),
            ) => self.state_device().map(str::to_string),
            _ => None,
        };
        let Some(id) = id else { return };
        if capture::device::find(&id, &self.devices).is_none() {
            self.state = Some(self.lost(&id));
            return;
        }
        if self.again.as_ref().is_none_or(|a| a.device != id) {
            self.again = Some(Again {
                device: id,
                mode: None,
                chosen: false,
            });
        }
        self.reopen(settings, out);
    }

    /// The user picked a mode for the open device: stored when it differs from the stored text
    /// (a stored mode the device lacks is replaced only here, contracts settings-file.md rule 2),
    /// and the device reopened when it resolves to another mode (FR-009).
    fn choose_mode(&mut self, choice: ModeChoice, settings: &Settings, out: &mut Vec<Command>) {
        let Some(open) = &self.open else {
            log::info!("preview: mode {choice} chosen with no open device");
            return;
        };
        let Some(device) = capture::device::find(&open.device, &self.devices).cloned() else {
            return;
        };
        let Some(mode) = mode::resolve(&choice, &device.modes, device.signal) else {
            log::warn!("preview: {} cannot open {choice}", device.name);
            return;
        };
        let stored = settings.device(&device.id).and_then(|c| ModeChoice::parse(&c.mode));
        if stored.as_ref() != Some(&choice) {
            out.push(Command::SaveMode {
                id: device.id.clone(),
                name: device.base_name.clone(),
                choice,
            });
        }
        if mode == open.mode {
            log::info!("preview: {} is already open in {}", device.name, mode.text());
        } else {
            let chosen = open.chosen;
            self.open_device(&device.id, chosen, Some(mode), settings, out);
        }
    }

    /// Opens a device in `mode`, else in the mode its stored choice resolves to, with its stored
    /// color range.
    fn open_device(&mut self, id: &str, chosen: bool, mode: Option<Mode>, settings: &Settings, out: &mut Vec<Command>) {
        let Some(device) = capture::device::find(id, &self.devices).cloned() else {
            log::warn!("preview: device {id} is not listed");
            return;
        };
        let stored = settings.device(&device.id);
        let range = range_choice(stored.map(|c| c.color_range).unwrap_or_default());
        let Some(mode) = mode.or_else(|| mode_to_open(&device, stored.map(|c| c.mode.as_str()))) else {
            log::error!("preview: {} ({}) offers no mode", device.name, device.id);
            if self.open.take().is_some() {
                out.push(Command::Close);
            }
            out.push(Command::ShowRange);
            self.again = Some(Again {
                device: device.id.clone(),
                mode: None,
                chosen,
            });
            self.state = Some(PreviewState::Failed(device.id));
            return;
        };
        self.again = None;
        self.sessions += 1;
        let session = self.sessions;
        self.open = Some(Open {
            device: device.id.clone(),
            session,
            mode: mode.clone(),
            chosen,
            range,
            in_use: None,
        });
        out.push(Command::ShowRange);
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
                self.woke = false;
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
                self.again = Some(Again::of(open));
                self.open = None;
                out.push(Command::Close);
                // The running monitor may still list a device that left (T033).
                out.push(Command::Relist);
                self.state = Some(match error {
                    DeviceError::Blocked { node } => PreviewState::Blocked { device: Some(id), node },
                    // After a wake: a reconnect (FR-014), opened again once listed.
                    _ if self.woke => PreviewState::Disconnected(id),
                    DeviceError::Gone => PreviewState::Disconnected(id),
                    DeviceError::InUse => PreviewState::InUse(id),
                    DeviceError::NoSignal => PreviewState::NoSignal(id),
                    DeviceError::Failed(_) => PreviewState::Failed(id),
                });
            }
            SessionEvent::Range(range) => {
                open.in_use = Some(range);
                out.push(Command::ShowRange);
            }
            SessionEvent::Playing { .. } => {}
        }
    }
}

/// The choice a device opens with: its stored choice when the device can open it, else its
/// default (unknown, missing or no longer offered; the stored text is not replaced, contracts
/// settings-file.md rule 2).
pub fn mode_choice(device: &Device, stored: Option<&str>) -> ModeChoice {
    let signal = device.signal;
    stored
        .and_then(ModeChoice::parse)
        .filter(|choice| mode::resolve(choice, &device.modes, signal).is_some())
        .unwrap_or_else(|| mode::default_choice(signal, &device.modes))
}

/// The mode a device opens in: [`mode_choice`] resolved; `None` when it offers no mode.
pub fn mode_to_open(device: &Device, stored: Option<&str>) -> Option<Mode> {
    mode::resolve(&mode_choice(device, stored), &device.modes, device.signal)
}

/// One row of the device or mode menu (FR-002, FR-004, FR-007). Texts that need a catalog are
/// named, not written: app/src/ui.rs turns each row into a Slint `MenuItem`.
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub text: Label,
    /// The second line: the mode a device opens in, or the mode an automatic choice resolves to.
    pub detail: Option<Detail>,
    /// The frame rate of the row's mode (`60`, `59.94`), shown in its tag after the size: on the
    /// detail line when there is one, else after the text (data-model.md "Mode").
    pub rate: Option<String>,
    /// The name and aspect tags of the row's mode (F43).
    pub tags: Option<Tags>,
    /// The open device, or the mode choice in use.
    pub current: bool,
    pub disabled: bool,
    /// What picking the row does; `None` for headers and "No devices".
    pub pick: Option<Pick>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Label {
    /// Shown as is: a device's (numbered) name or a mode's size.
    Plain(String),
    /// "No devices", the only row of an empty device menu.
    NoDevices,
    /// The header of the automatic modes.
    Automatic,
    /// The header of the native modes.
    Modes,
    /// The name of an automatic choice ("Auto max FPS").
    Auto(ModeChoice),
}

impl Label {
    pub fn is_header(&self) -> bool {
        matches!(self, Self::Automatic | Self::Modes)
    }
}

/// A detail line: the mode's size, after the automatic choice's name when it is one
/// ("Auto max resolution · 1920 × 1080"); the rate tag follows it.
#[derive(Debug, Clone, PartialEq)]
pub struct Detail {
    pub automatic: Option<ModeChoice>,
    pub mode: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Pick {
    Device(String),
    Mode(ModeChoice),
}

/// The device menu: each device with the mode it will open in (its stored choice, else its
/// default) and that mode's tags, the open one marked; one disabled "No devices" row when the
/// list is empty, never an empty menu.
pub fn device_items(devices: &[Device], open: Option<&str>, settings: &Settings) -> Vec<Item> {
    if devices.is_empty() {
        return vec![Item {
            text: Label::NoDevices,
            detail: None,
            rate: None,
            tags: None,
            current: false,
            disabled: true,
            pick: None,
        }];
    }
    devices
        .iter()
        .map(|device| {
            let choice = mode_choice(device, settings.device(&device.id).map(|c| c.mode.as_str()));
            let mode = mode::resolve(&choice, &device.modes, device.signal);
            Item {
                text: Label::Plain(device.name.clone()),
                detail: mode.as_ref().map(|m| Detail {
                    automatic: choice.is_automatic().then(|| choice.clone()),
                    mode: m.size_text(),
                }),
                rate: mode.as_ref().map(|m| mode::rate_text(m.fps)),
                tags: mode.as_ref().map(Mode::tags),
                current: open == Some(device.id.as_str()),
                disabled: false,
                pick: Some(Pick::Device(device.id.clone())),
            }
        })
        .collect()
}

/// The mode menu of a device: the **Automatic** header and the automatic choices it shows (no
/// two resolving to the same mode) with their resolved mode as detail, then the **Modes** header
/// and each native mode, largest first, each with its tags; `choice` marked. An automatic choice
/// hidden as a duplicate marks the shown one that opens the same mode.
pub fn mode_items(device: &Device, choice: &ModeChoice) -> Vec<Item> {
    let header = |text| Item {
        text,
        detail: None,
        rate: None,
        tags: None,
        current: false,
        disabled: false,
        pick: None,
    };
    let mut items = Vec::new();
    let automatic = mode::shown_automatic(&device.modes, device.signal);
    if !automatic.is_empty() {
        items.push(header(Label::Automatic));
    }
    for (auto, resolved) in &automatic {
        items.push(Item {
            text: Label::Auto(auto.clone()),
            detail: Some(Detail {
                automatic: None,
                mode: resolved.size_text(),
            }),
            rate: Some(mode::rate_text(resolved.fps)),
            tags: Some(resolved.tags()),
            current: false,
            disabled: false,
            pick: Some(Pick::Mode(auto.clone())),
        });
    }
    if !device.modes.is_empty() {
        items.push(header(Label::Modes));
    }
    for native in &device.modes {
        items.push(Item {
            text: Label::Plain(native.size_text()),
            detail: None,
            rate: Some(mode::rate_text(native.fps)),
            tags: Some(native.tags()),
            current: false,
            disabled: false,
            pick: Some(Pick::Mode(ModeChoice::manual(native))),
        });
    }
    let resolved = mode::resolve(choice, &device.modes, device.signal);
    let opens = |item: &Item| match &item.pick {
        Some(Pick::Mode(c)) if c.is_automatic() == choice.is_automatic() => {
            resolved.is_some() && mode::resolve(c, &device.modes, device.signal) == resolved
        }
        _ => false,
    };
    let marked = items
        .iter()
        .position(|i| i.pick.as_ref() == Some(&Pick::Mode(choice.clone())))
        .or_else(|| items.iter().position(opens));
    if let Some(index) = marked {
        items[index].current = true;
    }
    items
}

pub fn range_choice(stored: ColorRange) -> RangeChoice {
    match stored {
        ColorRange::Auto => RangeChoice::Auto,
        ColorRange::Full => RangeChoice::Full,
        ColorRange::Limited => RangeChoice::Limited,
    }
}

/// A color range choice as stored.
pub fn color_range(choice: RangeChoice) -> ColorRange {
    match choice {
        RangeChoice::Auto => ColorRange::Auto,
        RangeChoice::Full => ColorRange::Full,
        RangeChoice::Limited => ColorRange::Limited,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use capture::mode::Format;

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
        // No stored choice: 1080p near 60 (FR-008).
        assert_eq!(mode, self::mode(1920, 1080, 60));
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
        // FR-008: 1080p near 60, not Auto max FPS (1080p120).
        let default = Some(mode(1920, 1080, 60));
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
    fn choosing_the_open_device_again_does_not_reopen_it() {
        let settings = Settings::default();
        let mut p = Preview::new();
        let out = p.handle(listed(vec![card(), webcam()]), &settings);
        let session = opened(&out).unwrap().2;
        let out = p.handle(Event::ChooseDevice("card".into()), &settings);
        assert!(opened(&out).is_none() && !closes(&out), "{out:?}");
        assert_eq!(p.session(), Some(session));
    }

    #[test]
    fn menus_follow_the_devices_and_the_open_one() {
        let settings = Settings::default();
        let mut p = Preview::new();
        let (devices, modes) = p.menus(&settings);
        assert_eq!(devices[0].text, Label::NoDevices);
        assert!(modes.is_empty(), "no mode menu without an open device");

        p.handle(listed(vec![card(), webcam()]), &settings);
        let (devices, modes) = p.menus(&settings);
        assert_eq!(current(&devices), [&Label::Plain("ezcap Live GAMER ULTRA".into())]);
        assert_eq!(modes, mode_items(&card(), &mode_choice(&card(), None)));

        // A device that leaves is gone from the menu at once (spec edge case).
        p.handle(Event::Monitor(MonitorEvent::Removed("cam".into())), &settings);
        assert_eq!(p.menus(&settings).0.len(), 1);
        p.handle(Event::Monitor(MonitorEvent::Removed("card".into())), &settings);
        let (devices, modes) = p.menus(&settings);
        assert_eq!(devices[0].text, Label::NoDevices);
        assert!(modes.is_empty());
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

    fn error(session: u64, error: DeviceError) -> Event {
        Event::Session {
            session,
            event: SessionEvent::Error(error),
        }
    }

    fn added(device: Device) -> Event {
        Event::Monitor(MonitorEvent::Added(device))
    }

    fn removed(id: &str) -> Event {
        Event::Monitor(MonitorEvent::Removed(id.into()))
    }

    fn relisted(devices: Vec<Device>, timer: bool) -> Event {
        Event::Relisted { devices, timer }
    }

    fn blocked(device: Option<&str>, node: Option<&str>) -> PreviewState {
        PreviewState::Blocked {
            device: device.map(Into::into),
            node: node.map(Into::into),
        }
    }

    /// A preview whose camera access reads from the returned cell.
    fn with_access(access: Access) -> (Preview, std::sync::Arc<std::sync::Mutex<Access>>) {
        let cell = std::sync::Arc::new(std::sync::Mutex::new(access));
        let read = std::sync::Arc::clone(&cell);
        (Preview::with_access(move || *read.lock().unwrap()), cell)
    }

    /// The card playing in 1280 × 720 (not its default mode); returns its session.
    fn playing_720(p: &mut Preview, settings: &Settings, devices: Vec<Device>) -> u64 {
        p.handle(listed(devices), settings);
        let session = opened(&p.handle(Event::ChooseMode(choice("1280x720@60")), settings))
            .unwrap()
            .2;
        p.handle(first_frame(session), settings);
        assert_eq!(p.state(), PreviewState::Playing);
        session
    }

    #[test]
    fn each_session_error_shows_its_state_and_asks_for_the_list() {
        let cases = [
            (DeviceError::InUse, PreviewState::InUse("card".into())),
            (
                DeviceError::Blocked {
                    node: Some("/dev/video0".into()),
                },
                blocked(Some("card"), Some("/dev/video0")),
            ),
            (DeviceError::Blocked { node: None }, blocked(Some("card"), None)),
            (DeviceError::Gone, PreviewState::Disconnected("card".into())),
            (DeviceError::NoSignal, PreviewState::NoSignal("card".into())),
            (DeviceError::Failed("boom".into()), PreviewState::Failed("card".into())),
        ];
        for (err, state) in cases {
            let settings = Settings::default();
            let mut p = Preview::new();
            let session = opened(&p.handle(listed(vec![card()]), &settings)).unwrap().2;
            let out = p.handle(error(session, err.clone()), &settings);
            assert_eq!(p.state(), state, "{err:?}");
            assert!(closes(&out), "{err:?}");
            assert!(has(&out, |c| matches!(c, Command::Relist)), "{err:?}");
            assert!(has(&out, |c| matches!(c, Command::ShowState)), "{err:?}");
            assert_eq!(p.session(), None);
        }
    }

    #[test]
    fn try_again_reopens_in_the_same_mode() {
        for err in [
            DeviceError::InUse,
            DeviceError::NoSignal,
            DeviceError::Blocked { node: None },
            DeviceError::Failed("x".into()),
        ] {
            let settings = Settings::default();
            let mut p = Preview::new();
            let session = playing_720(&mut p, &settings, vec![card()]);
            p.handle(error(session, err.clone()), &settings);
            let (id, mode, new) = opened(&p.handle(Event::TryAgain, &settings)).unwrap();
            assert_eq!((id.as_str(), mode), ("card", self::mode(1280, 720, 60)), "{err:?}");
            assert!(new > session);
            assert_eq!(p.state(), PreviewState::Opening("card".into()));
        }
    }

    #[test]
    fn try_again_keeps_a_user_choice_to_save() {
        let settings = Settings::default();
        let mut p = Preview::new();
        p.handle(listed(vec![card(), webcam()]), &settings);
        let session = opened(&p.handle(Event::ChooseDevice("cam".into()), &settings))
            .unwrap()
            .2;
        p.handle(error(session, DeviceError::InUse), &settings);
        let session = opened(&p.handle(Event::TryAgain, &settings)).unwrap().2;
        assert!(saves(&p.handle(first_frame(session), &settings)));
    }

    #[test]
    fn try_again_does_nothing_where_not_offered() {
        let settings = Settings::default();
        let mut p = Preview::new();
        p.handle(listed(vec![]), &settings);
        assert!(p.handle(Event::TryAgain, &settings).is_empty());
        let session = opened(&p.handle(added(card()), &settings)).unwrap().2;
        assert!(p.handle(Event::TryAgain, &settings).is_empty(), "opening");
        p.handle(first_frame(session), &settings);
        assert!(p.handle(Event::TryAgain, &settings).is_empty(), "playing");
        p.handle(removed("card"), &settings);
        p.handle(added(webcam()), &settings);
        assert!(p.handle(Event::TryAgain, &settings).is_empty(), "disconnected");
    }

    #[test]
    fn try_again_on_a_device_no_longer_listed_disconnects() {
        let settings = Settings::default();
        let mut p = Preview::new();
        let session = opened(&p.handle(listed(vec![card(), webcam()]), &settings)).unwrap().2;
        p.handle(error(session, DeviceError::Failed("x".into())), &settings);
        // The re-read after the error no longer lists it.
        p.handle(relisted(vec![webcam()], false), &settings);
        assert_eq!(p.state(), PreviewState::Disconnected("card".into()));
        assert!(opened(&p.handle(Event::TryAgain, &settings)).is_none());
    }

    #[test]
    fn disconnected_reopens_the_same_device_in_the_same_mode() {
        let settings = Settings::default();
        let mut p = Preview::new();
        let session = playing_720(&mut p, &settings, vec![card(), webcam()]);
        let out = p.handle(removed("card"), &settings);
        assert!(closes(&out));
        assert_eq!(p.state(), PreviewState::Disconnected("card".into()));
        assert!(p.waiting());
        // Another device plugged in meanwhile only appears in the menu (spec edge case).
        p.handle(removed("cam"), &settings);
        assert_eq!(p.state(), PreviewState::Disconnected("card".into()));
        let out = p.handle(added(webcam()), &settings);
        assert!(opened(&out).is_none());
        assert_eq!(p.menus(&settings).0.len(), 1);
        let (id, mode, new) = opened(&p.handle(added(card()), &settings)).unwrap();
        assert_eq!((id.as_str(), mode), ("card", self::mode(1280, 720, 60)));
        assert!(new > session);
        assert!(!p.waiting());
    }

    #[test]
    fn a_device_that_comes_back_without_the_mode_opens_its_stored_choice() {
        let settings = Settings::default();
        let mut p = Preview::new();
        playing_720(&mut p, &settings, vec![card()]);
        p.handle(removed("card"), &settings);
        let back = Device::described("card", "ezcap Live GAMER ULTRA", vec![mode(1920, 1080, 60)]);
        assert_eq!(
            opened(&p.handle(added(back), &settings)).unwrap().1,
            mode(1920, 1080, 60)
        );
    }

    #[test]
    fn gone_waits_and_the_user_can_pick_another_device() {
        let settings = Settings::default();
        let mut p = Preview::new();
        let session = opened(&p.handle(listed(vec![card(), webcam()]), &settings)).unwrap().2;
        p.handle(error(session, DeviceError::Gone), &settings);
        assert_eq!(p.state(), PreviewState::Disconnected("card".into()));
        let (id, ..) = opened(&p.handle(Event::ChooseDevice("cam".into()), &settings)).unwrap();
        assert_eq!(id, "cam");
        // No longer waited for: its return does not switch.
        p.handle(removed("card"), &settings);
        assert!(opened(&p.handle(added(card()), &settings)).is_none());
    }

    #[test]
    fn the_re_read_adds_and_removes_what_the_monitor_missed() {
        let settings = Settings::default();
        let mut p = Preview::new();
        p.handle(listed(vec![]), &settings);
        assert!(p.waiting());
        // A missed add while waiting with no devices: the start device opens.
        let out = p.handle(relisted(vec![card()], true), &settings);
        assert_eq!(opened(&out).map(|o| o.0).as_deref(), Some("card"));
        let session = p.session().unwrap();
        p.handle(first_frame(session), &settings);
        // A missed remove of the open device disconnects.
        let out = p.handle(relisted(vec![webcam()], false), &settings);
        assert!(closes(&out));
        assert_eq!(p.state(), PreviewState::Disconnected("card".into()));
        assert_eq!(p.devices().len(), 1);
        // The same list again changes nothing.
        assert!(p.handle(relisted(vec![webcam()], true), &settings).is_empty());
        // Its return through a re-read reopens it.
        let out = p.handle(relisted(vec![webcam(), card()], true), &settings);
        assert_eq!(opened(&out).map(|o| o.0).as_deref(), Some("card"));
    }

    #[test]
    fn a_device_of_an_error_state_that_leaves_disconnects() {
        let settings = Settings::default();
        let mut p = Preview::new();
        let session = opened(&p.handle(listed(vec![card(), webcam()]), &settings)).unwrap().2;
        p.handle(error(session, DeviceError::InUse), &settings);
        p.handle(removed("card"), &settings);
        assert_eq!(p.state(), PreviewState::Disconnected("card".into()));
        assert!(opened(&p.handle(added(card()), &settings)).is_some());
    }

    #[test]
    fn no_devices_with_camera_access_denied_is_blocked() {
        let settings = Settings::default();
        let (mut p, access) = with_access(Access::Denied);
        p.handle(listed(vec![]), &settings);
        assert_eq!(p.state(), blocked(None, None));
        assert!(p.waiting());
        // Still denied: Try again stays Blocked.
        let out = p.handle(Event::TryAgain, &settings);
        assert!(opened(&out).is_none());
        assert_eq!(p.state(), blocked(None, None));
        // Allowed again, still nothing listed: the re-read shows No devices.
        *access.lock().unwrap() = Access::Allowed;
        p.handle(relisted(vec![], true), &settings);
        assert_eq!(p.state(), PreviewState::NoDevices);
        *access.lock().unwrap() = Access::Denied;
        p.handle(relisted(vec![], true), &settings);
        assert_eq!(p.state(), blocked(None, None));
        // The device listed once allowed opens.
        *access.lock().unwrap() = Access::Allowed;
        assert!(opened(&p.handle(added(card()), &settings)).is_some());
    }

    #[test]
    fn unknown_access_is_not_blocked() {
        let (mut p, _) = with_access(Access::Unknown);
        p.handle(listed(vec![]), &Settings::default());
        assert_eq!(p.state(), PreviewState::NoDevices);
    }

    #[test]
    fn access_turned_off_while_playing_is_blocked() {
        let settings = Settings::default();
        // Windows: the session error is mapped to Blocked by the capture crate.
        let mut p = Preview::new();
        let session = opened(&p.handle(listed(vec![card()]), &settings)).unwrap().2;
        p.handle(first_frame(session), &settings);
        p.handle(error(session, DeviceError::Blocked { node: None }), &settings);
        assert_eq!(p.state(), blocked(Some("card"), None));
        assert!(!p.waiting(), "still listed");

        // The device leaves the list as access goes off: Blocked, not Disconnected, and it opens
        // again when it is listed again.
        let (mut p, access) = with_access(Access::Allowed);
        let session = playing_720(&mut p, &settings, vec![card()]);
        *access.lock().unwrap() = Access::Denied;
        let out = p.handle(removed("card"), &settings);
        assert!(closes(&out));
        assert_eq!(p.state(), blocked(Some("card"), None));
        assert!(p.waiting());
        assert!(opened(&p.handle(Event::TryAgain, &settings)).is_none());
        *access.lock().unwrap() = Access::Allowed;
        let (id, mode, new) = opened(&p.handle(added(card()), &settings)).unwrap();
        assert_eq!((id.as_str(), mode), ("card", self::mode(1280, 720, 60)));
        assert!(new > session);
    }

    #[test]
    fn errors_after_a_wake_disconnect_and_reopen_once_listed() {
        let settings = Settings::default();
        let mut p = Preview::new();
        let session = playing_720(&mut p, &settings, vec![card()]);
        // The wake reopens the device at once.
        let out = p.handle(Event::Resumed, &settings);
        // Open closes the old session first.
        let (_, mode, woke) = opened(&out).unwrap();
        assert_eq!(mode, self::mode(1280, 720, 60));
        assert!(woke > session);
        // Not ready yet: a disconnect, not Failed.
        let out = p.handle(error(woke, DeviceError::Failed("x".into())), &settings);
        assert!(has(&out, |c| matches!(c, Command::Relist)));
        assert_eq!(p.state(), PreviewState::Disconnected("card".into()));
        // The re-read right after the error does not reopen (no loop); the timer's does.
        assert!(opened(&p.handle(relisted(vec![card()], false), &settings)).is_none());
        let again = opened(&p.handle(relisted(vec![card()], true), &settings)).unwrap();
        assert_eq!(again.1, self::mode(1280, 720, 60));
        p.handle(first_frame(again.2), &settings);
        assert_eq!(p.state(), PreviewState::Playing);
        // Playing again: errors map as usual.
        p.handle(error(again.2, DeviceError::Failed("x".into())), &settings);
        assert_eq!(p.state(), PreviewState::Failed("card".into()));
    }

    #[test]
    fn an_error_just_before_the_wake_is_noticed_reopens() {
        let settings = Settings::default();
        let mut p = Preview::new();
        let session = playing_720(&mut p, &settings, vec![card()]);
        p.handle(error(session, DeviceError::InUse), &settings);
        assert_eq!(p.state(), PreviewState::InUse("card".into()));
        // The wake is noticed after the error: the device opens again in its mode.
        let (_, mode, woke) = opened(&p.handle(Event::Resumed, &settings)).unwrap();
        assert_eq!(mode, self::mode(1280, 720, 60));
        // Still not ready: a disconnect, reopened by the timer once listed.
        p.handle(error(woke, DeviceError::Failed("x".into())), &settings);
        assert_eq!(p.state(), PreviewState::Disconnected("card".into()));
        assert!(p.waiting());

        // Blocked and Disconnected are left to access and the list.
        let mut p = Preview::new();
        let session = playing_720(&mut p, &settings, vec![card()]);
        p.handle(error(session, DeviceError::Blocked { node: None }), &settings);
        assert!(opened(&p.handle(Event::Resumed, &settings)).is_none());
        let mut p = Preview::new();
        playing_720(&mut p, &settings, vec![card()]);
        p.handle(removed("card"), &settings);
        assert!(opened(&p.handle(Event::Resumed, &settings)).is_none());
    }

    #[test]
    fn a_wake_with_nothing_open_does_nothing() {
        let settings = Settings::default();
        let mut p = Preview::new();
        p.handle(listed(vec![]), &settings);
        assert!(p.handle(Event::Resumed, &settings).is_empty());
        let session = opened(&p.handle(added(card()), &settings)).unwrap().2;
        p.handle(Event::Hidden, &settings);
        assert!(p.handle(Event::Resumed, &settings).is_empty());
        assert!(session > 0);
    }

    #[test]
    fn a_device_that_returns_while_hidden_opens_when_shown() {
        let settings = Settings::default();
        let mut p = Preview::new();
        playing_720(&mut p, &settings, vec![card()]);
        p.handle(removed("card"), &settings);
        p.handle(Event::Hidden, &settings);
        let out = p.handle(added(card()), &settings);
        assert!(opened(&out).is_none(), "nothing opens in the tray");
        let (id, mode, _) = opened(&p.handle(Event::Shown, &settings)).unwrap();
        assert_eq!((id.as_str(), mode), ("card", self::mode(1280, 720, 60)));

        // Removed while in the tray: Disconnected, and it opens when shown after it came back.
        p.handle(Event::Hidden, &settings);
        p.handle(removed("card"), &settings);
        assert_eq!(p.state(), PreviewState::Disconnected("card".into()));
        assert!(opened(&p.handle(Event::Shown, &settings)).is_none());
        assert!(opened(&p.handle(added(card()), &settings)).is_some());
    }

    #[test]
    fn hidden_and_shown_keep_the_mode() {
        let settings = Settings::default();
        let mut p = Preview::new();
        playing_720(&mut p, &settings, vec![card()]);
        p.handle(Event::Hidden, &settings);
        assert_eq!(
            opened(&p.handle(Event::Shown, &settings)).unwrap().1,
            mode(1280, 720, 60)
        );
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
        // Its name is still known for the state message.
        assert_eq!(p.name("card"), "ezcap Live GAMER ULTRA");
        assert_eq!(p.name("never"), "never");
    }

    #[test]
    fn a_range_choice_applies_and_is_stored_without_reopening() {
        let mut settings = Settings::default();
        settings.touch_device("card", "ezcap Live GAMER ULTRA").color_range = ColorRange::Limited;
        let mut p = Preview::new();
        let out = p.handle(listed(vec![card()]), &settings);
        let session = opened(&out).unwrap().2;
        assert_eq!(p.range(), (RangeChoice::Limited, None));
        let out = p.handle(
            Event::Session {
                session,
                event: SessionEvent::Range(Range::Limited),
            },
            &settings,
        );
        assert!(has(&out, |c| matches!(c, Command::ShowRange)));
        assert_eq!(p.range(), (RangeChoice::Limited, Some(Range::Limited)));

        let out = p.handle(Event::ChooseRange(RangeChoice::Auto), &settings);
        assert!(opened(&out).is_none(), "no reopen");
        assert!(has(&out, |c| matches!(c, Command::SetRange(RangeChoice::Auto))));
        assert!(has(&out, |c| matches!(
            c,
            Command::SaveRange { id, range: RangeChoice::Auto, .. } if id == "card"
        )));
        assert_eq!(p.range().0, RangeChoice::Auto);
        // The same choice again does nothing.
        assert!(p.handle(Event::ChooseRange(RangeChoice::Auto), &settings).is_empty());
    }

    #[test]
    fn a_range_choice_without_a_device_does_nothing() {
        let mut p = Preview::new();
        let settings = Settings::default();
        p.handle(listed(vec![]), &settings);
        assert!(p.handle(Event::ChooseRange(RangeChoice::Full), &settings).is_empty());
        assert_eq!(color_range(RangeChoice::Full), ColorRange::Full);
        assert_eq!(range_choice(color_range(RangeChoice::Limited)), RangeChoice::Limited);
    }

    /// The (id, choice) of the commands' SaveMode, if any.
    fn saved_mode(commands: &[Command]) -> Option<(String, ModeChoice)> {
        commands.iter().find_map(|c| match c {
            Command::SaveMode { id, choice, .. } => Some((id.clone(), choice.clone())),
            _ => None,
        })
    }

    fn choice(text: &str) -> ModeChoice {
        ModeChoice::parse(text).unwrap()
    }

    #[test]
    fn picking_a_device_opens_it_with_its_stored_mode_and_range() {
        let mut settings = Settings::default();
        let entry = settings.touch_device("cam", "HD Pro Webcam C920");
        entry.mode = "640x480@30".into();
        entry.color_range = ColorRange::Limited;
        let mut p = Preview::new();
        p.handle(listed(vec![card(), webcam()]), &settings);
        let out = p.handle(Event::ChooseDevice("cam".into()), &settings);
        let (id, mode, _) = opened(&out).unwrap();
        assert_eq!((id.as_str(), mode), ("cam", self::mode(640, 480, 30)));
        assert_eq!(p.range().0, RangeChoice::Limited);
        assert!(saved_mode(&out).is_none());
        // Without a stored choice: its defaults.
        let out = p.handle(Event::ChooseDevice("card".into()), &settings);
        assert_eq!(opened(&out).unwrap().1, self::mode(1920, 1080, 60));
        assert_eq!(p.range().0, RangeChoice::Auto);
    }

    #[test]
    fn picking_a_mode_reopens_the_device_and_stores_it() {
        let mut settings = Settings::default();
        let mut p = Preview::new();
        let first = opened(&p.handle(listed(vec![card(), webcam()]), &settings)).unwrap().2;
        p.handle(first_frame(first), &settings);
        let out = p.handle(Event::ChooseMode(choice("1280x720@60")), &settings);
        let (id, mode, session) = opened(&out).unwrap();
        assert_eq!((id.as_str(), mode), ("card", self::mode(1280, 720, 60)));
        assert!(session > first);
        assert_eq!(p.state(), PreviewState::Opening("card".into()));
        assert_eq!(saved_mode(&out), Some(("card".into(), choice("1280x720@60"))));
        // A fallback start device is not remembered by picking its mode.
        assert!(!saves(&p.handle(first_frame(session), &settings)));
        assert_eq!(p.state(), PreviewState::Playing);

        // Stored by the app; the mode menu marks it.
        settings.touch_device("card", "ezcap Live GAMER ULTRA").mode = "1280x720@60".into();
        let current = p.menus(&settings).1.into_iter().find(|i| i.current).unwrap();
        assert_eq!(current.pick, Some(Pick::Mode(choice("1280x720@60"))));

        // The same choice again does nothing.
        assert!(p.handle(Event::ChooseMode(choice("1280x720@60")), &settings).is_empty());
        // An automatic choice that opens another mode reopens too.
        let out = p.handle(Event::ChooseMode(ModeChoice::AutoMaxFps), &settings);
        assert_eq!(opened(&out).unwrap().1, self::mode(1920, 1080, 120));
        assert_eq!(saved_mode(&out), Some(("card".into(), ModeChoice::AutoMaxFps)));
    }

    #[test]
    fn a_mode_that_opens_the_same_mode_is_stored_without_reopening() {
        let settings = Settings::default();
        let mut p = Preview::new();
        p.handle(listed(vec![card()]), &settings);
        // The default opens 1080p60, as does this manual choice.
        let out = p.handle(Event::ChooseMode(choice("1920x1080@60")), &settings);
        assert!(opened(&out).is_none() && !closes(&out), "{out:?}");
        assert_eq!(saved_mode(&out), Some(("card".into(), choice("1920x1080@60"))));
    }

    #[test]
    fn a_stored_mode_the_device_lacks_is_replaced_only_by_a_pick() {
        let mut settings = Settings::default();
        settings.touch_device("card", "ezcap Live GAMER ULTRA").mode = "3840x2160@60".into();
        let mut p = Preview::new();
        let out = p.handle(listed(vec![card()]), &settings);
        // Opens the default and leaves the stored text alone.
        assert_eq!(opened(&out).unwrap().1, mode(1920, 1080, 60));
        assert!(saved_mode(&out).is_none());
        // Picking the mode it already plays stores it.
        let out = p.handle(Event::ChooseMode(choice("1920x1080@60")), &settings);
        assert!(opened(&out).is_none());
        assert_eq!(saved_mode(&out), Some(("card".into(), choice("1920x1080@60"))));
    }

    #[test]
    fn a_mode_pick_without_a_device_or_offered_mode_does_nothing() {
        let settings = Settings::default();
        let mut p = Preview::new();
        p.handle(listed(vec![]), &settings);
        assert!(
            p.handle(Event::ChooseMode(ModeChoice::AutoMaxFps), &settings)
                .is_empty()
        );
        p.handle(Event::Monitor(MonitorEvent::Added(webcam())), &settings);
        assert!(
            p.handle(Event::ChooseMode(choice("1920x1080@60")), &settings)
                .is_empty()
        );
    }

    #[test]
    fn a_mode_pick_while_opening_reopens_and_ignores_the_old_session() {
        let settings = Settings::default();
        let mut p = Preview::new();
        p.handle(listed(vec![card()]), &settings);
        let old = p.session().unwrap();
        let new = opened(&p.handle(Event::ChooseMode(choice("1280x720@60")), &settings))
            .unwrap()
            .2;
        assert!(p.handle(first_frame(old), &settings).is_empty());
        assert_eq!(p.state(), PreviewState::Opening("card".into()));
        p.handle(first_frame(new), &settings);
        assert_eq!(p.state(), PreviewState::Playing);
    }

    #[test]
    fn the_remembered_device_plugged_in_while_another_plays_does_not_switch() {
        let mut settings = Settings::default();
        settings.preview.device = Some("cam".into());
        let mut p = Preview::new();
        let session = opened(&p.handle(listed(vec![card()]), &settings)).unwrap().2;
        p.handle(first_frame(session), &settings);
        let out = p.handle(Event::Monitor(MonitorEvent::Added(webcam())), &settings);
        assert!(opened(&out).is_none() && !closes(&out), "{out:?}");
        assert_eq!(p.session(), Some(session));
        assert_eq!(p.menus(&settings).0.len(), 2);
    }

    /// A device from the caps recorded for the capture crate's tests.
    fn recorded(text: &str) -> Device {
        capture::recorded::device(text).expect("a recorded device")
    }

    fn c920() -> Device {
        recorded(include_str!("../../crates/capture/tests/data/c920-windows.txt"))
    }

    fn ezcap() -> Device {
        recorded(include_str!("../../crates/capture/tests/data/ezcap-linux.txt"))
    }

    fn detail(automatic: Option<ModeChoice>, mode: &str) -> Option<Detail> {
        Some(Detail {
            automatic,
            mode: mode.to_string(),
        })
    }

    /// Each row as (text, detail, tags, current), for comparing whole menus; the rate tag reads
    /// `[30 FPS]` after the line it is on.
    fn rows(items: &[Item]) -> Vec<(String, String, String, bool)> {
        items
            .iter()
            .map(|i| {
                let mut text = match &i.text {
                    Label::Plain(text) => text.clone(),
                    Label::NoDevices => "No devices".into(),
                    Label::Automatic => "# Automatic".into(),
                    Label::Modes => "# Modes".into(),
                    Label::Auto(choice) => choice.to_string(),
                };
                let mut detail = i.detail.as_ref().map_or_else(String::new, |d| match &d.automatic {
                    Some(choice) => format!("{choice} · {}", d.mode),
                    None => d.mode.clone(),
                });
                if let Some(rate) = &i.rate {
                    let line = if i.detail.is_some() { &mut detail } else { &mut text };
                    line.push_str(&format!(" [{rate} FPS]"));
                }
                (text, detail, tag_text(i.tags), i.current)
            })
            .collect()
    }

    fn current_index(items: &[Item]) -> usize {
        items.iter().position(|i| i.current).expect("a marked row")
    }

    fn tag_text(tags: Option<Tags>) -> String {
        tags.map_or_else(String::new, |t| format!("{} {}", t.name.unwrap_or("Custom"), t.aspect))
    }

    fn current(items: &[Item]) -> Vec<&Label> {
        items.iter().filter(|i| i.current).map(|i| &i.text).collect()
    }

    #[test]
    fn device_menu_shows_the_mode_each_device_opens_in() {
        let (cam, card) = (c920(), ezcap());
        let items = device_items(&[cam.clone(), card.clone()], Some(&card.id), &Settings::default());
        assert_eq!(
            items[0],
            Item {
                text: Label::Plain("HD Pro Webcam C920".into()),
                detail: detail(Some(ModeChoice::AutoMaxResolution), "1920 × 1080"),
                rate: Some("30".into()),
                tags: Some(capture::mode::tags(1920, 1080)),
                current: false,
                disabled: false,
                pick: Some(Pick::Device(cam.id.clone())),
            }
        );
        // The ezcap's default is the 1080p60 mode itself (FR-008), not an automatic choice.
        assert_eq!(items[1].text, Label::Plain("ezcap Live GAMER ULTRA: ezcap L".into()));
        assert_eq!(items[1].detail, detail(None, "1920 × 1080"));
        assert_eq!(tag_text(items[1].tags), "Full HD 16:9");
        assert!(items[1].current);
        assert_eq!(items[1].pick, Some(Pick::Device(card.id.clone())));
        // Nothing open: nothing marked.
        assert!(current(&device_items(&[cam, card], None, &Settings::default())).is_empty());
    }

    #[test]
    fn device_menu_follows_the_stored_mode_and_falls_back_to_the_default() {
        let (cam, card) = (c920(), ezcap());
        let mut settings = Settings::default();
        settings.touch_device(&card.id, &card.base_name).mode = "1280x720@60".into();
        settings.touch_device(&cam.id, &cam.base_name).mode = "auto-max-fps".into();
        let items = device_items(&[cam.clone(), card.clone()], None, &settings);
        assert_eq!(items[0].detail, detail(Some(ModeChoice::AutoMaxFps), "1920 × 1080"));
        assert_eq!(items[1].detail, detail(None, "1280 × 720"));
        assert_eq!(tag_text(items[1].tags), "HD 16:9");

        // A stored mode the device lacks, or rubbish: the default.
        settings.touch_device(&card.id, &card.base_name).mode = "3840x2160@60".into();
        settings.touch_device(&cam.id, &cam.base_name).mode = "rubbish".into();
        let items = device_items(&[cam, card], None, &settings);
        assert_eq!(
            items[0].detail,
            detail(Some(ModeChoice::AutoMaxResolution), "1920 × 1080")
        );
        assert_eq!(items[1].detail, detail(None, "1920 × 1080"));
    }

    #[test]
    fn empty_device_menu_has_one_disabled_row() {
        let items = device_items(&[], None, &Settings::default());
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].text, Label::NoDevices);
        assert!(items[0].disabled);
        assert_eq!(items[0].pick, None);
        // A device without modes is listed without a detail line or tags.
        let items = device_items(&[Device::described("x", "Empty", vec![])], None, &Settings::default());
        assert_eq!((&items[0].detail, items[0].tags), (&None, None));
    }

    #[test]
    fn ezcap_mode_menu() {
        let card = ezcap();
        let choice = mode_choice(&card, None);
        let items = mode_items(&card, &choice);
        let expected = [
            ("# Automatic", "", "", false),
            ("auto-max-resolution", "2560 × 1440 [60 FPS]", "WQHD 16:9", false),
            ("auto-max-fps", "1920 × 1080 [120 FPS]", "Full HD 16:9", false),
            ("# Modes", "", "", false),
            ("3840 × 2160 [30 FPS]", "", "4K UHD 16:9", false),
            ("2560 × 1440 [60 FPS]", "", "WQHD 16:9", false),
            ("2560 × 1440 [50 FPS]", "", "WQHD 16:9", false),
            ("1920 × 1080 [120 FPS]", "", "Full HD 16:9", false),
            // The default (FR-008) marked in the Modes group.
            ("1920 × 1080 [60 FPS]", "", "Full HD 16:9", true),
            ("1920 × 1080 [30 FPS]", "", "Full HD 16:9", false),
            ("1280 × 720 [60 FPS]", "", "HD 16:9", false),
        ];
        let expected: Vec<_> = expected
            .iter()
            .map(|(t, d, b, c)| (t.to_string(), d.to_string(), b.to_string(), *c))
            .collect();
        assert_eq!(rows(&items), expected);
        assert!(items[0].text.is_header() && items[3].text.is_header());
        assert_eq!(items[0].pick, None);
        assert_eq!(items[2].pick, Some(Pick::Mode(ModeChoice::AutoMaxFps)));
        assert_eq!(items[8].pick, ModeChoice::parse("1920x1080@60").map(Pick::Mode));

        // An automatic choice is marked in the Automatic group, not on the mode it opens.
        let items = mode_items(&card, &ModeChoice::AutoMaxFps);
        assert_eq!(current(&items), [&Label::Auto(ModeChoice::AutoMaxFps)]);
    }

    #[test]
    fn c920_mode_menu_has_no_duplicate_automatic_choice() {
        let cam = c920();
        let items = mode_items(&cam, &mode_choice(&cam, None));
        let rows = rows(&items);
        // Auto max FPS resolves to 1080p30 too: only Auto max resolution is shown, and marked
        // (the default).
        assert_eq!(rows[0].0, "# Automatic");
        assert_eq!(
            rows[1],
            (
                "auto-max-resolution".into(),
                "1920 × 1080 [30 FPS]".into(),
                "Full HD 16:9".into(),
                true
            )
        );
        assert_eq!(rows[2].0, "# Modes");
        assert_eq!(rows[3].0, "2304 × 1536 [2 FPS]");
        assert_eq!(rows.iter().filter(|r| r.0.starts_with("auto-")).count(), 1);
        assert_eq!(rows.len(), 3 + cam.modes.len());
        // Each native mode once.
        for (i, a) in rows.iter().enumerate() {
            assert!(!rows[i + 1..].iter().any(|b| b.0 == a.0), "{} twice", a.0);
        }
        // A stored Auto max FPS (hidden as a duplicate) marks the automatic row opening the same
        // mode.
        let items = mode_items(&cam, &ModeChoice::AutoMaxFps);
        assert_eq!(current(&items), [&Label::Auto(ModeChoice::AutoMaxResolution)]);
    }

    #[test]
    fn a_manual_choice_within_half_a_frame_is_marked() {
        let ntsc = Mode {
            fps: (60000, 1001),
            ..mode(1920, 1080, 0)
        };
        let device = Device::described("x", "NTSC card", vec![ntsc]);
        let items = mode_items(&device, &ModeChoice::parse("1920x1080@60").unwrap());
        assert_eq!(current(&items), [&Label::Plain("1920 × 1080".into())]);
        assert_eq!(items[current_index(&items)].rate.as_deref(), Some("59.94"));
        // No mode: no menu rows at all, nothing marked.
        let empty = Device::described("y", "Empty", vec![]);
        assert!(mode_items(&empty, &mode_choice(&empty, None)).is_empty());
    }
}
