//! SC-007: a save interrupted at any moment leaves a settings file that holds either the old or
//! the new values. The test runs its own binary again as a child that saves two sets of values in
//! turn, kills it at a random moment 100 times, and checks the file after each kill.

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use capturax_settings::{FILE_NAME, LanguageSetting, LoadOutcome, Settings, ThemeSetting, file};
use toml::Table;

/// Set to the folder to save in, it makes `child_saves` the saving child.
const CHILD_FOLDER: &str = "CAPTURAX_INTERRUPTED_SAVE_FOLDER";
const KILLS: usize = 100;
const LANGUAGES: &[&str] = &["en", "pt-BR"];

/// The two sets of values the child saves in turn.
fn values() -> [Table; 2] {
    let mut a = Settings::default();
    a.general.theme = ThemeSetting::Dark;
    a.window.x = Some(-1920);
    a.window.monitor = Some(r"\\.\DISPLAY1".into());
    let mut b = Settings::default();
    b.general.language = LanguageSetting::Code("pt-BR".into());
    b.general.theme = ThemeSetting::Light;
    b.general.start_fullscreen = true;
    b.general.keep_in_tray = true;
    b.window.width = 1920;
    b.window.height = 1080;
    b.window.maximized = true;
    [a, b].map(|settings| {
        let mut table = Table::new();
        settings.write_into(&mut table);
        table
    })
}

/// The child: saves the two sets in turn until it is killed.
#[test]
fn child_saves() {
    let Some(folder) = std::env::var_os(CHILD_FOLDER) else {
        return;
    };
    let path = Path::new(&folder).join(FILE_NAME);
    let tables = values();
    let mut out = std::io::stdout();
    let mut first = true;
    for table in tables.iter().cycle() {
        file::save(&path, table).expect("save");
        if std::mem::take(&mut first) {
            // Written straight to stdout (the test harness captures only `print!`), on its own
            // line: the harness leaves "test child_saves ... " open before it.
            out.write_all(b"\nsaving\n").and_then(|()| out.flush()).expect("stdout");
        }
    }
}

#[test]
fn interrupted_save_keeps_old_or_new_values() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join(FILE_NAME);
    let tables = values();
    let mut random = Random::seeded();
    let mut picks = [0; 2];
    for kill in 0..KILLS {
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "child_saves", "--nocapture", "--test-threads=1"])
            .env(CHILD_FOLDER, folder.path())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
        let started = lines.any(|line| line.is_ok_and(|line| line.trim() == "saving"));
        assert!(started, "kill {kill}: the child ended before its first save");
        std::thread::sleep(Duration::from_micros(random.below(20_001)));
        child.kill().unwrap();
        child.wait().unwrap();

        let table = match file::load(&path) {
            LoadOutcome::Loaded(table) => table,
            other => panic!("kill {kill}: {other:?}"),
        };
        let settings = Settings::read(&table, LANGUAGES);
        let pick = tables
            .iter()
            .position(|saved| Settings::read(saved, LANGUAGES) == settings)
            .unwrap_or_else(|| panic!("kill {kill}: the file holds neither set: {table:?}"));
        picks[pick] += 1;
    }
    // Temporary files a kill left behind sit beside the settings file and are never loaded.
    let left: Vec<_> = std::fs::read_dir(folder.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name != FILE_NAME)
        .collect();
    assert!(
        left.iter()
            .all(|name| name.starts_with(".capturax-") && name.ends_with(".tmp")),
        "{left:?}"
    );
    eprintln!(
        "set A {} times, set B {} times, {} temporary files left",
        picks[0],
        picks[1],
        left.len()
    );
}

/// xorshift64: enough to spread the kills, no dependency for it.
struct Random(u64);

impl Random {
    fn seeded() -> Self {
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos() as u64;
        Self(nanos | 1)
    }

    fn below(&mut self, bound: u64) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0 % bound
    }
}
