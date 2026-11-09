//! Toast notifications for a portable app (research R15): with no package identity, the app id
//! `CapturaX` is set on the process and registered for the user under
//! `HKCU\Software\Classes\AppUserModelId\CapturaX` (name and icon), with the icon written as
//! `%LOCALAPPDATA%\CapturaX\notification.ico`. The button comes back through the toast's
//! `Activated` event while the app runs; no COM activator, so a toast never starts the app.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use windows::Data::Xml::Dom::XmlDocument;
use windows::Foundation::TypedEventHandler;
use windows::UI::Notifications::{
    NotificationSetting, ToastActivatedEventArgs, ToastNotification, ToastNotificationManager, ToastNotifier,
};
use windows::core::{HSTRING, IInspectable, Interface};
use windows_sys::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID;

use super::registry;
use crate::notify::{Notice, NoticeIcon};

const APP_ID: &str = "CapturaX";
const APP_ID_KEY: &str = r"Software\Classes\AppUserModelId\CapturaX";
const DISPLAY_NAME: &str = "CapturaX";
const ICON_FILE: &str = "notification.ico";
/// The button's `arguments`, told apart from a click on the toast's body (empty).
const OPEN: &str = "open";

/// The notifier, created on the first notice, and the toast on screen, kept so its `Activated`
/// handler stays alive.
struct Toaster {
    notifier: ToastNotifier,
    shown: Option<ToastNotification>,
}

static TOASTER: Mutex<Option<Toaster>> = Mutex::new(None);
/// Set once a reason for not showing toasts was logged.
static LOGGED: AtomicBool = AtomicBool::new(false);

fn log_once(why: std::fmt::Arguments<'_>) {
    if !LOGGED.swap(true, Ordering::Relaxed) {
        log::warn!("notify: no system notifications: {why}");
    }
}

pub fn init() {
    let id = registry::wide(APP_ID);
    // SAFETY: a NUL-terminated string that outlives the call.
    let result = unsafe { SetCurrentProcessExplicitAppUserModelID(id.as_ptr()) };
    if result < 0 {
        log::warn!("notify: cannot set the app id: {result:#x}");
    }
}

pub fn show(notice: &Notice, icon: NoticeIcon) -> bool {
    let mut toaster = TOASTER.lock().unwrap_or_else(|e| e.into_inner());
    if toaster.is_none() {
        register(icon);
        match ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(APP_ID)) {
            Ok(notifier) => {
                *toaster = Some(Toaster { notifier, shown: None });
            }
            Err(e) => {
                log_once(format_args!("no notifier: {e}"));
                return false;
            }
        }
    }
    let Some(toaster) = toaster.as_mut() else {
        return false;
    };
    // An app id without a Start menu shortcut may not have a readable setting (0x80070490);
    // only an explicit "off" stops the toast.
    match toaster.notifier.Setting() {
        Ok(NotificationSetting::Enabled) => {}
        Ok(setting) => {
            log_once(format_args!("turned off for the app ({setting:?})"));
            return false;
        }
        Err(e) => log::debug!("notify: setting unknown: {e}"),
    }
    let toast = match toast(notice) {
        Ok(toast) => toast,
        Err(e) => {
            log::warn!("notify: cannot build the toast: {e}");
            return false;
        }
    };
    if let Err(e) = toaster.notifier.Show(&toast) {
        log_once(format_args!("cannot show: {e}"));
        return false;
    }
    // Windows shows one popup of an app at a time and queues the next until the previous ends;
    // hiding the previous lets the new one in after its exit animation. Hidden after `Show`: it
    // came in a little sooner than hiding first or replacing through a shared tag (T061).
    if let Some(previous) = toaster.shown.replace(toast) {
        let _ = toaster.notifier.Hide(&previous);
    }
    true
}

/// The toast for `notice`, its button wired to the notice's tag.
fn toast(notice: &Notice) -> windows::core::Result<ToastNotification> {
    let xml = XmlDocument::new()?;
    xml.LoadXml(&HSTRING::from(toast_xml(notice)))?;
    let toast = ToastNotification::CreateToastNotification(&xml)?;
    if notice.action.is_some() {
        let tag = notice.tag.clone();
        toast.Activated(&TypedEventHandler::<ToastNotification, IInspectable>::new(
            move |_, args| {
                let pressed = args
                    .as_ref()
                    .and_then(|args| args.cast::<ToastActivatedEventArgs>().ok())
                    .and_then(|args| args.Arguments().ok())
                    .is_some_and(|arguments| arguments == OPEN);
                if pressed {
                    crate::notify::report_action(tag.clone());
                }
                Ok(())
            },
        ))?;
    }
    Ok(toast)
}

/// Registers the app id's name and icon for the user, rewriting what differs. Failures are
/// logged: the toast may still show, with a generic icon.
fn register(icon: NoticeIcon) {
    let Some(path) = icon_path() else {
        log::warn!("notify: no local app data folder for the icon");
        return;
    };
    let ico = ico(icon);
    if std::fs::read(&path).ok().as_deref() != Some(&ico[..]) {
        let written = path
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| std::fs::write(&path, &ico));
        if let Err(e) = written {
            log::warn!("notify: cannot write {}: {e}", path.display());
        }
    }
    set_if_different("DisplayName", DISPLAY_NAME);
    set_if_different("IconUri", &path.to_string_lossy());
}

