//! Global hotkeys under Wayland through the XDG desktop portal's GlobalShortcuts (research R7,
//! probe T004, contracts/platform.md portal rules). A `hotkeys` thread owns the session; a
//! `hotkeys signals` thread forwards the portal's signals to it, so commands and answers are
//! handled in one place and in order.
//!
//! The desktop owns the keys: the settings' key is only proposed (`preferred_trigger`) the first
//! time, and what the desktop bound comes back as [`HotkeyEvent::Bound`]. Any failure before the
//! session exists (no portal, no interface, the app id refused) reports
//! [`HotkeyEvent::WindowOnly`].

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, mpsc};

use zbus::blocking::{Connection, MessageIterator, Proxy};
use zbus::message::Type;
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};
use zbus::{MatchRule, Message};

use super::super::desktop_entry::{self, Ensured};
use crate::hotkeys::{Binding, Command, HotkeyEvent, OnHotkeyEvent};

const DESTINATION: &str = "org.freedesktop.portal.Desktop";
const PATH: &str = "/org/freedesktop/portal/desktop";
const SHORTCUTS: &str = "org.freedesktop.portal.GlobalShortcuts";
const REGISTRY: &str = "org.freedesktop.host.portal.Registry";
const REQUEST: &str = "org.freedesktop.portal.Request";
const SESSION: &str = "org.freedesktop.portal.Session";
/// The app id: the desktop entry's name (research R12).
const APP_ID: &str = "capturax";

type Results = HashMap<String, OwnedValue>;
type Shortcuts = Vec<(String, Results)>;

/// What the `hotkeys` thread is given.
enum Input {
    Command(Command),
    Configure,
    Signal(Message),
    Quit,
}

/// What the caller's thread reads without asking the `hotkeys` thread.
#[derive(Default)]
struct Shared {
    /// The session exists: the desktop owns the keys.
    session: AtomicBool,
    /// The portal's GlobalShortcuts version.
    version: AtomicU32,
}

pub struct Portal {
    inputs: mpsc::Sender<Input>,
    shared: Arc<Shared>,
}

impl Portal {
    pub fn start(on_event: OnHotkeyEvent) -> Portal {
        let (inputs, received) = mpsc::channel();
        let shared = Arc::new(Shared::default());
        let (sender, state) = (inputs.clone(), shared.clone());
        let spawned = std::thread::Builder::new().name("hotkeys".into()).spawn(move || {
            if let Err(err) = run(received, sender, state.clone(), &on_event) {
                state.session.store(false, Ordering::Relaxed);
                log::warn!("hotkeys: no global shortcuts portal: {err}");
                log::info!("hotkeys: no global keys here; the keys work while the window has the focus");
                on_event(HotkeyEvent::WindowOnly);
            }
        });
        if let Err(err) = spawned {
            log::warn!("hotkeys: cannot start the hotkeys thread: {err}");
        }
        Portal { inputs, shared }
    }

    pub fn set(&self, bindings: Vec<Binding>) {
        let _ = self.inputs.send(Input::Command(Command::Set(bindings)));
    }

    pub fn suspend(&self, suspended: bool) {
        let _ = self.inputs.send(Input::Command(Command::Suspend(suspended)));
    }

    /// Opens the desktop's shortcut settings (portal version 2); false when it cannot.
    pub fn configure(&self) -> bool {
        self.shared.session.load(Ordering::Relaxed)
            && self.shared.version.load(Ordering::Relaxed) >= 2
            && self.inputs.send(Input::Configure).is_ok()
    }

    pub fn changed_by_desktop(&self) -> bool {
        self.shared.session.load(Ordering::Relaxed)
    }
}

impl Drop for Portal {
    fn drop(&mut self) {
        let _ = self.inputs.send(Input::Quit);
    }
}

/// The request a call is waiting on.
enum Pending {
    Create(String),
    /// The actions and descriptions sent.
    Bind(String, Vec<(String, String)>),
}

struct State<'a> {
    connection: Connection,
    shortcuts: Proxy<'static>,
    on_event: &'a OnHotkeyEvent,
    shared: Arc<Shared>,
    /// The connection's unique name as a request path element (`1_42`).
    sender: String,
    next_token: u32,
    session: Option<OwnedObjectPath>,
    pending: Option<Pending>,
    /// The actions and descriptions the session was bound with.
    bound: Option<Vec<(String, String)>>,
    bindings: Vec<Binding>,
    suspended: bool,
}

