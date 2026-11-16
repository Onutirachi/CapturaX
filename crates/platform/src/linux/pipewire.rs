//! The `pipewire` thread (preview audio research R3, R6, R9): one per process, started by the
//! first [`send`], running a PipeWire main loop that owns the connection and everything on it.
//! Every request from the app is a [`Command`] through a `pipewire::channel`; nothing waits for
//! the thread after its setup, and every outcome comes back through a callback.
//!
//! The monitor's side lives here: the registry's `Audio/Source` and `Audio/Sink` nodes, the
//! devices they belong to and the `default` metadata. PipeWire answers in pieces (the global, then
//! the bound device's info, the node's `EnumFormat`, the metadata's properties), so nothing is
//! reported as it arrives: every change asks the server for a `sync`, and when the latest one is
//! answered the thread compares what it holds with what it reported and sends the difference.
//! That also keeps the order: an endpoint is `Added` before it is named as the default output.
//!
//! When the connection to PipeWire is lost (the server restarted), every endpoint is reported
//! `Removed` and the thread connects again, trying once a second; the endpoints of the new
//! connection are then reported `Added`. The watchers stay through it.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::mpsc::{self, SyncSender};
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use ::pipewire as pw;
use pw::spa;
use pw::spa::param::ParamType;
use pw::spa::param::audio::AudioFormat;
use pw::spa::pod::deserialize::PodDeserializer;
use pw::spa::pod::{CanonicalFixedSizedPod, ChoiceValue, Pod, Value};
use pw::spa::utils::{Choice, ChoiceEnum};
use pw::types::ObjectType;

use super::audio::usb_key;
use crate::audio::{AudioError, AudioEvent, Endpoint, Flow, Format, OnAudioEvent, SampleFormat, numbered};

/// `EPIPE`: the error the core reports when the connection to the server closes.
const EPIPE: i32 = 32;

/// The wait between two attempts to connect again after the connection is lost.
const RETRY: Duration = Duration::from_secs(1);

/// What the app asks of the `pipewire` thread.
pub(super) enum Command {
    /// Report the endpoints to this callback: the list first, then the changes.
    Watch(u64, OnAudioEvent),
    /// Stop reporting to the watcher with this number.
    Unwatch(u64),
}

/// The running thread's command channel; None until the first [`send`] that reaches PipeWire.
static THREAD: Mutex<Option<pw::channel::Sender<Command>>> = Mutex::new(None);

/// Sends a command to the `pipewire` thread, starting it first when it is not running. Waits
/// only for a new thread's setup (the connection to PipeWire, no device); errs when PipeWire
/// cannot be reached.
pub(super) fn send(command: Command) -> Result<(), AudioError> {
    let mut thread = THREAD.lock().unwrap_or_else(PoisonError::into_inner);
    let sender = match &*thread {
        Some(sender) => sender,
        None => thread.insert(start()?),
    };
    sender
        .send(command)
        .map_err(|_| AudioError::Failed("the pipewire thread does not answer".into()))
}

/// Sends a command only when the thread runs; for requests that mean nothing without it.
pub(super) fn post(command: Command) {
    if let Some(sender) = &*THREAD.lock().unwrap_or_else(PoisonError::into_inner) {
        let _ = sender.send(command);
    }
}

fn start() -> Result<pw::channel::Sender<Command>, AudioError> {
    let (sender, receiver) = pw::channel::channel();
    let (ready, setup) = mpsc::sync_channel(1);
    std::thread::Builder::new()
        .name("pipewire".into())
        .spawn(move || run(receiver, &ready))
        .map_err(|e| AudioError::Failed(format!("pipewire thread: {e}")))?;
    match setup.recv() {
        Ok(Ok(())) => Ok(sender),
        Ok(Err(error)) => Err(error),
        Err(_) => Err(AudioError::Failed("the pipewire thread stopped at start".into())),
    }
}

