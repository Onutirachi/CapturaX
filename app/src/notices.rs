//! The settings notices' queue (app shell FR-034, here FR-033; data-model.md "State message and
//! notice"): one notice shown at a time in the notice balloon, in the corner of the Notification
//! position setting, the rest waiting; each id is shown at most once per run.
//!
//! And the event notices (device preview FR-027, screenshot FR-008, research R13): one at a time,
//! the latest replacing the one shown, in the event notice balloon or as a system notification.

use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};

use crate::ui::Texts;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Info,
    Warning,
}

#[derive(Debug, Clone)]
pub struct Notice {
    /// Identifier, for example `settings-damaged`.
    pub id: &'static str,
    pub kind: Kind,
    /// Reads the title from `Texts`, again after a language change.
    pub title: fn(&Texts<'_>) -> slint::SharedString,
    /// Reads the text from `Texts` (research R7), again after a language change (FR-022), with
    /// `arg` (a folder or file it names; empty when it names none).
    pub text: fn(&Texts<'_>, &str) -> slint::SharedString,
    pub arg: String,
}

/// `settings-fallback` (FR-027): the app's folder is read-only, settings go to `folder`.
pub fn settings_fallback(folder: &Path) -> Notice {
    Notice {
        id: "settings-fallback",
        kind: Kind::Info,
        title: |texts| texts.get_settings_fallback_title(),
        text: |texts, folder| texts.invoke_settings_fallback(folder.into()),
        arg: folder.display().to_string(),
    }
}

/// `settings-not-kept` (FR-028): no folder is writable, settings live in memory.
pub fn settings_not_kept() -> Notice {
    Notice {
        id: "settings-not-kept",
        kind: Kind::Warning,
        title: |texts| texts.get_settings_not_kept_title(),
        text: |texts, _| texts.invoke_settings_not_kept(),
        arg: String::new(),
    }
}

/// `settings-damaged` (FR-030): defaults are in use; the damaged file was kept as `kept_as`
/// (beside the settings file, so only its name is shown), or could not be set aside.
pub fn settings_damaged(kept_as: Option<&Path>) -> Notice {
    let name = kept_as
        .and_then(Path::file_name)
        .map(|name| name.to_string_lossy().into_owned());
    Notice {
        id: "settings-damaged",
        kind: Kind::Warning,
        title: |texts| texts.get_settings_damaged_title(),
        text: |texts, name| match name {
            "" => texts.invoke_settings_unreadable(),
            name => texts.invoke_settings_damaged(name.into()),
        },
        arg: name.unwrap_or_default(),
    }
}

#[derive(Default)]
pub struct Notices {
    seen: HashSet<&'static str>,
    current: Option<Notice>,
    waiting: VecDeque<Notice>,
}

impl Notices {
    /// Queues a notice; false if its id was already queued or shown in this run.
    pub fn push(&mut self, notice: Notice) -> bool {
        if !self.seen.insert(notice.id) {
            return false;
        }
        if self.current.is_none() {
            self.current = Some(notice);
        } else {
            self.waiting.push_back(notice);
        }
        true
    }

    /// The notice on screen.
    pub fn current(&self) -> Option<&Notice> {
        self.current.as_ref()
    }

    /// Closes the notice on screen; the next one waiting, if any, takes its place.
    pub fn dismiss(&mut self) {
        self.current = self.waiting.pop_front();
    }
}

/// An event notice (research R13), kept to redo its text in another language.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventNotice {
    /// A device joined or left (device preview FR-027).
    Device {
        kind: crate::preview::NoticeKind,
        device: String,
        /// The device's name as the menu shows it.
        name: String,
        /// It offers **Open**.
        open: bool,
    },
    /// Screenshot FR-008: saved and copied.
    Saved(PathBuf),
    SavedNotCopied(PathBuf),
    CopiedNotSaved(Reason),
    Failed(Reason),
}

/// Why a screenshot was not saved or not taken, in a few words (data-model.md "Outcome").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    FolderNotFound,
    NoPermission,
    DiskFull,
    WriteError,
    ImageError,
}

