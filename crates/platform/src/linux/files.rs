//! The file manager on Linux (research R12): `org.freedesktop.FileManager1` (Dolphin, Nautilus)
//! with the file selected, else the desktop portal's `OpenURI` (`OpenDirectory` with the file,
//! else `OpenURI` with the folder). The folder dialog is the portal's `FileChooser.OpenFile` with
//! `directory`, its answer the `Request`'s `Response` signal.

use std::collections::HashMap;
use std::fs::File;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::Window;
use zbus::blocking::fdo::DBusProxy;
use zbus::blocking::{Connection, Proxy};
use zbus::names::BusName;
use zbus::zvariant::{Fd, OwnedObjectPath, OwnedValue, Value};

use crate::files::{FolderPicked, file_uri, path_from_file_uri};

const PORTAL: &str = "org.freedesktop.portal.Desktop";

/// A file manager or portal the bus starts on demand may take a moment.
const TIMEOUT: Duration = Duration::from_secs(5);

fn connect() -> zbus::Result<Connection> {
    zbus::blocking::connection::Builder::session()?
        .method_timeout(TIMEOUT)
        .build()
}

fn file_manager(connection: &Connection) -> zbus::Result<Proxy<'static>> {
    Proxy::new(
        connection,
        "org.freedesktop.FileManager1",
        "/org/freedesktop/FileManager1",
        "org.freedesktop.FileManager1",
    )
}

fn open_uri(connection: &Connection) -> zbus::Result<Proxy<'static>> {
    Proxy::new(
        connection,
        "org.freedesktop.portal.Desktop",
        "/org/freedesktop/portal/desktop",
        "org.freedesktop.portal.OpenURI",
    )
}

/// The portal's options: none.
fn no_options() -> HashMap<&'static str, Value<'static>> {
    HashMap::new()
}

pub fn show_file(file: &Path) {
    let connection = match connect() {
        Ok(connection) => connection,
        Err(err) => {
            log::warn!("files: no session bus: {err}");
            return;
        }
    };
    let items =
        file_manager(&connection).and_then(|proxy| proxy.call::<_, _, ()>("ShowItems", &(vec![file_uri(file)], "")));
    let Err(items_err) = items else {
        return;
    };
    let directory = File::open(file).map_err(zbus::Error::from).and_then(|opened| {
        open_uri(&connection).and_then(|proxy| {
            proxy.call::<_, _, zbus::zvariant::OwnedObjectPath>("OpenDirectory", &("", Fd::from(&opened), no_options()))
        })
    });
    let Err(directory_err) = directory else {
        return;
    };
    log::info!(
        "files: no file manager to select {} ({items_err}; {directory_err})",
        file.display()
    );
    if let Some(folder) = file.parent() {
        open_with_portal(&connection, folder);
    }
}

pub fn open_folder(folder: &Path) {
    let connection = match connect() {
        Ok(connection) => connection,
        Err(err) => {
            log::warn!("files: no session bus: {err}");
            return;
        }
    };
    let shown = file_manager(&connection)
        .and_then(|proxy| proxy.call::<_, _, ()>("ShowFolders", &(vec![file_uri(folder)], "")));
    if let Err(err) = shown {
        log::info!("files: no file manager for {} ({err})", folder.display());
        open_with_portal(&connection, folder);
    }
}

fn open_with_portal(connection: &Connection, folder: &Path) {
    let opened = open_uri(connection).and_then(|proxy| {
        proxy.call::<_, _, zbus::zvariant::OwnedObjectPath>("OpenURI", &("", file_uri(folder), no_options()))
    });
    if let Err(err) = opened {
        log::warn!("files: cannot open {}: {err}", folder.display());
    }
}

/// True when the bus runs the portal or starts it on demand: asks only the bus itself, so it
/// answers at once.
fn portal_present(connection: &Connection) -> bool {
    let Ok(bus) = DBusProxy::new(connection) else {
        return false;
    };
    let name = BusName::try_from(PORTAL).expect("a valid bus name");
    bus.name_has_owner(name).unwrap_or(false)
        || bus
            .list_activatable_names()
            .is_ok_and(|names| names.iter().any(|activatable| activatable.as_str() == PORTAL))
}

