//! Global hotkeys on X11 (research R9): a `hotkeys` thread with its own `x11rb` connection grabs
//! each key on the root window four times (with and without Num Lock and Caps Lock), keycodes
//! from the keyboard mapping. Detectable auto-repeat makes a held key one press and one release;
//! where the server refuses it, a repeat's release and press share a timestamp and the press is
//! skipped. Commands come through a channel; a client message to the thread's own window wakes
//! it.

use std::collections::HashSet;
use std::fmt::Display;
use std::sync::{Arc, mpsc};
use std::thread::JoinHandle;

use x11rb::connection::Connection;
use x11rb::protocol::Event;
use x11rb::protocol::xkb::{self, ConnectionExt as _};
use x11rb::protocol::xproto::{
    AtomEnum, ClientMessageEvent, ConnectionExt as _, CreateWindowAux, EventMask, GrabMode, Keycode, Mapping, ModMask,
    Window, WindowClass,
};
use x11rb::rust_connection::RustConnection;

use crate::hotkeys::{Binding, Command, HotkeyEvent, OnHotkeyEvent, Wanted};

/// Shift, Control, Mod1 (Alt) and Mod4 (Super): the modifiers a binding names.
const NAMED_MODIFIERS: u16 = 0x1 | 0x4 | 0x8 | 0x40;
/// Caps Lock (Lock) and Num Lock (Mod2): grabbed in every combination so they do not matter.
const LOCKS: [u16; 4] = [0, 0x2, 0x10, 0x2 | 0x10];

fn text(err: impl Display) -> String {
    err.to_string()
}

pub struct X11 {
    conn: Arc<RustConnection>,
    /// The thread's own window, for the wake-up message.
    wake: Window,
    /// None asks the thread to end.
    commands: mpsc::Sender<Option<Command>>,
    handle: Option<JoinHandle<()>>,
}

impl X11 {
    pub fn start(on_event: OnHotkeyEvent) -> Result<X11, String> {
        let (conn, screen) = x11rb::connect(None).map_err(text)?;
        let conn = Arc::new(conn);
        let root = conn.setup().roots[screen].root;
        let wake = conn.generate_id().map_err(text)?;
        conn.create_window(
            x11rb::COPY_DEPTH_FROM_PARENT,
            wake,
            root,
            0,
            0,
            1,
            1,
            0,
            WindowClass::INPUT_ONLY,
            x11rb::COPY_FROM_PARENT,
            &CreateWindowAux::new(),
        )
        .map_err(text)?
        .check()
        .map_err(text)?;
        if let Err(err) = detectable_auto_repeat(&conn) {
            log::info!("hotkeys: no detectable auto-repeat ({err}); repeats are told by their time");
        }
        let (commands, inbox) = mpsc::channel();
        let thread_conn = Arc::clone(&conn);
        let handle = std::thread::Builder::new()
            .name("hotkeys".into())
            .spawn(move || run(&thread_conn, root, wake, &inbox, &on_event))
            .map_err(text)?;
        Ok(X11 {
            conn,
            wake,
            commands,
            handle: Some(handle),
        })
    }

    pub fn set(&self, bindings: Vec<Binding>) {
        self.send(Some(Command::Set(bindings)));
    }

    pub fn suspend(&self, suspended: bool) {
        self.send(Some(Command::Suspend(suspended)));
    }

    fn send(&self, command: Option<Command>) {
        if self.commands.send(command).is_err() {
            return;
        }
        let event = ClientMessageEvent::new(32, self.wake, AtomEnum::NONE, [0u32; 5]);
        // With no event mask the message goes to the window's creator: this connection's thread.
        let sent = self
            .conn
            .send_event(false, self.wake, EventMask::NO_EVENT, event)
            .map(drop)
            .and_then(|()| self.conn.flush());
        if let Err(err) = sent {
            log::warn!("hotkeys: cannot wake the thread: {err}");
        }
    }
}

