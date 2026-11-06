//! CapturaX: watch and hear capture cards with the lowest possible latency.

mod app;
mod cli;
mod frame;
mod i18n;
mod icon;
mod log;
mod notices;
mod placement;
mod render;
mod shadows;
mod stats;
mod test_video;
mod theme;
mod ui;

use std::time::Instant;

use slint::Global;
use winit::event_loop::EventLoop;

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
    // The language for this start (FR-022): `--lang` (never saved), else the saved one; automatic
    // follows the system.
    let saved = settings::Settings::read(&table, &i18n::codes()).general.language;
    let language = i18n::resolve(
        options.lang.as_deref().unwrap_or(i18n::setting(&saved)),
        i18n::system_locale().as_deref(),
    );
    ::log::info!("language {language}");
    // One instance per settings location (FR-013): a later start hands over and exits before it
    // writes anything. The in-memory location (spec US3) names it from the executable instead.
    let instance = match platform::claim_instance(&platform::instance_name(&path)) {
        Ok(platform::Claim::First(listener)) => Some(listener),
        Ok(platform::Claim::Running(client)) => {
            hand_over(&client, language);
            ::log::logger().flush();
            return 0;
        }
        Err(err) => {
            ::log::warn!("single instance: cannot claim the channel, starting anyway: {err}");
            None
        }
    };
    let store = settings::Store::new(table, Some(path), &i18n::codes());
    if write_now {
        store.save_soon();
    }
    follow_autostart();

    let code = match start(options, process_start, store, instance, language) {
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

/// With Start with the system on, points the entry to this executable when it starts another one,
/// so a moved portable folder keeps starting (FR-014b). An entry turned off in the system's own
/// tools is left as it is.
fn follow_autostart() {
    use platform::autostart;
    if !autostart::state() {
        return;
    }
    let exe = match autostart::executable() {
        Ok(exe) => exe,
        Err(err) => {
            ::log::warn!("start with the system: {err}");
            return;
        }
    };
    if autostart::target().as_deref() == Some(exe.as_path()) {
        return;
    }
    match autostart::set(true, &exe) {
        Ok(()) => ::log::info!("start with the system: entry now points to {}", exe.display()),
        Err(err) => ::log::warn!("start with the system: {err}"),
    }
}

/// A second start: asks the running instance to show its window (contracts/single-instance.md).
fn hand_over(client: &platform::Client, language: &str) {
    match client.show() {
        Some(platform::Answer::Raised) => ::log::info!("already running: its window was brought to the front"),
        Some(platform::Answer::ShownNotRaised) => {
            ::log::info!("already running: its window was shown but could not come to the front");
            show_already_open(language);
        }
        // `show` logged why.
        None => {}
    }
}

/// The system message `already-open`, its text read from `Texts` (research R7).
fn show_already_open(language: &str) {
    match ui::Ui::new(language) {
        Ok(ui) => {
            let texts = ui::Texts::get(&ui.app);
            platform::show_system_message(&texts.get_app_name(), &texts.get_already_open());
        }
        Err(err) => ::log::warn!("already-open message: {err:#}"),
    }
}

fn start(
    options: cli::StartupOptions,
    process_start: Instant,
    store: settings::Store,
    instance: Option<platform::Listener>,
    language: &'static str,
) -> anyhow::Result<i32> {
    let ui = ui::Ui::new(language)?;
    let event_loop = EventLoop::<app::UserEvent>::with_user_event().build()?;
    // The listener thread hands `show` to the main loop and answers whether the platform lets the
    // window come to the front.
    let instance = instance.and_then(|listener| {
        let proxy = event_loop.create_proxy();
        listener
            .serve(move || {
                let _ = proxy.send_event(app::UserEvent::ShowWindow);
                platform::can_raise_window()
            })
            .inspect_err(|err| ::log::warn!("single instance: cannot listen: {err}"))
            .ok()
    });
    let mut app = app::App::new(options, process_start, store, ui, event_loop.create_proxy(), instance);
    ::log::debug!("options: {:?}", app.options());
    event_loop.run_app(&mut app)?;
    Ok(app.exit_code)
}