/// The thread's body: connects and reports the setup through `ready`; when that succeeded, runs
/// the loop for the rest of the process, connecting again each time the connection is lost.
fn run(commands: pw::channel::Receiver<Command>, ready: &SyncSender<Result<(), AudioError>>) {
    pw::init();
    let reports = Rc::new(Reports::default());
    let setup = (|| {
        let mainloop = pw::main_loop::MainLoopRc::new(None)?;
        let context = pw::context::ContextRc::new(&mainloop, None)?;
        let connection = Connection::open(&context, &mainloop, &reports)?;
        Ok::<_, pw::Error>((mainloop, context, connection))
    })();
    let (mainloop, context, mut connection) = match setup {
        Ok(setup) => setup,
        Err(error) => {
            log::warn!("pipewire: {error}");
            let _ = ready.send(Err(AudioError::Failed(format!("PipeWire: {error}"))));
            return;
        }
    };
    let _ = ready.send(Ok(()));

    let _commands = commands.attach(mainloop.loop_(), {
        let reports = reports.clone();
        move |command| reports.command(command)
    });
    // Ends the wait between two attempts to connect again; commands are served during it.
    let retry = mainloop.loop_().add_timer({
        let mainloop = mainloop.clone();
        move |_| mainloop.quit()
    });
    loop {
        // Returns when the connection is lost.
        mainloop.run();
        // Here and not where the loss is reported: that is the core's own callback.
        drop(connection);
        connection = loop {
            match Connection::open(&context, &mainloop, &reports) {
                Ok(connection) => break connection,
                Err(error) => {
                    log::debug!("pipewire: not back yet: {error}");
                    let _ = retry.update_timer(Some(RETRY), None);
                    mainloop.run();
                }
            }
        };
        log::info!("pipewire: connected again");
    }
}

/// One connection to PipeWire and what listens on it. The fields are dropped in this order: the
/// listeners, the state with its proxies, then the registry and the core.
struct Connection {
    _core_listener: pw::core::Listener,
    _registry_listener: pw::registry::Listener,
    _state: Rc<State>,
    _registry: pw::registry::RegistryRc,
    _core: pw::core::CoreRc,
}

impl Connection {
    /// Connects and starts listening; the server's globals follow as events. When the connection
    /// is lost, the endpoints are reported removed and `mainloop` is told to quit.
    fn open(
        context: &pw::context::ContextRc,
        mainloop: &pw::main_loop::MainLoopRc,
        reports: &Rc<Reports>,
    ) -> Result<Self, pw::Error> {
        let core = context.connect_rc(None)?;
        let registry = core.get_registry_rc()?;
        // The listeners hold the state weakly: the state owns the proxies whose listeners they
        // are.
        let state = Rc::new(State::new(core.clone(), reports.clone()));
        let core_listener = core
            .add_listener_local()
            .done({
                let state = Rc::downgrade(&state);
                move |id, seq| {
                    if let Some(state) = state.upgrade()
                        && id == pw::core::PW_ID_CORE
                        && state.pending.get() == Some(seq)
                    {
                        state.pending.set(None);
                        state.flush();
                    }
                }
            })
            .error({
                let (reports, mainloop) = (reports.clone(), mainloop.clone());
                move |id, _seq, res, message| {
                    if id == pw::core::PW_ID_CORE && res == -EPIPE {
                        log::warn!("pipewire: connection lost: {message}");
                        reports.lost();
                        mainloop.quit();
                    } else {
                        log::debug!("pipewire: error on object {id}: {message} ({res})");
                    }
                }
            })
            .register();
        let registry_listener = registry
            .add_listener_local()
            .global({
                let (state, registry) = (Rc::downgrade(&state), registry.downgrade());
                move |global| {
                    if let (Some(state), Some(registry)) = (state.upgrade(), registry.upgrade()) {
                        State::global(&state, &registry, global);
                    }
                }
            })
            .global_remove({
                let state = Rc::downgrade(&state);
                move |id| {
                    if let Some(state) = state.upgrade() {
                        state.global_removed(id);
                    }
                }
            })
            .register();
        // The first report comes when the server has sent every global, even when there is none.
        state.sync();
        Ok(Self {
            _core_listener: core_listener,
            _registry_listener: registry_listener,
            _state: state,
            _registry: registry,
            _core: core,
        })
    }
}

/// What the watchers were told; kept through every connection. Used on the `pipewire` thread
/// only.
#[derive(Default)]
struct Reports {
    /// The first list was sent.
    listed: Cell<bool>,
    /// The endpoints reported present, by global id.
    present: RefCell<BTreeMap<u32, Endpoint>>,
    /// Ids of reported endpoints whose node left, to report at the next flush.
    gone: RefCell<Vec<String>>,
    default_output: RefCell<Option<String>>,
    watchers: RefCell<Vec<(u64, OnAudioEvent)>>,
}