fn run(
    inputs: mpsc::Receiver<Input>,
    sender: mpsc::Sender<Input>,
    shared: Arc<Shared>,
    on_event: &OnHotkeyEvent,
) -> zbus::Result<()> {
    // The portal accepts only an app id with a desktop entry (probe T004).
    match desktop_entry::executable().and_then(|exe| desktop_entry::ensure(&exe)) {
        Ok(Ensured::Installed(path)) => log::info!("hotkeys: desktop entry {}", path.display()),
        Ok(Ensured::Current(_)) => {}
        Ok(Ensured::Written(path)) => log::info!("hotkeys: wrote the hidden desktop entry {}", path.display()),
        Ok(Ensured::Updated(path)) => log::info!("hotkeys: desktop entry {} now names this program", path.display()),
        Err(err) => log::warn!("hotkeys: cannot write the desktop entry: {err}"),
    }

    let connection = Connection::session()?;
    // Every signal under the portal's path (the requests' answers, the shortcuts' and the
    // session's signals), subscribed before any call so no answer is missed.
    let rule = MatchRule::builder()
        .msg_type(Type::Signal)
        .path_namespace(PATH)?
        .build();
    let signals = MessageIterator::for_match_rule(rule, &connection, None)?;
    std::thread::Builder::new()
        .name("hotkeys signals".into())
        .spawn(move || {
            for message in signals {
                match message {
                    Ok(message) => {
                        if sender.send(Input::Signal(message)).is_err() {
                            break;
                        }
                    }
                    Err(err) => log::debug!("hotkeys: bad portal signal: {err}"),
                }
            }
        })
        .map_err(|err| zbus::Error::Failure(format!("cannot start the signals thread: {err}")))?;

    register(&connection);
    let shortcuts = Proxy::new_owned(connection.clone(), DESTINATION, PATH, SHORTCUTS)?;
    let version: u32 = shortcuts
        .get_property("version")
        .map_err(|err| zbus::Error::Failure(format!("no GlobalShortcuts interface: {err}")))?;
    shared.version.store(version, Ordering::Relaxed);
    log::info!("hotkeys: GlobalShortcuts portal version {version}");

    let sender = connection
        .unique_name()
        .map(|name| name.trim_start_matches(':').replace('.', "_"))
        .unwrap_or_default();
    let mut state = State {
        connection,
        shortcuts,
        on_event,
        shared,
        sender,
        next_token: 0,
        session: None,
        pending: None,
        bound: None,
        bindings: Vec::new(),
        suspended: false,
    };
    state.create_session()?;
    for input in inputs {
        match input {
            Input::Quit => break,
            Input::Command(Command::Set(bindings)) => {
                state.bindings = bindings;
                state.bind_if_needed()?;
            }
            Input::Command(Command::Suspend(suspended)) => state.suspended = suspended,
            Input::Configure => state.configure(),
            Input::Signal(message) => state.signal(&message)?,
        }
    }
    state.close_session();
    Ok(())
}

/// Registers the app id (an unsandboxed app has none otherwise); must come before any other
/// portal call on this connection. Older portals have no registry: the session call then decides.
fn register(connection: &Connection) {
    let result = Proxy::new(connection, DESTINATION, PATH, REGISTRY).and_then(|registry| {
        let options: HashMap<&str, Value> = HashMap::new();
        registry.call_method("Register", &(APP_ID, options)).map(|_| ())
    });
    if let Err(err) = result {
        log::info!("hotkeys: app id not registered: {err}");
    }
}

