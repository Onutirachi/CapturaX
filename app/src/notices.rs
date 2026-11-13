//! The settings notices' queue (app shell FR-034, here FR-033; data-model.md "State message and
//! notice"): one notice shown at a time in the notice balloon, in the corner of the Notification
//! position setting, the rest waiting; each id is shown at most once per run.

use std::collections::{HashSet, VecDeque};
use std::path::Path;

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

#[cfg(test)]
mod tests {
    use super::*;

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
