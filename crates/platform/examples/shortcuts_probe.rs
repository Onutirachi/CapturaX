//! GlobalShortcuts probe for the screenshot (spec 20261121-134830-screenshot, research R7,
//! quickstart "Probes" 2): binds one global key through the XDG desktop portal and prints what
//! the desktop answers and every press, so the maintainer can check on KDE Plasma (Wayland) that
//! the key arrives while another app has the focus, during a fullscreen game and after a restart.
//!
//! `cargo run -p capturax-platform --example shortcuts_probe -- [<trigger>] [--configure]`
//!
//! - `<trigger>`: the preferred trigger in the portal's form, default `CTRL+SHIFT+F8`.
//! - `--configure`: after binding, calls `ConfigureShortcuts` (portal version 2 and later), which
//!   opens the desktop's own dialog to change the key.
//!
//! Steps: `org.freedesktop.host.portal.Registry.Register("capturax")` when the portal offers it
//! (an unsandboxed app has no id otherwise), `CreateSession`, `BindShortcuts` with the shortcut
//! `screenshot` ("Screenshot"), then every `Activated`, `Deactivated` and `ShortcutsChanged` is
//! printed until Ctrl+C.

#[cfg(target_os = "linux")]
fn main() {
    if let Err(error) = portal::main() {
        eprintln!("shortcuts_probe: {error}");
        std::process::exit(1);
    }
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("shortcuts_probe runs on Linux only (the GlobalShortcuts portal)");
    std::process::exit(1);
}

#[cfg(target_os = "linux")]
mod portal {
    use std::collections::HashMap;

    use zbus::blocking::{Connection, MessageIterator, Proxy};
    use zbus::message::Type;
    use zbus::zvariant::{ObjectPath, OwnedObjectPath, OwnedValue, Value};
    use zbus::{MatchRule, Message};

    const DESTINATION: &str = "org.freedesktop.portal.Desktop";
    const PATH: &str = "/org/freedesktop/portal/desktop";
    const SHORTCUTS: &str = "org.freedesktop.portal.GlobalShortcuts";
    const REGISTRY: &str = "org.freedesktop.host.portal.Registry";
    const REQUEST: &str = "org.freedesktop.portal.Request";
    const APP_ID: &str = "capturax";
    const SHORTCUT_ID: &str = "screenshot";

    type Results = HashMap<String, OwnedValue>;
    type Shortcuts = Vec<(String, Results)>;

    pub fn main() -> zbus::Result<()> {
        let args: Vec<String> = std::env::args().skip(1).collect();
        let configure = args.iter().any(|a| a == "--configure");
        let trigger = args
            .iter()
            .find(|a| !a.starts_with("--"))
            .map_or("CTRL+SHIFT+F8", String::as_str)
            .to_owned();

        let connection = Connection::session()?;
        // Every signal under the portal's path: the requests' `Response` and the shortcuts'
        // signals. Subscribed before any call so no answer is missed.
        let rule = MatchRule::builder()
            .msg_type(Type::Signal)
            .path_namespace(PATH)?
            .build();
        let mut signals = MessageIterator::for_match_rule(rule, &connection, None)?;

        register(&connection);

        let shortcuts = Proxy::new(&connection, DESTINATION, PATH, SHORTCUTS)?;
        let version = shortcuts.get_property::<u32>("version");
        match &version {
            Ok(version) => println!("GlobalShortcuts version {version}"),
            Err(error) => {
                println!("no GlobalShortcuts interface: {error}");
                return Ok(());
            }
        }

        let token = "capturax_probe";
        let request = request_path(&connection, "create")?;
        let options = HashMap::from([
            ("handle_token", Value::from("create")),
            ("session_handle_token", Value::from(token)),
        ]);
        let _: OwnedObjectPath = shortcuts.call("CreateSession", &(options,))?;
        let (code, results) = wait_response(&mut signals, &request)?;
        println!("CreateSession response {code}: {results:?}");
        if code != 0 {
            return Ok(());
        }
        let session: String = match results.get("session_handle") {
            Some(value) => value
                .try_clone()
                .ok()
                .and_then(|v| String::try_from(v).ok())
                .unwrap_or_default(),
            None => String::new(),
        };
        let session =
            ObjectPath::try_from(session).map_err(|e| zbus::Error::Failure(format!("bad session handle: {e}")))?;
        println!("session {session}");

        let request = request_path(&connection, "bind")?;
        let shortcut = (
            SHORTCUT_ID,
            HashMap::from([
                ("description", Value::from("Screenshot")),
                ("preferred_trigger", Value::from(trigger.as_str())),
            ]),
        );
        let options = HashMap::from([("handle_token", Value::from("bind"))]);
        println!("BindShortcuts {SHORTCUT_ID} preferred {trigger}");
        let _: OwnedObjectPath = shortcuts.call("BindShortcuts", &(&session, vec![shortcut], "", options))?;
        let (code, results) = wait_response(&mut signals, &request)?;
        println!("BindShortcuts response {code}");
        if let Some(value) = results.get("shortcuts") {
            match value.try_clone().ok().and_then(|v| Shortcuts::try_from(v).ok()) {
                Some(list) => print_shortcuts(&list),
                None => println!("  shortcuts: {value:?}"),
            }
        }
        if code != 0 {
            return Ok(());
        }

        if configure {
            if version.as_ref().is_ok_and(|v| *v >= 2) {
                let options: HashMap<&str, Value> = HashMap::new();
                match shortcuts.call_method("ConfigureShortcuts", &(&session, "", options)) {
                    Ok(_) => println!("ConfigureShortcuts called"),
                    Err(error) => println!("ConfigureShortcuts failed: {error}"),
                }
            } else {
                println!("ConfigureShortcuts needs portal version 2");
            }
        }

        println!("press the key (with another app focused, in a fullscreen game); Ctrl+C quits");
        for message in signals {
            match message {
                Ok(message) => print_signal(&message),
                Err(error) => println!("bad signal: {error}"),
            }
        }
        Ok(())
    }