impl EventNotice {
    /// The balloon's kind (main.slint `event-notice-kind`): its icon and color.
    pub fn kind(&self) -> &'static str {
        use crate::preview::NoticeKind;
        match self {
            Self::Device { kind, .. } => match kind {
                NoticeKind::Connected => "connected",
                NoticeKind::Reconnected => "reconnected",
                NoticeKind::Disconnected => "disconnected",
            },
            Self::Saved(_) => "saved",
            Self::SavedNotCopied(_) => "saved-not-copied",
            Self::CopiedNotSaved(_) => "copied-not-saved",
            Self::Failed(_) => "failed",
        }
    }

    /// The text in the current language ("C920 connected", "Screenshot saved").
    pub fn text(&self, texts: &Texts<'_>) -> slint::SharedString {
        use crate::preview::NoticeKind;
        match self {
            Self::Device { kind, name, .. } => {
                let name = name.as_str().into();
                match kind {
                    NoticeKind::Connected => texts.invoke_notice_connected(name),
                    NoticeKind::Reconnected => texts.invoke_notice_reconnected(name),
                    NoticeKind::Disconnected => texts.invoke_notice_disconnected(name),
                }
            }
            Self::Saved(_) => texts.get_notice_screenshot_saved(),
            Self::SavedNotCopied(_) => texts.get_notice_screenshot_saved_not_copied(),
            Self::CopiedNotSaved(reason) => texts.invoke_notice_screenshot_copied_not_saved(reason.text(texts)),
            Self::Failed(reason) => texts.invoke_notice_screenshot_failed(reason.text(texts)),
        }
    }

    /// What its button does, if it has one.
    pub fn action(&self) -> Option<NoticeAction> {
        match self {
            Self::Device { device, open, .. } => open.then(|| NoticeAction::Open(device.clone())),
            Self::Saved(path) | Self::SavedNotCopied(path) => Some(NoticeAction::Show(path.clone())),
            Self::CopiedNotSaved(_) => Some(NoticeAction::Settings),
            Self::Failed(_) => None,
        }
    }
}

impl Reason {
    fn text(self, texts: &Texts<'_>) -> slint::SharedString {
        match self {
            Self::FolderNotFound => texts.get_screenshot_folder_not_found(),
            Self::NoPermission => texts.get_screenshot_no_permission(),
            Self::DiskFull => texts.get_screenshot_disk_full(),
            Self::WriteError => texts.get_screenshot_write_error(),
            Self::ImageError => texts.get_screenshot_image_error(),
        }
    }
}

/// What an event notice's button does. A system notification carries it in its tag, which comes
/// back when its button is clicked (research R13).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoticeAction {
    /// **Open** the device with this id (device preview FR-027); its tag is the id alone.
    Open(String),
    /// **Show** the file in the file manager (screenshot FR-009).
    Show(PathBuf),
    /// **Settings**, on the screenshots folder (screenshot FR-008).
    Settings,
}

const SHOW_TAG: &str = "show:";
const SETTINGS_TAG: &str = "settings:screenshot";

impl NoticeAction {
    pub fn tag(&self) -> String {
        match self {
            Self::Open(device) => device.clone(),
            Self::Show(path) => format!("{SHOW_TAG}{}", path.to_string_lossy()),
            Self::Settings => SETTINGS_TAG.to_owned(),
        }
    }

    /// The action a tag names; a tag of neither screenshot form is a device id.
    pub fn from_tag(tag: &str) -> Self {
        if tag == SETTINGS_TAG {
            Self::Settings
        } else if let Some(path) = tag.strip_prefix(SHOW_TAG) {
            Self::Show(PathBuf::from(path))
        } else {
            Self::Open(tag.to_owned())
        }
    }