impl Drop for X11 {
    fn drop(&mut self) {
        self.send(None);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

/// Asks the server to send a held key as presses with a single release at the end.
fn detectable_auto_repeat(conn: &RustConnection) -> Result<(), String> {
    let version = conn.xkb_use_extension(1, 0).map_err(text)?.reply().map_err(text)?;
    if !version.supported {
        return Err("XKB not supported".into());
    }
    let flag = xkb::PerClientFlag::DETECTABLE_AUTO_REPEAT;
    let reply = conn
        .xkb_per_client_flags(
            xkb::ID::USE_CORE_KBD.into(),
            flag,
            flag,
            0u32.into(),
            0u32.into(),
            0u32.into(),
        )
        .map_err(text)?
        .reply()
        .map_err(text)?;
    if u32::from(reply.value) & u32::from(flag) == 0 {
        return Err("refused".into());
    }
    Ok(())
}

/// One binding's grab: every keycode that types its keysym, with its modifier mask.
struct Grab {
    action: String,
    keycodes: Vec<Keycode>,
    mask: u16,
}

fn run(
    conn: &RustConnection,
    root: Window,
    wake: Window,
    inbox: &mpsc::Receiver<Option<Command>>,
    on_event: &OnHotkeyEvent,
) {
    let mut wanted = Wanted::default();
    let mut grabs: Vec<Grab> = Vec::new();
    // Keycodes pressed and not yet released, and the last release (keycode, time).
    let mut down: HashSet<Keycode> = HashSet::new();
    let mut last_release: Option<(Keycode, u32)> = None;
    'events: loop {
        let event = match conn.wait_for_event() {
            Ok(event) => event,
            Err(err) => {
                log::warn!("hotkeys: X11 connection lost: {err}");
                break;
            }
        };
        match event {
            Event::KeyPress(press) => {
                let mask = u16::from(press.state) & NAMED_MODIFIERS;
                let Some(grab) = grabs
                    .iter()
                    .find(|g| g.mask == mask && g.keycodes.contains(&press.detail))
                else {
                    continue;
                };
                let repeat = last_release == Some((press.detail, press.time));
                if down.insert(press.detail) && !repeat {
                    on_event(HotkeyEvent::Pressed(grab.action.clone()));
                }
            }
            Event::KeyRelease(release) => {
                down.remove(&release.detail);
                last_release = Some((release.detail, release.time));
            }
            Event::ClientMessage(message) if message.window == wake => {
                let mut changed = false;
                while let Ok(command) = inbox.try_recv() {
                    match command {
                        Some(command) => changed |= wanted.apply(command),
                        None => break 'events,
                    }
                }
                if changed {
                    regrab(conn, root, &mut wanted, &mut grabs, on_event);
                }
            }
            Event::MappingNotify(notify) if notify.request == Mapping::KEYBOARD => {
                regrab(conn, root, &mut wanted, &mut grabs, on_event);
            }
            _ => {}
        }
    }
    release(conn, root, &mut grabs);
    let _ = conn.flush();
}

/// Releases every grab, takes the active keys again and reports them.
fn regrab(conn: &RustConnection, root: Window, wanted: &mut Wanted, grabs: &mut Vec<Grab>, on_event: &OnHotkeyEvent) {
    release(conn, root, grabs);
    let taken = take(conn, root, wanted, grabs);
    let _ = conn.flush();
    wanted.report(&taken, on_event);
}

/// Grabs every active key; returns which bindings hold theirs.
fn take(conn: &RustConnection, root: Window, wanted: &mut Wanted, grabs: &mut Vec<Grab>) -> Vec<bool> {
    let mut taken = vec![false; wanted.bindings.len()];
    let mapping = match keyboard_mapping(conn) {
        Ok(mapping) => mapping,
        Err(err) => {
            log::warn!("hotkeys: cannot read the keyboard mapping: {err}");
            return taken;
        }
    };
    let active: Vec<_> = wanted
        .active()
        .map(|(index, action, key)| (index, action.to_owned(), key))
        .collect();
    for (index, action, key) in active {
        let keycodes = mapping.keycodes(key.key.x11_keysym());
        let grab = Grab {
            action,
            keycodes,
            mask: key.modifiers.x11_mask(),
        };
        if grab.keycodes.is_empty() {
            wanted.refused(key, "no key on this keyboard types it");
            continue;
        }
        match grab_all(conn, root, &grab) {
            Ok(()) => {
                grabs.push(grab);
                taken[index] = true;
            }
            Err(err) => {
                ungrab(conn, root, &grab);
                wanted.refused(key, &err);
            }
        }
    }
    taken
}

fn grab_all(conn: &RustConnection, root: Window, grab: &Grab) -> Result<(), String> {
    for &keycode in &grab.keycodes {
        for lock in LOCKS {
            conn.grab_key(
                false,
                root,
                ModMask::from(grab.mask | lock),
                keycode,
                GrabMode::ASYNC,
                GrabMode::ASYNC,
            )
            .map_err(text)?
            .check()
            .map_err(|err| format!("another app holds it ({err})"))?;
        }
    }
    Ok(())
}

fn ungrab(conn: &RustConnection, root: Window, grab: &Grab) {
    for &keycode in &grab.keycodes {
        for lock in LOCKS {
            let _ = conn.ungrab_key(keycode, root, ModMask::from(grab.mask | lock));
        }
    }
}

fn release(conn: &RustConnection, root: Window, grabs: &mut Vec<Grab>) {
    for grab in grabs.drain(..) {
        ungrab(conn, root, &grab);
    }
}

/// The server's keycode → keysyms table.
struct KeyboardMapping {
    first: Keycode,
    per_keycode: usize,
    keysyms: Vec<u32>,
}

impl KeyboardMapping {
    /// The keycodes whose first group (its two levels) types `keysym`.
    fn keycodes(&self, keysym: u32) -> Vec<Keycode> {
        if self.per_keycode == 0 {
            return Vec::new();
        }
        self.keysyms
            .chunks(self.per_keycode)
            .enumerate()
            .filter(|(_, syms)| syms.iter().take(2).any(|&s| s == keysym))
            .filter_map(|(i, _)| u8::try_from(usize::from(self.first) + i).ok())
            .collect()
    }
}

fn keyboard_mapping(conn: &RustConnection) -> Result<KeyboardMapping, String> {
    let setup = conn.setup();
    let (first, last) = (setup.min_keycode, setup.max_keycode);
    let reply = conn
        .get_keyboard_mapping(first, last - first + 1)
        .map_err(text)?
        .reply()
        .map_err(text)?;
    Ok(KeyboardMapping {
        first,
        per_keycode: usize::from(reply.keysyms_per_keycode),
        keysyms: reply.keysyms,
    })
}
