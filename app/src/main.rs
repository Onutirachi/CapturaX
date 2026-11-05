//! CapturaX: watch and hear capture cards with the lowest possible latency.

mod app;
mod cli;
mod icon;
mod log;
mod notices;
#[cfg_attr(not(test), expect(dead_code, reason = "the window placement task wires it (T032)"))]
mod placement;
mod render;
mod shadows;
mod stats;
mod test_video;
mod ui;

use std::time::Instant;

use winit::event_loop::EventLoop;

/// Language codes the app has; replaced by the language list of the languages task (spec US2).
const KNOWN_LANGUAGES: &[&str] = &["en", "pt-BR"];

fn main() {
    std::process::exit(run());
}

/// The startup sequence (plan): command line, settings and log, UI, window and video thread.
fn run() -> i32 {
    let process_start = Instant::now();
    let options = match cli::parse(std::env::args_os()) {
        Ok(options) => options,
        Err(exit) => {
            if exit.info {
                print!("{}", exit.message);
            } else {
                eprint!("{}", exit.message);
            }
            return exit.code;
        }
    };

    // Settings next to the executable; the per-user and memory fallbacks come with spec US3.
    let folder = platform::app_folder();
    if let Err(err) = log::Logger::new(Some(&folder), ::log::LevelFilter::Info).install() {
        eprintln!("log: {err}");
    }
    ::log::info!(
        "CapturaX {} starting, settings in {}",
        env!("CARGO_PKG_VERSION"),
        folder.display()
    );
    let path = folder.join(settings::FILE_NAME);
    let (table, write_now) = match settings::file::load(&path) {
        settings::LoadOutcome::Loaded(table) => (table, false),
        // The damaged-file notice comes with spec US3.
        settings::LoadOutcome::NoFile | settings::LoadOutcome::Damaged { .. } => (Default::default(), true),
    };
    let store = settings::Store::new(table, Some(path), KNOWN_LANGUAGES);
    if write_now {
        store.save_soon();
    }

    let code = match start(options, process_start, store) {
        Ok(code) => code,
        Err(err) => {
            ::log::error!("{err:#}");
            eprintln!("{err:#}");
            1
        }
    };
    ::log::logger().flush();
    code
}

fn start(options: cli::StartupOptions, process_start: Instant, store: settings::Store) -> anyhow::Result<i32> {
    let ui = ui::Ui::new()?;
    let event_loop = EventLoop::<app::UserEvent>::with_user_event().build()?;
    let mut app = app::App::new(options, process_start, store, ui, event_loop.create_proxy());
    ::log::debug!("options: {:?}", app.options());
    event_loop.run_app(&mut app)?;
    Ok(app.exit_code)
}