    /// The button's label in the current language.
    pub fn label(&self, texts: &Texts<'_>) -> slint::SharedString {
        match self {
            Self::Open(_) => texts.get_notice_open(),
            Self::Show(_) => texts.get_notice_show(),
            Self::Settings => texts.get_notice_settings(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_notices_have_their_kind_and_action() {
        use crate::preview::NoticeKind;
        let device = |kind, open| EventNotice::Device {
            kind,
            device: "usb:1234".into(),
            name: "C920".into(),
            open,
        };
        let open = Some(NoticeAction::Open("usb:1234".into()));
        assert_eq!(device(NoticeKind::Connected, true).kind(), "connected");
        assert_eq!(device(NoticeKind::Connected, true).action(), open);
        assert_eq!(device(NoticeKind::Reconnected, false).kind(), "reconnected");
        assert_eq!(device(NoticeKind::Disconnected, false).kind(), "disconnected");
        assert_eq!(device(NoticeKind::Disconnected, false).action(), None);
        let file = PathBuf::from("C:/Pictures/CapturaX/CapturaX 2026-11-22 10-00-00.png");
        let show = Some(NoticeAction::Show(file.clone()));
        let cases = [
            (EventNotice::Saved(file.clone()), "saved", show.clone()),
            (EventNotice::SavedNotCopied(file), "saved-not-copied", show),
            (
                EventNotice::CopiedNotSaved(Reason::DiskFull),
                "copied-not-saved",
                Some(NoticeAction::Settings),
            ),
            (EventNotice::Failed(Reason::ImageError), "failed", None),
        ];
        for (notice, kind, action) in cases {
            assert_eq!(notice.kind(), kind);
            assert_eq!(notice.action(), action, "{kind}");
        }
        // The reason changes the text only.
        use Reason::*;
        for reason in [FolderNotFound, NoPermission, DiskFull, WriteError, ImageError] {
            assert_eq!(
                EventNotice::CopiedNotSaved(reason).action(),
                Some(NoticeAction::Settings)
            );
            assert_eq!(EventNotice::Failed(reason).action(), None);
        }
    }

    #[test]
    fn notice_actions_go_through_their_tags() {
        let actions = [
            NoticeAction::Open("usb:1234".into()),
            NoticeAction::Open(r"\\?\usb#vid_046d&pid_082d#8&1a2b#{e5323777}\global".into()),
            NoticeAction::Show(PathBuf::from(
                r"D:\Pictures\CapturaX\CapturaX 2026-11-22 10-00-00 (2).png",
            )),
            NoticeAction::Show(PathBuf::from(
                "/home/me/Pictures/CapturaX/CapturaX 2026-11-22 10-00-00.png",
            )),
            NoticeAction::Settings,
        ];
        for action in actions {
            assert_eq!(NoticeAction::from_tag(&action.tag()), action);
        }
        assert_eq!(NoticeAction::Settings.tag(), "settings:screenshot");
        assert_eq!(NoticeAction::Show(PathBuf::from("/a.png")).tag(), "show:/a.png");
    }

    fn notice(id: &'static str) -> Notice {
        let kind = if id == "a" { Kind::Info } else { Kind::Warning };
        Notice {
            id,
            kind,
            title: |texts| texts.get_app_name(),
            text: |texts, _| texts.get_app_name(),
            arg: String::new(),
        }
    }

    #[test]
    fn shows_one_at_a_time_in_order() {
        let mut n = Notices::default();
        assert!(n.current().is_none());
        assert!(n.push(notice("a")));
        assert!(n.push(notice("b")));
        assert_eq!(n.current().map(|c| c.id), Some("a"));
        n.dismiss();
        assert_eq!(n.current().map(|c| c.id), Some("b"));
        n.dismiss();
        assert!(n.current().is_none());
        n.dismiss();
        assert!(n.current().is_none());
    }

    #[test]
    fn each_id_once_per_run() {
        let mut n = Notices::default();
        assert!(n.push(notice("a")));
        assert!(!n.push(notice("a")), "while shown");
        n.dismiss();
        assert!(!n.push(notice("a")), "after dismissal");
        assert!(n.current().is_none());
    }
}