/// The portal's dialog, owned by the window on X11 (`x11:<id>`); on Wayland the window would need
/// an exported handle (xdg-foreign), so the dialog comes without an owner there.
pub fn pick_folder(owner: &Window, start: &Path, done: FolderPicked) -> bool {
    let connection = match connect() {
        Ok(connection) => connection,
        Err(err) => {
            log::warn!("files: no session bus: {err}");
            return false;
        }
    };
    if !portal_present(&connection) {
        log::info!("files: no desktop portal; no folder dialog");
        return false;
    }
    let parent = match owner.window_handle().map(|handle| handle.as_raw()) {
        Ok(RawWindowHandle::Xlib(handle)) => format!("x11:{:x}", handle.window),
        Ok(RawWindowHandle::Xcb(handle)) => format!("x11:{:x}", handle.window),
        _ => String::new(),
    };
    let start = start.to_path_buf();
    crate::files::spawn("folder dialog", move || {
        let picked = match choose_folder(&connection, &parent, &start) {
            Ok(picked) => picked,
            Err(err) => {
                log::warn!("files: folder dialog failed: {err}");
                None
            }
        };
        done(picked);
    });
    true
}

/// The folder chosen, None when cancelled. The dialog may stay open for long: the wait for its
/// answer has no timeout (only the method calls have).
fn choose_folder(connection: &Connection, parent: &str, start: &Path) -> zbus::Result<Option<PathBuf>> {
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let token = format!(
        "capturax_{}_{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    );
    // The request's path is known before the call (portal spec, `Request`): listening to it first
    // means the answer cannot come before the listener.
    let sender = connection
        .unique_name()
        .map(|name| name.trim_start_matches(':').replace('.', "_"))
        .unwrap_or_default();
    let expected = format!("/org/freedesktop/portal/desktop/request/{sender}/{token}");
    let request = request_proxy(connection, expected.clone())?;
    let mut responses = request.receive_signal("Response")?;

    let mut folder = start.as_os_str().as_bytes().to_vec();
    folder.push(0);
    let options: HashMap<&str, Value<'_>> = HashMap::from([
        ("handle_token", Value::from(token.as_str())),
        ("directory", Value::from(true)),
        ("modal", Value::from(true)),
        ("current_folder", Value::from(folder)),
    ]);
    let chooser = Proxy::new(
        connection,
        PORTAL,
        "/org/freedesktop/portal/desktop",
        "org.freedesktop.portal.FileChooser",
    )?;
    let handle: OwnedObjectPath = chooser.call("OpenFile", &(parent, "", options))?;
    // A portal older than the token rule answers on a path of its own.
    let mut other;
    let responses: &mut dyn Iterator<Item = zbus::Message> = if handle.as_str() == expected {
        &mut responses
    } else {
        other = request_proxy(connection, handle.as_str().to_string())?.receive_signal("Response")?;
        &mut other
    };
    let Some(message) = responses.next() else {
        return Err(zbus::Error::Failure(
            "the portal's request ended without an answer".into(),
        ));
    };
    let (response, results): (u32, HashMap<String, OwnedValue>) = message.body().deserialize()?;
    // 0 chosen, 1 cancelled, 2 ended another way.
    if response != 0 {
        return Ok(None);
    }
    let uris: Vec<String> = match results.get("uris") {
        Some(uris) => uris.try_clone()?.try_into()?,
        None => Vec::new(),
    };
    Ok(uris.first().and_then(|uri| path_from_file_uri(uri)))
}

fn request_proxy(connection: &Connection, path: String) -> zbus::Result<Proxy<'static>> {
    Proxy::new_owned(
        connection.clone(),
        PORTAL,
        zbus::zvariant::OwnedObjectPath::try_from(path)?,
        "org.freedesktop.portal.Request",
    )
}
