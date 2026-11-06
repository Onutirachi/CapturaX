//! The notice queue (FR-034, data-model.md "State message and notice"): one notice shown at a
//! time at the top of the video area, the rest waiting; each id is shown at most once per run.

use std::collections::{HashSet, VecDeque};

use crate::ui::Texts;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "the settings location notices create them (spec US3)")
)]
pub enum Kind {
    Info,
    Warning,
}

#[derive(Debug, Clone)]
pub struct Notice {
    /// Identifier, for example `settings-damaged`.
    pub id: &'static str,
    pub kind: Kind,
    /// Reads the text from `Texts` (research R7), again after a language change (FR-022).
    pub text: fn(&Texts<'_>) -> slint::SharedString,
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
            text: |texts| texts.get_app_name(),
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
