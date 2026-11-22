//! Global hotkeys on Windows (research R6): a `hotkeys` thread with its own message queue calls
//! `RegisterHotKey` with no window, so `WM_HOTKEY` arrives in that queue whichever app has the
//! focus. `MOD_NOREPEAT` gives one message per press. Commands come through a channel and a
//! `WM_APP` wakes the thread for them; `WM_QUIT` ends it.

use std::sync::mpsc;
use std::thread::JoinHandle;

use windows_sys::Win32::Foundation::GetLastError;
use windows_sys::Win32::System::Threading::GetCurrentThreadId;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{MOD_NOREPEAT, RegisterHotKey, UnregisterHotKey};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetMessageW, MSG, PM_NOREMOVE, PeekMessageW, PostThreadMessageW, WM_APP, WM_HOTKEY, WM_QUIT,
};

use crate::hotkeys::{Binding, Command, OnHotkeyEvent, Wanted};

pub struct Hotkeys {
    /// The thread's id for `PostThreadMessageW`; 0 when it did not start.
    thread: u32,
    commands: mpsc::Sender<Command>,
    handle: Option<JoinHandle<()>>,
}

impl Hotkeys {
    pub fn start(on_event: OnHotkeyEvent) -> Hotkeys {
        let (commands, inbox) = mpsc::channel();
        let (id_tx, id_rx) = mpsc::sync_channel(1);
        let spawned = std::thread::Builder::new().name("hotkeys".into()).spawn(move || {
            let mut msg = MSG::default();
            // SAFETY: `msg` is a local MSG. Peeking creates this thread's message queue, so a
            // message posted once the id is known is never lost.
            unsafe { PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_NOREMOVE) };
            // SAFETY: plain call.
            let _ = id_tx.send(unsafe { GetCurrentThreadId() });
            run(&inbox, &on_event);
        });
        let (thread, handle) = match spawned {
            Ok(handle) => (id_rx.recv().unwrap_or(0), Some(handle)),
            Err(err) => {
                log::warn!("hotkeys: cannot start the thread: {err}");
                (0, None)
            }
        };
        Hotkeys {
            thread,
            commands,
            handle,
        }
    }

    pub fn set(&self, bindings: Vec<Binding>) {
        self.send(Command::Set(bindings));
    }

    pub fn suspend(&self, suspended: bool) {
        self.send(Command::Suspend(suspended));
    }

    pub fn configure(&self) -> bool {
        false
    }

    pub fn changed_by_desktop(&self) -> bool {
        false
    }

    fn send(&self, command: Command) {
        if self.commands.send(command).is_ok() {
            self.post(WM_APP);
        }
    }

    fn post(&self, message: u32) {
        if self.thread == 0 {
            return;
        }
        // SAFETY: plain call; a thread that already ended makes it fail, which is harmless.
        unsafe { PostThreadMessageW(self.thread, message, 0, 0) };
    }
}

impl Drop for Hotkeys {
    fn drop(&mut self) {
        self.post(WM_QUIT);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

/// The keys this thread holds: the id given to `RegisterHotKey` (the binding's index + 1) and
/// its action.
type Held = Vec<(i32, String)>;

fn run(inbox: &mpsc::Receiver<Command>, on_event: &OnHotkeyEvent) {
    let mut wanted = Wanted::default();
    let mut held = Held::new();
    let mut msg = MSG::default();
    // SAFETY: `msg` is a local MSG; GetMessageW returns 0 on WM_QUIT and -1 on an error.
    while unsafe { GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) } > 0 {
        match msg.message {
            WM_HOTKEY => {
                if let Some((_, action)) = held.iter().find(|(id, _)| *id as usize == msg.wParam) {
                    on_event(crate::hotkeys::HotkeyEvent::Pressed(action.clone()));
                }
            }
            WM_APP => {
                let mut changed = false;
                while let Ok(command) = inbox.try_recv() {
                    changed |= wanted.apply(command);
                }
                if changed {
                    release(&mut held);
                    let taken = take(&mut wanted, &mut held);
                    wanted.report(&taken, on_event);
                }
            }
            _ => {}
        }
    }
    release(&mut held);
}

/// Registers every active key; returns which bindings hold theirs.
fn take(wanted: &mut Wanted, held: &mut Held) -> Vec<bool> {
    let mut taken = vec![false; wanted.bindings.len()];
    let active: Vec<_> = wanted
        .active()
        .map(|(index, action, key)| (index, action.to_owned(), key))
        .collect();
    for (index, action, key) in active {
        let id = index as i32 + 1;
        let modifiers = key.modifiers.windows_mod() | MOD_NOREPEAT;
        // SAFETY: plain call; with no window the hotkey belongs to this thread's queue.
        if unsafe { RegisterHotKey(std::ptr::null_mut(), id, modifiers, u32::from(key.key.windows_vk())) } != 0 {
            held.push((id, action));
            taken[index] = true;
        } else {
            // SAFETY: plain call, right after the failed one.
            let code = unsafe { GetLastError() };
            wanted.refused(key, &std::io::Error::from_raw_os_error(code as i32).to_string());
        }
    }
    taken
}

fn release(held: &mut Held) {
    for (id, _) in held.drain(..) {
        // SAFETY: plain call for an id this thread registered.
        unsafe { UnregisterHotKey(std::ptr::null_mut(), id) };
    }
}
