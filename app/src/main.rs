//! CapturaX: watch and hear capture cards with the lowest possible latency.

mod app;
mod cli;
mod frame;
mod i18n;
mod icon;
#[allow(dead_code)] // Wired to the bubbles in T038.
mod latency;
mod log;
mod notices;
mod placement;
mod preview;
mod render;
mod shadows;
mod stats;
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
    if options.list_devices {
        return list_devices();
    }

    // Plan step 2: where settings and log are kept (FR-026–FR-028), then the settings (FR-030).
    let location = settings_location();
    if let Err(err) = log::Logger::new(location.folder(), ::log::LevelFilter::Info).install() {
        eprintln!("log: {err}");
    }
    ::log::info!(
        "CapturaX {} starting, settings: {:?}",
        env!("CARGO_PKG_VERSION"),
        location.kind
    );
    let mut notices = Vec::new();
    match &location.kind {
        settings::LocationKind::AppFolder(_) => {}
        settings::LocationKind::UserFolder(folder) => notices.push(notices::settings_fallback(folder)),
        settings::LocationKind::Memory => notices.push(notices::settings_not_kept()),
    }
    let (table, write_now) = match location.load_path() {
        Some(path) => {
            if location.read_from.is_some() {
                ::log::info!("settings read from {}", path.display());
            }
            match settings::file::load(&path) {
                settings::LoadOutcome::Loaded(table) => (table, false),
                settings::LoadOutcome::NoFile => (Default::default(), true),
                settings::LoadOutcome::Damaged { kept_as } => {
                    notices.push(notices::settings_damaged(kept_as.as_deref()));
                    (Default::default(), true)
                }
            }
        }
        None => (Default::default(), false),
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
    // writes anything. In memory the name comes from the app folder's file.
    let instance_file = location
        .file()
        .unwrap_or_else(|| platform::app_folder().join(settings::FILE_NAME));
    let instance = match platform::claim_instance(&platform::instance_name(&instance_file)) {
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
    // No other instance saves in this folder now: the temporary files of interrupted saves can go
    // (contracts/settings-file.md rule 6).
    if let Some(folder) = location.folder() {
        let removed = settings::file::remove_temps(folder);
        if removed > 0 {
            ::log::info!("removed {removed} temporary files of interrupted saves");
        }
    }
    let store = settings::Store::new(table, location.file(), &i18n::codes());
    if write_now {
        store.save_soon();
    }
    follow_autostart();

    let code = match start(options, process_start, store, instance, language, notices) {
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

/// Resolves the settings location (data-model.md "Settings location"): the per-user folder is
/// probed, and so created, only when the app folder is not writable.
fn settings_location() -> settings::Location {
    let app_folder = platform::app_folder();
    let app_writable = platform::is_writable(&app_folder);
    let user_folder = if app_writable {
        None
    } else {
        platform::user_config_folder()
    };
    let user_writable = user_folder.as_deref().is_some_and(platform::is_writable);
    let times = settings::FileTimes {
        app: file_time(&app_folder),
        user: user_folder.as_deref().and_then(file_time),
    };
    settings::location::resolve(&app_folder, app_writable, user_folder.as_deref(), user_writable, times)
}

/// When the settings file in `folder` was last changed, if it exists and can be opened.
fn file_time(folder: &std::path::Path) -> Option<std::time::SystemTime> {
    let file = std::fs::File::open(folder.join(settings::FILE_NAME)).ok()?;
    file.metadata().and_then(|meta| meta.modified()).ok()
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
            platform::show_system_message(
                platform::MessageKind::Warning,
                &texts.get_app_name(),
                &texts.get_already_open(),
            );
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
    notices: Vec<notices::Notice>,
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
    for notice in notices {
        app.notify(notice);
    }
    ::log::debug!("options: {:?}", app.options());
    event_loop.run_app(&mut app)?;
    if app.exit_code == app::EXIT_NO_RENDERER {
        let (title, text) = app.renderer_unavailable();
        platform::show_system_message(platform::MessageKind::Error, &title, &text);
    }
    Ok(app.exit_code)
}

/// `--list-devices` (contracts/cli.md): every device with its id, the automatic modes it shows and
/// what they open, and its modes; exit 69 when GStreamer cannot start.
fn list_devices() -> i32 {
    let (sender, receiver) = std::sync::mpsc::channel();
    let sender = std::sync::Mutex::new(sender);
    let monitor = capture::Monitor::start(move |event| {
        if let capture::MonitorEvent::Listed(devices) = event
            && let Ok(sender) = sender.lock()
        {
            let _ = sender.send(devices);
        }
    });
    let mut monitor = match monitor {
        Ok(monitor) => monitor,
        Err(err) => {
            eprintln!("devices unavailable: {err}");
            return app::EXIT_NO_TEST_VIDEO;
        }
    };
    let devices = receiver
        .recv_timeout(std::time::Duration::from_secs(10))
        .unwrap_or_default();
    monitor.stop();
    if devices.is_empty() {
        println!("No capture device found.");
    }
    for device in &devices {
        println!("{}", device.name);
        println!("  id: {}", device.id);
        let automatic = capture::mode::shown_automatic(&device.modes, device.signal);
        // Aligned columns; the settings text first, so a line can be copied into the settings
        // file (contracts/cli.md).
        let width = device.modes.iter().map(|m| m.text().len()).max().unwrap_or(0);
        let choices = automatic.iter().map(|(c, _)| c.to_string().len()).max().unwrap_or(0);
        println!("  automatic:");
        for (choice, mode) in &automatic {
            let choice = choice.to_string();
            println!("    {choice:<choices$} -> {}", mode_line(mode, width));
        }
        println!("  modes:");
        for mode in &device.modes {
            println!("    {}", mode_line(mode, width));
        }
    }
    0
}

/// `1920x1080@60   NV12  [Full HD] [16:9]`, the settings text padded to `width`.
fn mode_line(mode: &capture::Mode, width: usize) -> String {
    let tags = mode.tags();
    format!(
        "{:<width$}  {:<4}  [{}] [{}]",
        mode.text(),
        mode.format.name(),
        tags.name.unwrap_or("Custom"),
        tags.aspect
    )
}