fn set_if_different(value: &str, data: &str) {
    if registry::read_string(APP_ID_KEY, value).as_deref() == Some(data) {
        return;
    }
    match registry::set_string(APP_ID_KEY, value, data) {
        Ok(()) => log::info!("notify: registered {value} = {data}"),
        Err(e) => log::warn!("notify: cannot register {value}: {e}"),
    }
}

/// `%LOCALAPPDATA%\CapturaX\notification.ico`.
fn icon_path() -> Option<PathBuf> {
    dirs::data_local_dir().map(|folder| icon_file(&folder))
}

fn icon_file(local: &Path) -> PathBuf {
    local.join(APP_ID).join(ICON_FILE)
}

/// The toast's XML: the title, and one button when the notice has an action.
fn toast_xml(notice: &Notice) -> String {
    let mut xml = format!(
        "<toast><visual><binding template=\"ToastGeneric\"><text>{}</text></binding></visual>",
        escape(&notice.title)
    );
    if let Some(label) = &notice.action {
        xml.push_str(&format!(
            "<actions><action content=\"{}\" arguments=\"{OPEN}\" activationType=\"foreground\"/></actions>",
            escape(label)
        ));
    }
    xml.push_str("</toast>");
    xml
}

fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            c => out.push(c),
        }
    }
    out
}

/// A one-image `.ico`: 32-bit BGRA rows bottom-up and an empty AND mask, as Windows reads them.
fn ico(icon: NoticeIcon) -> Vec<u8> {
    let side = icon.size as usize;
    let mask_row = side.div_ceil(32) * 4;
    let image_len = 40 + side * side * 4 + mask_row * side;
    let mut out = Vec::with_capacity(22 + image_len);
    // ICONDIR: reserved, type 1 (icon), one image.
    for v in [0u16, 1, 1] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    // ICONDIRENTRY: 0 means 256 pixels.
    let byte_side = if side >= 256 { 0 } else { side as u8 };
    out.extend_from_slice(&[byte_side, byte_side, 0, 0]);
    for v in [1u16, 32] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out.extend_from_slice(&(image_len as u32).to_le_bytes());
    out.extend_from_slice(&22u32.to_le_bytes());
    // BITMAPINFOHEADER: the height counts the color rows and the mask's.
    out.extend_from_slice(&40u32.to_le_bytes());
    out.extend_from_slice(&(side as i32).to_le_bytes());
    out.extend_from_slice(&(2 * side as i32).to_le_bytes());
    for v in [1u16, 32] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    for v in [0u32, (side * side * 4) as u32, 0, 0, 0, 0] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    for row in icon.rgba.chunks_exact(side * 4).rev() {
        for &[r, g, b, a] in row.as_chunks::<4>().0 {
            out.extend_from_slice(&[b, g, r, a]);
        }
    }
    out.resize(out.len() + mask_row * side, 0);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn notice(action: Option<&str>) -> Notice {
        Notice {
            title: "Cam <2> & \"Link\" connected".into(),
            action: action.map(String::from),
            tag: "usb:1234".into(),
        }
    }

    #[test]
    fn toast_with_a_button() {
        assert_eq!(
            toast_xml(&notice(Some("Abrir"))),
            "<toast><visual><binding template=\"ToastGeneric\">\
             <text>Cam &lt;2&gt; &amp; &quot;Link&quot; connected</text></binding></visual>\
             <actions><action content=\"Abrir\" arguments=\"open\" activationType=\"foreground\"/></actions>\
             </toast>"
        );
    }

    #[test]
    fn toast_without_a_button() {
        let xml = toast_xml(&notice(None));
        assert!(!xml.contains("<actions>"));
        assert!(xml.ends_with("</binding></visual></toast>"));
    }

    #[test]
    fn ico_layout() {
        // 2 × 2: red, green on the top row; blue, half-transparent white below.
        static RGBA: [u8; 16] = [255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 128];
        let data = ico(NoticeIcon { size: 2, rgba: &RGBA });
        assert_eq!(&data[..6], &[0, 0, 1, 0, 1, 0]);
        assert_eq!(&data[6..10], &[2, 2, 0, 0]);
        let image_len = u32::from_le_bytes(data[14..18].try_into().unwrap()) as usize;
        assert_eq!(u32::from_le_bytes(data[18..22].try_into().unwrap()), 22);
        assert_eq!(data.len(), 22 + image_len);
        assert_eq!(image_len, 40 + 16 + 4 * 2);
        assert_eq!(i32::from_le_bytes(data[30..34].try_into().unwrap()), 4);
        // Bottom row first, BGRA.
        assert_eq!(
            &data[62..78],
            &[255, 0, 0, 255, 255, 255, 255, 128, 0, 0, 255, 255, 0, 255, 0, 255]
        );
        assert!(data[78..].iter().all(|&b| b == 0));
    }

    #[test]
    fn ico_of_256_writes_zero_side() {
        static RGBA: [u8; 256 * 256 * 4] = [0; 256 * 256 * 4];
        let data = ico(NoticeIcon { size: 256, rgba: &RGBA });
        assert_eq!(&data[6..8], &[0, 0]);
    }

    #[test]
    fn icon_goes_to_local_app_data() {
        assert_eq!(
            icon_file(Path::new(r"C:\Users\u\AppData\Local")),
            PathBuf::from(r"C:\Users\u\AppData\Local\CapturaX\notification.ico")
        );
    }
}
