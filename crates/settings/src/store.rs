//! The settings in memory and their saver (research R6): every change marks them dirty, a thread
//! saves 300 ms after the last change, and [`Store::flush`] saves at once on quit.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use toml::Table;

use crate::file;
use crate::model::Settings;

const DEBOUNCE: Duration = Duration::from_millis(300);

pub struct Store {
    shared: Arc<Shared>,
    saver: Option<JoinHandle<()>>,
}

struct Shared {
    state: Mutex<State>,
    wake: Condvar,
    /// Held while a snapshot is taken and written, so saves reach the disk in change order.
    saving: Mutex<()>,
    delay: Duration,
}

struct State {
    /// The loaded document; known values are written into it, unknown ones are kept.
    table: Table,
    settings: Settings,
    /// None: settings live in memory only (FR-028).
    path: Option<PathBuf>,
    dirty: bool,
    last_change: Instant,
    stop: bool,
    saves: usize,
}

impl Store {
    /// Settings read from `table` (empty for defaults), saved to `path` when there is one.
    pub fn new(table: Table, path: Option<PathBuf>, known_languages: &[&str]) -> Self {
        Self::with_delay(table, path, known_languages, DEBOUNCE)
    }

    fn with_delay(table: Table, path: Option<PathBuf>, known_languages: &[&str], delay: Duration) -> Self {
        let settings = Settings::read(&table, known_languages);
        let shared = Arc::new(Shared {
            state: Mutex::new(State {
                table,
                settings,
                path,
                dirty: false,
                last_change: Instant::now(),
                stop: false,
                saves: 0,
            }),
            wake: Condvar::new(),
            saving: Mutex::new(()),
            delay,
        });
        let saver = {
            let shared = shared.clone();
            std::thread::Builder::new()
                .name("settings".into())
                .spawn(move || saver(&shared))
                .map_err(|err| log::error!("settings saver thread: {err}"))
                .ok()
        };
        Self { shared, saver }
    }

    pub fn get(&self) -> Settings {
        self.shared.lock().settings.clone()
    }

    /// Where the settings are saved; none when they live in memory only.
    pub fn path(&self) -> Option<PathBuf> {
        self.shared.lock().path.clone()
    }

    /// Changes the settings; they are saved shortly after the last change.
    pub fn update(&self, change: impl FnOnce(&mut Settings)) {
        let mut state = self.shared.lock();
        let before = state.settings.clone();
        change(&mut state.settings);
        if state.settings != before {
            state.dirty = true;
            state.last_change = Instant::now();
            drop(state);
            self.shared.wake.notify_all();
        }
    }

    /// Saves soon even without a change: there was no file, or it was damaged and set aside, so
    /// the user finds one with the current values next to the app.
    pub fn save_soon(&self) {
        let mut state = self.shared.lock();
        state.dirty = true;
        state.last_change = Instant::now();
        drop(state);
        self.shared.wake.notify_all();
    }

    /// Saves now if anything changed since the last save (called on quit).
    pub fn flush(&self) {
        self.shared.save_if_dirty();
    }

    #[cfg(test)]
    fn saves(&self) -> usize {
        self.shared.lock().saves
    }
}

impl Drop for Store {
    fn drop(&mut self) {
        self.shared.lock().stop = true;
        self.shared.wake.notify_all();
        if let Some(saver) = self.saver.take() {
            let _ = saver.join();
        }
    }
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn save_if_dirty(&self) {
        let _saving = self.saving.lock().unwrap_or_else(|e| e.into_inner());
        let mut state = self.lock();
        if !state.dirty {
            return;
        }
        state.dirty = false;
        let State { table, settings, .. } = &mut *state;
        settings.write_into(table);
        let (Some(path), table) = (state.path.clone(), state.table.clone()) else {
            return;
        };
        state.saves += 1;
        drop(state);
        write(&path, &table);
    }
}

