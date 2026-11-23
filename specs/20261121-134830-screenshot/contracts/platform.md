# Contract: Platform Layer (additions)

New public items of `crates/platform` (constitution IV: the only crate with OS calls). Each has a
Windows and a Linux implementation; the app sees this interface only.

## Hotkeys (`hotkeys.rs`)

```rust
pub enum Modifier { Ctrl, Alt, Shift, Super }
pub struct Modifiers(/* set of Modifier */);
pub enum Key { Letter(u8) /* b'A'–b'Z' */, Digit(u8) /* 0–9 */, F(u8) /* 1–24 */, Space, Insert,
               Delete, Home, End, PageUp, PageDown, Up, Down, Left, Right, Pause, PrintScreen }

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Hotkey { pub modifiers: Modifiers, pub key: Key }

impl Hotkey {
    pub fn parse(text: &str) -> Option<Hotkey>;       // "Ctrl+Shift+F8"
    pub fn allowed(&self) -> bool;                     // FR-016 rules
}
impl Display for Hotkey { /* file form: "Ctrl+Shift+F8", "Super" */ }

pub enum HotkeyEvent {
    /// The action's key was pressed; once per press.
    Pressed(String),
    /// The key the system uses for the action, as text to show ("Ctrl+Shift+F8"; on Wayland the
    /// desktop's own description). None: the system bound no key for it.
    Bound { action: String, shown: Option<String> },
    /// Global keys cannot work here (Wayland without the portal): the app matches key events
    /// in its window.
    WindowOnly,
}

pub struct Hotkeys { /* the `hotkeys` thread */ }

impl Hotkeys {
    /// Starts the thread; events come through `on_event` from that thread.
    pub fn start(on_event: impl Fn(HotkeyEvent) + Send + Sync + 'static) -> Hotkeys;
    /// The keys wanted now, per action (`None`: no key). Replaces the previous set; returns at
    /// once. Actions keep their order (the portal's list).
    pub fn set(&self, bindings: Vec<(String, Option<Hotkey>, String /* description */)>);
    /// Releases every key while the user records a new one (FR-017); `false` takes them again.
    pub fn suspend(&self, suspended: bool);
    /// Wayland: opens the desktop's shortcut dialog when the portal offers it; false otherwise.
    pub fn configure(&self) -> bool;
    /// True when keys are changed in the desktop, not in the app (Wayland portal).
    pub fn changed_by_desktop(&self) -> bool;
}
```

Rules: a key the system refuses is logged once and reported as `Bound { shown: None }`; the
thread ends with `Hotkeys` (drop unregisters everything).

Linux picks, in order: X11 grabs in an X11 session; the GlobalShortcuts portal under Wayland;
else `WindowOnly`. Portal rules (research R7, probe T004):

- Before the portal, the app's desktop entry is made sure of: an installed `capturax.desktop` in
  `$XDG_DATA_HOME` or `$XDG_DATA_DIRS` `/applications` is used as it is; when none exists the
  thread writes `$XDG_DATA_HOME/applications/capturax.desktop` (`NoDisplay=true`,
  `X-CapturaX-Written=true`, `Exec` the executable or the AppImage file); an entry with that mark
  whose `Exec` names another program gets only its `Exec` line replaced.
- `Registry.Register("capturax")`, `CreateSession`, then `BindShortcuts` with every action
  (`preferred_trigger` = the setting's `to_portal()`, none for no key). The response and each
  `ShortcutsChanged` give one `Bound` per action (`trigger_description`, `""` → None); a
  cancelled response (1) gives `Bound { shown: None }` for each.
- `set` binds again only when the actions or their descriptions changed (a language switch):
  the session is closed and a new one bound. Key changes from the app are not sent (the
  desktop's key wins).
- `Activated` → `Pressed` (ignored while suspended); `Deactivated` is ignored.
- `changed_by_desktop()` is true once the session exists; `configure()` calls
  `ConfigureShortcuts` (portal version ≥ 2) and returns whether it could.
- No portal, no interface, or a refused registration/session: logged once, `WindowOnly`.

## Clipboard (`clipboard.rs`)

```rust
pub struct Clipboard { /* the `clipboard` thread with one arboard::Clipboard */ }

impl Clipboard {
    pub fn start() -> Clipboard;
    /// Puts an RGBA image on the clipboard; blocks the caller (the screenshot worker) until done
    /// or `timeout`. Err carries a short reason for the log.
    pub fn set_image(&self, width: u32, height: u32, rgba: Arc<[u8]>, timeout: Duration)
        -> Result<(), String>;
}
```

## Files (`files.rs`)

```rust
/// The system's Pictures folder plus "CapturaX" (research R12).
pub fn default_screenshots_folder() -> PathBuf;
/// Opens the system's folder dialog starting at `start`; the answer comes through `done`
/// (None: cancelled or no dialog). Returns false at once when no dialog exists (Linux without
/// the portal).
pub fn pick_folder(owner: &Window, start: &Path, done: impl FnOnce(Option<PathBuf>) + Send + 'static) -> bool;
/// The file manager on the folder with the file selected, else on the folder.
pub fn show_file(file: &Path);
/// The file manager on the folder.
pub fn open_folder(folder: &Path);
```

## Local time (`lib.rs`)

```rust
/// The local time's offset from UTC at `time`, in seconds east (FR-005); 0 when unknown.
/// Windows `FileTimeToLocalFileTime`, Linux `localtime_r` (`tm_gmtoff`), declared like
/// `clock_gettime`: no new dependency.
pub fn utc_offset(time: SystemTime) -> i32;
```

## Output-only session (`audio.rs` change)

`SessionRequest.input` becomes `Option<String>`: `None` opens only the output side, which calls
`Samples::fill` as before and never `on_input`; `reopen_input` on it is ignored. Used by the
app's sound player when no preview session has an open output (research R10). Same events
(`Opened` for the output side only, `Error`, `Closed`), same threads and code on both systems;
no other playback path.

## Notifications

Unchanged API. Screenshot notices use the tags `show:<path>` and `settings:screenshot`.