    /// Registers the app id when the portal offers the host registry; must come before any other
    /// portal call on this connection.
    fn register(connection: &Connection) {
        let result = Proxy::new(connection, DESTINATION, PATH, REGISTRY).and_then(|registry| {
            let options: HashMap<&str, Value> = HashMap::new();
            registry.call_method("Register", &(APP_ID, options)).map(|_| ())
        });
        match result {
            Ok(()) => println!("Registry.Register({APP_ID}) ok"),
            Err(error) => println!("Registry.Register not available: {error}"),
        }
    }

    /// The path of the `Request` a call with this `handle_token` creates.
    fn request_path(connection: &Connection, token: &str) -> zbus::Result<String> {
        let name = connection
            .unique_name()
            .ok_or_else(|| zbus::Error::Failure("no unique name".into()))?;
        let sender = name.as_str().trim_start_matches(':').replace('.', "_");
        Ok(format!("{PATH}/request/{sender}/{token}"))
    }

    /// Reads signals until the `Response` of `request`, printing any other signal.
    fn wait_response(signals: &mut MessageIterator, request: &str) -> zbus::Result<(u32, Results)> {
        for message in signals.by_ref() {
            let message = message?;
            let header = message.header();
            let is_response = header.interface().is_some_and(|i| i.as_str() == REQUEST)
                && header.member().is_some_and(|m| m.as_str() == "Response")
                && header.path().is_some_and(|p| p.as_str() == request);
            if is_response {
                return message.body().deserialize::<(u32, Results)>();
            }
            print_signal(&message);
        }
        Err(zbus::Error::Failure("the bus closed".into()))
    }

    fn print_signal(message: &Message) {
        let header = message.header();
        if header.interface().is_none_or(|i| i.as_str() != SHORTCUTS) {
            return;
        }
        let Some(member) = header.member() else {
            return;
        };
        match member.as_str() {
            "Activated" | "Deactivated" => {
                match message.body().deserialize::<(OwnedObjectPath, String, u64, Results)>() {
                    Ok((_, id, timestamp, _)) => println!("{member} {id} at {timestamp} ms"),
                    Err(error) => println!("{member}: {error}"),
                }
            }
            "ShortcutsChanged" => match message.body().deserialize::<(OwnedObjectPath, Shortcuts)>() {
                Ok((_, list)) => {
                    println!("ShortcutsChanged");
                    print_shortcuts(&list);
                }
                Err(error) => println!("ShortcutsChanged: {error}"),
            },
            other => println!("{other}"),
        }
    }

    fn print_shortcuts(list: &Shortcuts) {
        for (id, values) in list {
            let text = |key: &str| {
                values
                    .get(key)
                    .and_then(|v| v.try_clone().ok())
                    .and_then(|v| String::try_from(v).ok())
                    .unwrap_or_default()
            };
            println!(
                "  {id}: description {:?}, trigger_description {:?}",
                text("description"),
                text("trigger_description")
            );
        }
    }
}