impl Reports {
    fn command(&self, command: Command) {
        match command {
            Command::Watch(number, on_event) => {
                if self.listed.get() {
                    on_event(self.list());
                }
                self.watchers.borrow_mut().push((number, on_event));
            }
            Command::Unwatch(number) => self.watchers.borrow_mut().retain(|(n, _)| *n != number),
        }
    }

    fn emit(&self, event: &AudioEvent) {
        for (_, on_event) in self.watchers.borrow().iter() {
            on_event(event.clone());
        }
    }

    /// The first list, from what was reported: numbered per flow, in the registry's order.
    fn list(&self) -> AudioEvent {
        let (mut inputs, mut outputs): (Vec<_>, Vec<_>) = self
            .present
            .borrow()
            .values()
            .cloned()
            .partition(|endpoint| endpoint.flow == Flow::Input);
        numbered(&mut inputs);
        numbered(&mut outputs);
        AudioEvent::Listed {
            inputs,
            outputs,
            default_output: self.default_output.borrow().clone(),
        }
    }

    /// The connection closed: everything reported is gone.
    fn lost(&self) {
        let present = self.present.take();
        let ids = self.gone.take().into_iter().chain(present.into_values().map(|e| e.id));
        for id in ids {
            self.emit(&AudioEvent::Removed(id));
        }
        if self.default_output.take().is_some() {
            self.emit(&AudioEvent::DefaultOutput(None));
        }
    }
}

/// An `Audio/Source` or `Audio/Sink` node of the registry.
struct Node {
    /// `node.name`.
    name: String,
    /// `node.description`.
    description: String,
    flow: Flow,
    /// The global id of the device the node belongs to (`device.id`).
    device: Option<u32>,
    /// From the node's first raw `EnumFormat`.
    format: Option<Format>,
    _proxy: pw::node::Node,
    _listener: pw::node::NodeListener,
}

/// A device of the registry, bound for its `device.bus-path` (the global does not carry it).
struct Device {
    bus_path: Option<String>,
    _proxy: pw::device::Device,
    _listener: pw::device::DeviceListener,
}

/// The `default` metadata object.
struct Metadata {
    id: u32,
    _proxy: pw::metadata::Metadata,
    _listener: pw::metadata::MetadataListener,
}

/// What one connection's server says now, by global id. The proxies come before the core so they
/// are dropped first.
struct State {
    nodes: RefCell<BTreeMap<u32, Node>>,
    devices: RefCell<BTreeMap<u32, Device>>,
    metadata: RefCell<Option<Metadata>>,
    /// The `node.name` in the metadata's `default.audio.sink`.
    default_sink: RefCell<Option<String>>,
    /// The latest `sync` not answered yet; its answer triggers [`State::flush`].
    pending: Cell<Option<spa::utils::result::AsyncSeq>>,
    reports: Rc<Reports>,
    core: pw::core::CoreRc,
}

impl State {
    fn new(core: pw::core::CoreRc, reports: Rc<Reports>) -> Self {
        Self {
            nodes: RefCell::default(),
            devices: RefCell::default(),
            metadata: RefCell::default(),
            default_sink: RefCell::default(),
            pending: Cell::new(None),
            reports,
            core,
        }
    }

    /// Asks the server to answer once it has handled everything sent so far; the answer to the
    /// latest request reports the changes.
    fn sync(&self) {
        match self.core.sync(0) {
            Ok(seq) => self.pending.set(Some(seq)),
            Err(error) => log::warn!("pipewire: sync: {error}"),
        }
    }

