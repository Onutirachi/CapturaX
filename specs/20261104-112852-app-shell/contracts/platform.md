# Contract: Platform Interface

Crate `platform`: the only place that calls OS-specific APIs (constitution IV). Each function has
a Windows and a Linux implementation selected at compile time; `app` sees one interface. Names
are indicative; tasks fix the exact signatures.

| Function | Windows | Linux | Spec |
|---|---|---|---|
| `app_folder() -> PathBuf` | executable's folder | `$APPIMAGE`'s folder, else executable's | FR-026 |
| `window_attributes(attrs) -> attrs` | `with_undecorated_shadow(true)` | app id `capturax` (`with_name`, Wayland and X11) | FR-006, research R5, R12 |
| `user_config_folder() -> Option<PathBuf>` | `%APPDATA%\CapturaX` | `$XDG_CONFIG_HOME/capturax`, `~/.config/capturax` | FR-027 |
| `is_writable(folder) -> bool` | probe file create + remove | same | FR-026 |
| `claim_instance(name) -> Claim` | named pipe | abstract socket | FR-013, [single-instance.md](single-instance.md) |
| `can_raise_window() -> bool` | true | false on Wayland, true on X11 | FR-013 |
| `autostart::state() -> bool` | Run key and StartupApproved | `autostart/capturax.desktop` | FR-014b |
| `autostart::set(on, exe)` | write or remove the Run value | write or remove the `.desktop` | FR-014b |
| `tray::available() -> bool` | true | StatusNotifierWatcher on the session bus | FR-014a |
| `tray::create(icon, labels, sender) -> Tray` | `tray-icon` | `ksni` | FR-014a |
| `system_theme_watch(sender)` | not used (winit reports it) | XDG Settings portal | FR-017 |
| `show_system_message(title, text)` | `MessageBoxW` | `kdialog`, `zenity`, else stderr | FR-036, FR-013 |
| `snap::attach(window, max_button_rect)` | subclass + `HTMAXBUTTON` (best effort) | no-op | FR-011 |
| `precise_timers() -> TimerGuard` | `timeBeginPeriod(1)` until the guard drops | no-op | FR-004, research R9 |

`platform` depends on winit for `window_attributes` and `snap::attach`; it is the only crate that
uses winit's platform extension traits.

Events from the platform (tray Open / Quit, theme changed, single-instance `show`, snap button
click) reach the main thread as winit user events through an `EventLoopProxy`.