impl State<'_> {
    /// A new `handle_token` and the `Request` path it gives.
    fn token(&mut self, kind: &str) -> (String, String) {
        self.next_token += 1;
        let token = format!("capturax_{kind}_{}", self.next_token);
        let path = format!("{PATH}/request/{}/{token}", self.sender);
        (token, path)
    }

    fn create_session(&mut self) -> zbus::Result<()> {
        let (token, expected) = self.token("create");
        let (session_token, _) = self.token("session");
        let options = HashMap::from([
            ("handle_token", Value::from(token.as_str())),
            ("session_handle_token", Value::from(session_token.as_str())),
        ]);
        let handle: OwnedObjectPath = self.shortcuts.call("CreateSession", &(options,))?;
        // A portal older than the token rule answers on a path of its own.
        let path = if handle.as_str() == expected {
            expected
        } else {
            handle.as_str().to_string()
        };
        self.pending = Some(Pending::Create(path));
        Ok(())
    }

    /// The actions and descriptions wanted now.
    fn wanted(&self) -> Vec<(String, String)> {
        self.bindings
            .iter()
            .map(|(action, _, description)| (action.clone(), description.clone()))
            .collect()
    }

    /// Binds when the actions or their descriptions changed since the last bind (key changes are
    /// the desktop's). A session already bound is replaced by a new one.
    fn bind_if_needed(&mut self) -> zbus::Result<()> {
        if self.pending.is_some() || self.session.is_none() || self.bindings.is_empty() {
            return Ok(());
        }
        let wanted = self.wanted();
        if self.bound.as_ref() == Some(&wanted) {
            return Ok(());
        }
        if self.bound.is_some() {
            self.close_session();
            self.bound = None;
            return self.create_session();
        }
        let Some(session) = self.session.clone() else {
            return Ok(());
        };
        let (token, expected) = self.token("bind");
        let list: Vec<(&str, HashMap<&str, Value>)> = self
            .bindings
            .iter()
            .map(|(action, key, description)| {
                let mut values = HashMap::from([("description", Value::from(description.as_str()))]);
                if let Some(key) = key {
                    values.insert("preferred_trigger", Value::from(key.to_portal()));
                }
                (action.as_str(), values)
            })
            .collect();
        let options = HashMap::from([("handle_token", Value::from(token.as_str()))]);
        let handle: OwnedObjectPath = self.shortcuts.call("BindShortcuts", &(&session, list, "", options))?;
        let path = if handle.as_str() == expected {
            expected
        } else {
            handle.as_str().to_string()
        };
        self.pending = Some(Pending::Bind(path, wanted));
        Ok(())
    }

    fn close_session(&mut self) {
        let Some(session) = self.session.take() else {
            return;
        };
        let closed = Proxy::new(&self.connection, DESTINATION, session.as_str(), SESSION)
            .and_then(|proxy| proxy.call_method("Close", &()).map(|_| ()));
        if let Err(err) = closed {
            log::debug!("hotkeys: closing the portal session: {err}");
        }
    }

    fn configure(&self) {
        let Some(session) = &self.session else {
            return;
        };
        let options: HashMap<&str, Value> = HashMap::new();
        match self
            .shortcuts
            .call_method("ConfigureShortcuts", &(session, "", options))
        {
            Ok(_) => log::info!("hotkeys: opened the desktop's shortcut settings"),
            Err(err) => log::warn!("hotkeys: cannot open the desktop's shortcut settings: {err}"),
        }
    }

    /// Reports each action's key from the desktop's list (`""` or missing: no key).
    fn report(&self, list: &Shortcuts) {
        for (action, _, _) in &self.bindings {
            let shown = list
                .iter()
                .find(|(id, _)| id == action)
                .and_then(|(_, values)| values.get("trigger_description"))
                .and_then(|value| value.try_clone().ok())
                .and_then(|value| String::try_from(value).ok())
                .filter(|text| !text.is_empty());
            (self.on_event)(HotkeyEvent::Bound {
                action: action.clone(),
                shown,
            });
        }
    }

    fn signal(&mut self, message: &Message) -> zbus::Result<()> {
        let header = message.header();
        let (Some(interface), Some(member), Some(path)) = (header.interface(), header.member(), header.path()) else {
            return Ok(());
        };
        match (interface.as_str(), member.as_str()) {
            (REQUEST, "Response") => {
                let answers = match &self.pending {
                    Some(Pending::Create(pending) | Pending::Bind(pending, _)) => pending == path.as_str(),
                    None => false,
                };
                if !answers {
                    return Ok(());
                }
                let (code, results) = message.body().deserialize::<(u32, Results)>()?;
                match self.pending.take() {
                    Some(Pending::Create(_)) => {
                        if code != 0 {
                            return Err(zbus::Error::Failure(format!("the session was refused ({code})")));
                        }
                        let session = results
                            .get("session_handle")
                            .and_then(|value| value.try_clone().ok())
                            .and_then(|value| String::try_from(value).ok())
                            .and_then(|text| OwnedObjectPath::try_from(text).ok())
                            .ok_or_else(|| zbus::Error::Failure("no session handle".into()))?;
                        self.session = Some(session);
                        self.shared.session.store(true, Ordering::Relaxed);
                    }
                    Some(Pending::Bind(_, sent)) => {
                        self.bound = Some(sent);
                        let list = results
                            .get("shortcuts")
                            .and_then(|value| value.try_clone().ok())
                            .and_then(|value| Shortcuts::try_from(value).ok())
                            .unwrap_or_default();
                        match code {
                            0 => {}
                            1 => log::info!("hotkeys: the desktop's shortcut question was cancelled"),
                            other => log::warn!("hotkeys: the desktop refused the shortcuts ({other})"),
                        }
                        self.report(&list);
                    }
                    None => {}
                }
                self.bind_if_needed()?;
            }
            (SHORTCUTS, "Activated") => {
                let (session, action, _, _) = message
                    .body()
                    .deserialize::<(OwnedObjectPath, String, u64, Results)>()?;
                let ours = self.session.as_ref() == Some(&session);
                if ours && !self.suspended && self.bindings.iter().any(|(name, _, _)| *name == action) {
                    (self.on_event)(HotkeyEvent::Pressed(action));
                }
            }
            (SHORTCUTS, "ShortcutsChanged") => {
                let (session, list) = message.body().deserialize::<(OwnedObjectPath, Shortcuts)>()?;
                if self.session.as_ref() == Some(&session) {
                    self.report(&list);
                }
            }
            (SESSION, "Closed")
                if self
                    .session
                    .as_ref()
                    .is_some_and(|session| session.as_str() == path.as_str()) =>
            {
                log::info!("hotkeys: the portal closed the session; opening a new one");
                self.session = None;
                self.bound = None;
                self.pending = None;
                self.create_session()?;
            }
            _ => {}
        }
        Ok(())
    }
}