    /// A new global: audio nodes, every device and the `default` metadata are bound and kept.
    fn global(
        this: &Rc<Self>,
        registry: &pw::registry::RegistryRc,
        global: &pw::registry::GlobalObject<&spa::utils::dict::DictRef>,
    ) {
        let Some(props) = global.props else { return };
        let id = global.id;
        let state = Rc::downgrade(this);
        match global.type_ {
            ObjectType::Node => {
                // Exactly these two classes: a sink's monitor is a port here, not a node, and
                // streams and virtual sources have other classes.
                let flow = match props.get("media.class") {
                    Some("Audio/Source") => Flow::Input,
                    Some("Audio/Sink") => Flow::Output,
                    _ => return,
                };
                let Some(name) = props.get("node.name") else { return };
                let Ok(proxy) = registry.bind::<pw::node::Node, _>(global) else {
                    return;
                };
                let listener = proxy
                    .add_listener_local()
                    .param(move |_, kind, _, _, pod| {
                        if kind == ParamType::EnumFormat
                            && let (Some(state), Some(pod)) = (state.upgrade(), pod)
                            && let Some(node) = state.nodes.borrow_mut().get_mut(&id)
                            && node.format.is_none()
                        {
                            node.format = raw_format(pod);
                        }
                    })
                    .register();
                proxy.enum_params(0, Some(ParamType::EnumFormat), 0, u32::MAX);
                let node = Node {
                    name: name.to_owned(),
                    description: props.get("node.description").unwrap_or(name).to_owned(),
                    flow,
                    device: props.get("device.id").and_then(|id| id.parse().ok()),
                    format: None,
                    _proxy: proxy,
                    _listener: listener,
                };
                this.nodes.borrow_mut().insert(id, node);
            }
            ObjectType::Device => {
                let Ok(proxy) = registry.bind::<pw::device::Device, _>(global) else {
                    return;
                };
                let listener = proxy
                    .add_listener_local()
                    .info(move |info| {
                        if let Some(state) = state.upgrade()
                            && let Some(path) = info.props().and_then(|props| props.get("device.bus-path"))
                            && let Some(device) = state.devices.borrow_mut().get_mut(&id)
                        {
                            device.bus_path = Some(path.to_owned());
                        }
                    })
                    .register();
                let device = Device {
                    bus_path: None,
                    _proxy: proxy,
                    _listener: listener,
                };
                this.devices.borrow_mut().insert(id, device);
            }
            ObjectType::Metadata if props.get("metadata.name") == Some("default") => {
                let Ok(proxy) = registry.bind::<pw::metadata::Metadata, _>(global) else {
                    return;
                };
                let listener = proxy
                    .add_listener_local()
                    .property(move |subject, key, _, value| {
                        // A key of None says every property of the subject was removed.
                        if subject == pw::core::PW_ID_CORE
                            && matches!(key, Some("default.audio.sink") | None)
                            && let Some(state) = state.upgrade()
                        {
                            *state.default_sink.borrow_mut() = value.and_then(default_name);
                            state.sync();
                        }
                        0
                    })
                    .register();
                *this.metadata.borrow_mut() = Some(Metadata {
                    id,
                    _proxy: proxy,
                    _listener: listener,
                });
            }
            _ => return,
        }
        this.sync();
    }

    fn global_removed(&self, id: u32) {
        if self.nodes.borrow_mut().remove(&id).is_some() {
            // Kept apart from the nodes: the server may give the same id to the next node before
            // the flush.
            if let Some(endpoint) = self.reports.present.borrow_mut().remove(&id) {
                self.reports.gone.borrow_mut().push(endpoint.id);
            }
        } else if self.devices.borrow_mut().remove(&id).is_some() {
            return;
        } else if self.metadata.borrow().as_ref().is_some_and(|m| m.id == id) {
            *self.metadata.borrow_mut() = None;
            *self.default_sink.borrow_mut() = None;
        } else {
            return;
        }
        self.sync();
    }

    fn endpoint(&self, node: &Node) -> Endpoint {
        let devices = self.devices.borrow();
        let bus_path = node
            .device
            .and_then(|id| devices.get(&id))
            .and_then(|device| device.bus_path.as_deref());
        Endpoint {
            id: node.name.clone(),
            name: node.description.clone(),
            base_name: node.description.clone(),
            flow: node.flow,
            link: bus_path.map(|path| usb_key(path).to_owned()),
            format: node.format,
        }
    }

    /// The server has answered everything asked so far: reports what changed since the last
    /// report, removals first, the default output last.
    fn flush(&self) {
        let reports = &self.reports;
        let current: Vec<(u32, Endpoint)> = self
            .nodes
            .borrow()
            .iter()
            .map(|(id, node)| (*id, self.endpoint(node)))
            .collect();
        let default = self.default_sink.borrow().clone();
        if !reports.listed.replace(true) {
            *reports.present.borrow_mut() = current.into_iter().collect();
            *reports.default_output.borrow_mut() = default;
            reports.emit(&reports.list());
            return;
        }
        for id in reports.gone.take() {
            reports.emit(&AudioEvent::Removed(id));
        }
        for (id, endpoint) in current {
            if !reports.present.borrow().contains_key(&id) {
                reports.present.borrow_mut().insert(id, endpoint.clone());
                reports.emit(&AudioEvent::Added(endpoint));
            }
        }
        if *reports.default_output.borrow() != default {
            reports.default_output.borrow_mut().clone_from(&default);
            reports.emit(&AudioEvent::DefaultOutput(default));
        }
    }
}