fn write(path: &Path, table: &Table) {
    if let Err(err) = file::save(path, table) {
        log::warn!("could not save the settings to {}: {err}", path.display());
    }
}

fn saver(shared: &Shared) {
    let mut state = shared.lock();
    loop {
        if state.stop {
            return;
        }
        if !state.dirty {
            state = shared.wake.wait(state).unwrap_or_else(|e| e.into_inner());
            continue;
        }
        let due = state.last_change + shared.delay;
        let now = Instant::now();
        if now < due {
            state = shared
                .wake
                .wait_timeout(state, due - now)
                .unwrap_or_else(|e| e.into_inner())
                .0;
            continue;
        }
        drop(state);
        shared.save_if_dirty();
        state = shared.lock();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ThemeSetting;

    const LANGUAGES: &[&str] = &["en", "pt-BR"];

    fn saved(path: &Path) -> Table {
        std::fs::read_to_string(path).unwrap().parse().unwrap()
    }

    #[test]
    fn several_updates_make_one_save() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("capturax.toml");
        let store = Store::with_delay(Table::new(), Some(path.clone()), LANGUAGES, Duration::from_millis(100));
        for width in 700..720 {
            store.update(|s| s.window.width = width);
        }
        assert_eq!(store.saves(), 0);
        std::thread::sleep(Duration::from_millis(400));
        assert_eq!(store.saves(), 1);
        assert_eq!(saved(&path)["window"]["width"].as_integer(), Some(719));
    }

    #[test]
    fn unchanged_update_does_not_save() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("capturax.toml");
        let store = Store::with_delay(Table::new(), Some(path.clone()), LANGUAGES, Duration::from_millis(10));
        store.update(|_| {});
        store.flush();
        std::thread::sleep(Duration::from_millis(50));
        assert_eq!(store.saves(), 0);
        assert!(!path.exists());
    }

    #[test]
    fn flush_saves_at_once() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("capturax.toml");
        let store = Store::with_delay(Table::new(), Some(path.clone()), LANGUAGES, Duration::from_secs(60));
        store.update(|s| s.general.theme = ThemeSetting::Light);
        store.flush();
        assert_eq!(saved(&path)["general"]["theme"].as_str(), Some("light"));
        // Nothing left to save.
        store.flush();
        assert_eq!(store.saves(), 1);
    }

    #[test]
    fn unknown_values_are_kept_on_save() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("capturax.toml");
        let table: Table = "[general]\nfuture = 1\n[later]\nx = true".parse().unwrap();
        let store = Store::with_delay(table, Some(path.clone()), LANGUAGES, Duration::from_secs(60));
        store.update(|s| s.general.keep_in_tray = true);
        store.flush();
        let t = saved(&path);
        assert_eq!(t["general"]["future"].as_integer(), Some(1));
        assert_eq!(t["later"]["x"].as_bool(), Some(true));
        assert_eq!(t["general"]["keep_in_tray"].as_bool(), Some(true));
    }

    #[test]
    fn save_soon_writes_without_a_change() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("capturax.toml");
        let store = Store::with_delay(Table::new(), Some(path.clone()), LANGUAGES, Duration::from_secs(60));
        store.save_soon();
        store.flush();
        assert_eq!(saved(&path)["general"]["theme"].as_str(), Some("auto"));
    }

    #[test]
    fn memory_only_never_writes() {
        let store = Store::with_delay(Table::new(), None, LANGUAGES, Duration::from_millis(1));
        store.update(|s| s.general.start_fullscreen = true);
        store.flush();
        assert!(store.get().general.start_fullscreen);
        assert_eq!(store.saves(), 0);
        assert_eq!(store.path(), None);
    }

    #[test]
    fn drop_stops_the_saver() {
        let store = Store::with_delay(Table::new(), None, LANGUAGES, Duration::from_secs(60));
        store.update(|s| s.window.maximized = true);
        drop(store);
    }
}
