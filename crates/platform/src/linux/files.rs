//! The file manager on Linux (research R12): `org.freedesktop.FileManager1` (Dolphin, Nautilus)
//! with the file selected, else the desktop portal's `OpenURI` (`OpenDirectory` with the file,
//! else `OpenURI` with the folder).

use std::collections::HashMap;
use std::fs::File;
use std::path::Path;
use std::time::Duration;

use zbus::blocking::{Connection, Proxy};
use zbus::zvariant::{Fd, Value};

use crate::files::file_uri;

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