/// The `node.name` in a `default.audio.sink` value, a JSON object as `{"name":"alsa_output.…"}`.
/// A backslash keeps the next character as it is, which covers the escapes a node name can have
/// (`\"`, `\\`).
fn default_name(value: &str) -> Option<String> {
    let rest = value.split_once("\"name\"")?.1.trim_start().strip_prefix(':')?;
    let mut chars = rest.trim_start().strip_prefix('"')?.chars();
    let mut name = String::new();
    loop {
        match chars.next()? {
            '"' => break,
            '\\' => name.push(chars.next()?),
            c => name.push(c),
        }
    }
    (!name.is_empty()).then_some(name)
}

/// What a node's `EnumFormat` says the device runs in, when it is raw audio in a sample format
/// the platform names. A property left open (the formats a device accepts, a range of rates) is
/// read at its default, the device's own preference.
fn raw_format(pod: &Pod) -> Option<Format> {
    let Ok((_, Value::Object(object))) = PodDeserializer::deserialize_any_from(pod.as_bytes()) else {
        return None;
    };
    let property = |key: u32| object.properties.iter().find(|p| p.key == key).map(|p| &p.value);
    if id_of(property(spa::sys::SPA_FORMAT_mediaType)?)? != spa::sys::SPA_MEDIA_TYPE_audio
        || id_of(property(spa::sys::SPA_FORMAT_mediaSubtype)?)? != spa::sys::SPA_MEDIA_SUBTYPE_raw
    {
        return None;
    }
    let sample = id_of(property(spa::sys::SPA_FORMAT_AUDIO_format)?)?;
    let (_, sample) = [
        (AudioFormat::F32LE, SampleFormat::F32),
        (AudioFormat::F32P, SampleFormat::F32),
        (AudioFormat::S32LE, SampleFormat::I32),
        (AudioFormat::S32P, SampleFormat::I32),
        (AudioFormat::S24_32LE, SampleFormat::I24In32),
        (AudioFormat::S24_32P, SampleFormat::I24In32),
        (AudioFormat::S24LE, SampleFormat::I24),
        (AudioFormat::S24P, SampleFormat::I24),
        (AudioFormat::S16LE, SampleFormat::I16),
        (AudioFormat::S16P, SampleFormat::I16),
    ]
    .into_iter()
    .find(|(format, _)| format.as_raw() == sample)?;
    Some(Format {
        rate: u32::try_from(int_of(property(spa::sys::SPA_FORMAT_AUDIO_rate)?)?).ok()?,
        channels: u16::try_from(int_of(property(spa::sys::SPA_FORMAT_AUDIO_channels)?)?).ok()?,
        sample,
    })
}

fn id_of(value: &Value) -> Option<u32> {
    match value {
        Value::Id(id) => Some(id.0),
        Value::Choice(ChoiceValue::Id(Choice(_, choice))) => Some(default_of(choice).0),
        _ => None,
    }
}

fn int_of(value: &Value) -> Option<i32> {
    match value {
        Value::Int(value) => Some(*value),
        Value::Choice(ChoiceValue::Int(Choice(_, choice))) => Some(default_of(choice)),
        _ => None,
    }
}

/// A choice's default value.
fn default_of<T: CanonicalFixedSizedPod + Copy>(choice: &ChoiceEnum<T>) -> T {
    match choice {
        ChoiceEnum::None(value) => *value,
        ChoiceEnum::Range { default, .. }
        | ChoiceEnum::Step { default, .. }
        | ChoiceEnum::Enum { default, .. }
        | ChoiceEnum::Flags { default, .. } => *default,
    }
}

#[cfg(test)]
mod tests {
    use pw::spa::pod::serialize::PodSerializer;
    use pw::spa::pod::{Object, Property};
    use pw::spa::utils::{ChoiceFlags, Id};

    use super::*;

    #[test]
    fn the_default_sink_name_is_read_from_its_json() {
        assert_eq!(
            default_name(r#"{"name":"alsa_output.pci-0000_00_1f.3.iec958-stereo"}"#).as_deref(),
            Some("alsa_output.pci-0000_00_1f.3.iec958-stereo")
        );
        assert_eq!(
            default_name(r#"{ "name" : "a \"b\" \\ c" }"#).as_deref(),
            Some(r#"a "b" \ c"#)
        );
        assert_eq!(default_name(r#"{"name":""}"#), None);
        assert_eq!(default_name(r#"{"name":"cut"#), None);
        assert_eq!(default_name("{}"), None);
    }

    fn format_pod(subtype: u32, properties: Vec<Property>) -> Vec<u8> {
        let mut all = vec![
            Property::new(
                spa::sys::SPA_FORMAT_mediaType,
                Value::Id(Id(spa::sys::SPA_MEDIA_TYPE_audio)),
            ),
            Property::new(spa::sys::SPA_FORMAT_mediaSubtype, Value::Id(Id(subtype))),
        ];
        all.extend(properties);
        let object = Object {
            type_: spa::sys::SPA_TYPE_OBJECT_Format,
            id: spa::sys::SPA_PARAM_EnumFormat,
            properties: all,
        };
        PodSerializer::serialize(std::io::Cursor::new(Vec::new()), &Value::Object(object))
            .expect("format pod")
            .0
            .into_inner()
    }

    fn sample(format: AudioFormat) -> Id {
        Id(format.as_raw())
    }

    #[test]
    fn a_fixed_format_is_read_as_it_is() {
        // The ezcap's node (measured on CachyOS).
        let pod = format_pod(
            spa::sys::SPA_MEDIA_SUBTYPE_raw,
            vec![
                Property::new(spa::sys::SPA_FORMAT_AUDIO_format, Value::Id(sample(AudioFormat::S16LE))),
                Property::new(spa::sys::SPA_FORMAT_AUDIO_rate, Value::Int(48000)),
                Property::new(spa::sys::SPA_FORMAT_AUDIO_channels, Value::Int(2)),
            ],
        );
        let format = raw_format(Pod::from_bytes(&pod).unwrap()).unwrap();
        assert_eq!(format.to_string(), "48000/2/i16");
    }

    #[test]
    fn an_open_format_is_read_at_its_default() {
        // The internal audio's node: two sample formats and a range of rates.
        let formats = ChoiceEnum::Enum {
            default: sample(AudioFormat::S32LE),
            alternatives: vec![sample(AudioFormat::S32LE), sample(AudioFormat::S16LE)],
        };
        let rates = ChoiceEnum::Range {
            default: 48000,
            min: 32000,
            max: 192_000,
        };
        let pod = format_pod(
            spa::sys::SPA_MEDIA_SUBTYPE_raw,
            vec![
                Property::new(
                    spa::sys::SPA_FORMAT_AUDIO_format,
                    Value::Choice(ChoiceValue::Id(Choice(ChoiceFlags::empty(), formats))),
                ),
                Property::new(
                    spa::sys::SPA_FORMAT_AUDIO_rate,
                    Value::Choice(ChoiceValue::Int(Choice(ChoiceFlags::empty(), rates))),
                ),
                Property::new(spa::sys::SPA_FORMAT_AUDIO_channels, Value::Int(2)),
            ],
        );
        let format = raw_format(Pod::from_bytes(&pod).unwrap()).unwrap();
        assert_eq!(format.to_string(), "48000/2/i32");
    }

    #[test]
    fn other_formats_are_not_read() {
        // The IEC958 output's second entry: encoded audio, no sample format.
        let pod = format_pod(
            spa::sys::SPA_MEDIA_SUBTYPE_iec958,
            vec![Property::new(spa::sys::SPA_FORMAT_AUDIO_rate, Value::Int(48000))],
        );
        assert_eq!(raw_format(Pod::from_bytes(&pod).unwrap()), None);
        // A sample format the platform does not name.
        let pod = format_pod(
            spa::sys::SPA_MEDIA_SUBTYPE_raw,
            vec![
                Property::new(spa::sys::SPA_FORMAT_AUDIO_format, Value::Id(sample(AudioFormat::U8))),
                Property::new(spa::sys::SPA_FORMAT_AUDIO_rate, Value::Int(48000)),
                Property::new(spa::sys::SPA_FORMAT_AUDIO_channels, Value::Int(2)),
            ],
        );
        assert_eq!(raw_format(Pod::from_bytes(&pod).unwrap()), None);
    }
}
