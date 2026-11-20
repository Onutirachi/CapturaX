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
//!
//! The session's side lives here too: each side is a stream in the node group
//! `capturax.preview`, so PipeWire runs the capture and the playback in one cycle (research R3).
//! A stream's format is held explicitly (F32 interleaved in the node's own channels, the rate
//! left to the graph) and its `process` runs on PipeWire's data thread, where it calls the
//! session's [`Samples`]. A side is opened once the connection's first list has come, so a node
//! that is not there is `Gone` and never replaced by the default. The streams belong to the
//! connection: when it is lost each side reports `Failed`, and the app's retry opens them on the
//! new one.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::rc::{Rc, Weak};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{self, SyncSender};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use ::pipewire as pw;
use pw::properties::properties;
use pw::spa;
use pw::spa::param::ParamType;
use pw::spa::param::audio::{AudioFormat, AudioInfoRaw, AudioInfoRawFlags, MAX_CHANNELS};
use pw::spa::pod::deserialize::PodDeserializer;
use pw::spa::pod::serialize::PodSerializer;
use pw::spa::pod::{CanonicalFixedSizedPod, ChoiceValue, Object, Pod, Value, ValueArray};
use pw::spa::utils::{Choice, ChoiceEnum, Direction};
use pw::stream::{StreamFlags, StreamState};
use pw::types::ObjectType;

use super::audio::usb_key;
use crate::audio::{
    AudioError, AudioEvent, Endpoint, Flow, Format, OnAudioEvent, OnSessionEvent, SampleFormat, Samples, SessionEvent,
    SessionRequest, Side, StreamMode, numbered,
};

/// `EPIPE`: the error the core reports when the connection to the server closes.
const EPIPE: i32 = 32;

/// The wait between two attempts to connect again after the connection is lost.
const RETRY: Duration = Duration::from_secs(1);

/// `node.latency` of both streams (research R3): the quantum asked of the graph while they run.
const LATENCY: &str = "128/48000";

/// `node.group` of both streams: PipeWire schedules a group under one driver.
const GROUP: &str = "capturax.preview";

/// Room, in frames, of the buffer the samples pass through between PipeWire and [`Samples`]; a
/// larger cycle makes it grow once.
const SCRATCH_FRAMES: usize = 8192;

/// How long after its first cycle a stream that reported no delay is asked again, in
/// milliseconds, before its side is reported `Opened` with what it says then.
const SETTLE_MS: u32 = 100;

/// What the app asks of the `pipewire` thread.
pub(super) enum Command {
    /// Report the endpoints to this callback: the list first, then the changes.
    Watch(u64, OnAudioEvent),
    /// Stop reporting to the watcher with this number.
    Unwatch(u64),
    /// Open a session's two sides.
    Open {
        request: SessionRequest,
        samples: Arc<dyn Samples>,
        on_event: OnSessionEvent,
    },
    /// Close one side of the session with this number and open it on `target` (for the output,
    /// None is the system default).
    Reopen {
        number: u64,
        side: Side,
        target: Option<String>,
    },
    /// Close the session with this number.
    Close(u64),
    /// From PipeWire's data thread: the stream with this serial ran its first cycle.
    Running(u64),
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
    let own = sender.clone();
    std::thread::Builder::new()
        .name("pipewire".into())
        .spawn(move || run(receiver, own, &ready))
        .map_err(|e| AudioError::Failed(format!("pipewire thread: {e}")))?;
    match setup.recv() {
        Ok(Ok(())) => Ok(sender),
        Ok(Err(error)) => Err(error),
        Err(_) => Err(AudioError::Failed("the pipewire thread stopped at start".into())),
    }
}

/// The thread's body: connects and reports the setup through `ready`; when that succeeded, runs
/// the loop for the rest of the process, connecting again each time the connection is lost.
/// `own` sends to `commands`: the streams report their first cycle through it.
fn run(
    commands: pw::channel::Receiver<Command>,
    own: pw::channel::Sender<Command>,
    ready: &SyncSender<Result<(), AudioError>>,
) {
    pw::init();
    let reports = Rc::new(Reports::default());
    let sessions = Rc::new(Sessions::new(own));
    let setup = (|| {
        let mainloop = pw::main_loop::MainLoopRc::new(None)?;
        let context = pw::context::ContextRc::new(&mainloop, None)?;
        let connection = Connection::open(&context, &mainloop, &reports, &sessions)?;
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
        let (reports, sessions) = (reports.clone(), sessions.clone());
        move |command| match command {
            Command::Watch(number, on_event) => reports.watch(number, on_event),
            Command::Unwatch(number) => reports.unwatch(number),
            Command::Open {
                request,
                samples,
                on_event,
            } => sessions.open(request, samples, on_event),
            Command::Reopen { number, side, target } => sessions.reopen(number, side, target),
            Command::Close(number) => sessions.close(number),
            Command::Running(serial) => sessions.running(serial),
        }
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
        sessions.lost();
        drop(connection);
        connection = loop {
            match Connection::open(&context, &mainloop, &reports, &sessions) {
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
        sessions: &Rc<Sessions>,
    ) -> Result<Self, pw::Error> {
        let core = context.connect_rc(None)?;
        let registry = core.get_registry_rc()?;
        // The listeners hold the state weakly: the state owns the proxies whose listeners they
        // are.
        let state = Rc::new(State::new(core.clone(), reports.clone(), sessions.clone()));
        *sessions.state.borrow_mut() = Rc::downgrade(&state);
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
    fn watch(&self, number: u64, on_event: OnAudioEvent) {
        if self.listed.get() {
            on_event(self.list());
        }
        self.watchers.borrow_mut().push((number, on_event));
    }

    fn unwatch(&self, number: u64) {
        self.watchers.borrow_mut().retain(|(n, _)| *n != number);
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
    /// The channels' positions (`SPA_AUDIO_CHANNEL_*`) of that format; empty when it has none.
    positions: Vec<u32>,
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
    /// This connection's first list has come: its nodes are known.
    listed: Cell<bool>,
    reports: Rc<Reports>,
    sessions: Rc<Sessions>,
    core: pw::core::CoreRc,
}

impl State {
    fn new(core: pw::core::CoreRc, reports: Rc<Reports>, sessions: Rc<Sessions>) -> Self {
        Self {
            nodes: RefCell::default(),
            devices: RefCell::default(),
            metadata: RefCell::default(),
            default_sink: RefCell::default(),
            pending: Cell::new(None),
            listed: Cell::new(false),
            reports,
            sessions,
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
                            && let Some((format, positions)) = raw_format(pod)
                        {
                            node.format = Some(format);
                            node.positions = positions;
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
                    positions: Vec::new(),
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
        let node = self.nodes.borrow_mut().remove(&id);
        if let Some(node) = node {
            // Kept apart from the nodes: the server may give the same id to the next node before
            // the flush.
            if let Some(endpoint) = self.reports.present.borrow_mut().remove(&id) {
                self.reports.gone.borrow_mut().push(endpoint.id);
            }
            self.sessions.node_removed(&node.name, node.flow);
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
        self.listed.set(true);
        self.sessions.settle();
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

/// The open audio sessions, by number. Kept through every connection; their streams are not.
/// Used on the `pipewire` thread only.
struct Sessions {
    list: RefCell<BTreeMap<u64, Session>>,
    /// Streams that ended inside one of their own callbacks, where they cannot be destroyed;
    /// dropped at the next flush.
    ended: RefCell<Vec<Stream>>,
    /// The connection the streams are opened on.
    state: RefCell<Weak<State>>,
    /// The serial of the next stream; a stream's callbacks carry theirs.
    next: Cell<u64>,
    /// To this thread's own commands, for a stream's first cycle.
    commands: pw::channel::Sender<Command>,
}

struct Session {
    samples: Arc<dyn Samples>,
    on_event: OnSessionEvent,
    input: Wanted,
    output: Wanted,
}

impl Session {
    fn side(&mut self, side: Side) -> &mut Wanted {
        match side {
            Side::Input => &mut self.input,
            Side::Output => &mut self.output,
        }
    }
}

/// One side of a session: what was asked and the stream that plays it.
struct Wanted {
    /// The `node.name` asked for; None (the output only) is the system default.
    target: Option<String>,
    /// Asked before the connection's first list: opened when it comes.
    waiting: bool,
    stream: Option<Stream>,
}

impl Wanted {
    fn new(target: Option<String>) -> Self {
        Self {
            target,
            waiting: false,
            stream: None,
        }
    }
}

/// A connected stream. The listener comes before the stream so it is dropped first.
struct Stream {
    serial: u64,
    /// What the device runs in, when its node says.
    device: Option<Format>,
    channels: u16,
    cycle: Arc<Cycle>,
    listener: pw::stream::StreamListener<()>,
    stream: pw::stream::StreamRc,
}

impl Stream {
    /// Disconnects and destroys the stream. Not for a stream's own callbacks.
    fn close(self) {
        // Without the listener, so the disconnection is not taken for the server's.
        drop(self.listener);
        if let Err(error) = self.stream.disconnect() {
            log::debug!("pipewire: stream disconnect: {error}");
        }
    }
}

/// What a stream's `process` (PipeWire's data thread) shares with the `pipewire` thread.
#[derive(Default)]
struct Cycle {
    /// The rate of the negotiated format; 0 while there is none.
    rate: AtomicU32,
    /// A format was negotiated at least once.
    negotiated: AtomicBool,
    /// Frames of the first cycle; 0 before it.
    period: AtomicU32,
    /// Frames played until the side is reported.
    played: AtomicU32,
    /// The `pipewire` thread was told a second time, [`SETTLE_MS`] after the first cycle.
    settled: AtomicBool,
    /// `Opened` was reported.
    reported: AtomicBool,
}

impl Cycle {
    /// Called every cycle from `process`. The first one tells the `pipewire` thread, which
    /// reports the side `Opened`; when the stream had no delay to report yet (it comes with the
    /// link's latency, a moment after the first cycle: measured), the thread waits to be told
    /// once more.
    fn ran(&self, frames: usize, rate: u32, serial: u64, commands: &pw::channel::Sender<Command>) {
        if self.reported.load(Ordering::Relaxed) {
            return;
        }
        let frames = u32::try_from(frames).unwrap_or(u32::MAX);
        let before = self.played.fetch_add(frames, Ordering::Relaxed);
        if before == 0 {
            self.period.store(frames, Ordering::Relaxed);
        } else if before < rate / 1000 * SETTLE_MS || self.settled.swap(true, Ordering::Relaxed) {
            return;
        }
        let _ = commands.send(Command::Running(serial));
    }
}

impl Sessions {
    fn new(commands: pw::channel::Sender<Command>) -> Self {
        Self {
            list: RefCell::default(),
            ended: RefCell::default(),
            state: RefCell::default(),
            next: Cell::new(1),
            commands,
        }
    }

    fn open(self: &Rc<Self>, request: SessionRequest, samples: Arc<dyn Samples>, on_event: OnSessionEvent) {
        let number = request.number;
        let session = Session {
            samples,
            on_event,
            input: Wanted::new(Some(request.input)),
            output: Wanted::new(request.output),
        };
        let replaced = self.list.borrow_mut().insert(number, session);
        if let Some(mut replaced) = replaced {
            log::warn!("pipewire: session {number} opened twice");
            for side in [Side::Input, Side::Output] {
                if let Some(stream) = replaced.side(side).stream.take() {
                    stream.close();
                }
            }
        }
        // The capture first, as the probe: it runs first in the cycle.
        self.open_side(number, Side::Input);
        self.open_side(number, Side::Output);
    }

    fn reopen(self: &Rc<Self>, number: u64, side: Side, target: Option<String>) {
        let stream = {
            let mut list = self.list.borrow_mut();
            let Some(session) = list.get_mut(&number) else {
                return;
            };
            let wanted = session.side(side);
            wanted.target = target;
            wanted.stream.take()
        };
        if let Some(stream) = stream {
            stream.close();
        }
        self.open_side(number, side);
    }

    fn close(&self, number: u64) {
        let Some(mut session) = self.list.borrow_mut().remove(&number) else {
            return;
        };
        for side in [Side::Input, Side::Output] {
            if let Some(stream) = session.side(side).stream.take() {
                stream.close();
            }
        }
        (session.on_event)(SessionEvent::Closed { number });
    }

    /// Opens a side's stream on the current connection, or leaves it waiting for the
    /// connection's first list; reports the error when it cannot.
    fn open_side(self: &Rc<Self>, number: u64, side: Side) {
        let state = self.state.borrow().upgrade();
        let (target, samples, on_event) = {
            let mut list = self.list.borrow_mut();
            let Some(session) = list.get_mut(&number) else {
                return;
            };
            let (samples, on_event) = (session.samples.clone(), session.on_event.clone());
            let wanted = session.side(side);
            wanted.waiting = state.as_ref().is_some_and(|state| !state.listed.get());
            if wanted.waiting {
                return;
            }
            (wanted.target.clone(), samples, on_event)
        };
        // Not while the list is borrowed: connecting a stream calls its listener.
        let result = match state {
            Some(state) => self.stream(&state, side, target.as_deref(), samples),
            None => Err(AudioError::Failed("PipeWire is not running".into())),
        };
        match result {
            Ok(stream) => {
                if let Some(session) = self.list.borrow_mut().get_mut(&number) {
                    session.side(side).stream = Some(stream);
                }
            }
            Err(error) => on_event(SessionEvent::Error { number, side, error }),
        }
    }

    /// Creates and connects a side's stream with the properties of research R3. `target` is the
    /// node's `node.name`; None leaves the choice to PipeWire, which follows the default output.
    fn stream(
        self: &Rc<Self>,
        state: &State,
        side: Side,
        target: Option<&str>,
        samples: Arc<dyn Samples>,
    ) -> Result<Stream, AudioError> {
        let (flow, direction, category, name) = match side {
            Side::Input => (Flow::Input, Direction::Input, "Capture", "Preview audio input"),
            Side::Output => (Flow::Output, Direction::Output, "Playback", "Preview audio"),
        };
        // The format comes from the node the stream goes to: the one named or the default
        // output. A named node that is not there is gone; PipeWire would pick another.
        let (device, channels, pod) = {
            let default = state.default_sink.borrow();
            let nodes = state.nodes.borrow();
            let node = target
                .or(default.as_deref())
                .and_then(|name| nodes.values().find(|node| node.flow == flow && node.name == name));
            if target.is_some() && node.is_none() {
                return Err(AudioError::Gone);
            }
            let device = node.and_then(|node| node.format);
            let channels = device.map_or(2, |format| format.channels);
            let positions = node.map_or(&[][..], |node| &node.positions);
            let pod = stream_format(channels, positions).ok_or(AudioError::Format)?;
            (device, channels, pod)
        };

        let mut properties = properties! {
            *pw::keys::MEDIA_TYPE => "Audio",
            *pw::keys::MEDIA_CATEGORY => category,
            *pw::keys::MEDIA_ROLE => "Game",
            *pw::keys::NODE_LATENCY => LATENCY,
            *pw::keys::NODE_GROUP => GROUP,
        };
        if let Some(target) = target {
            properties.insert(*pw::keys::TARGET_OBJECT, target);
        }
        let failed = |error: pw::Error| AudioError::Failed(format!("PipeWire: {error}"));
        let stream = pw::stream::StreamRc::new(state.core.clone(), name, properties).map_err(failed)?;

        let serial = self.next.replace(self.next.get() + 1);
        let cycle = Arc::new(Cycle::default());
        let stride = usize::from(channels) * size_of::<f32>();
        let format = move |rate| Format {
            rate,
            channels,
            sample: SampleFormat::F32,
        };
        let mut scratch: Vec<f32> = Vec::with_capacity(SCRATCH_FRAMES * usize::from(channels));
        let listener = stream
            .add_local_listener_with_user_data(())
            .state_changed({
                let (sessions, cycle) = (Rc::downgrade(self), cycle.clone());
                move |_, (), _, new| {
                    let error = match new {
                        // Before any format, the stream found none it could agree on.
                        StreamState::Error(text) if cycle.negotiated.load(Ordering::Relaxed) => {
                            AudioError::Failed(text)
                        }
                        StreamState::Error(text) => {
                            log::warn!("pipewire: stream error before a format: {text}");
                            AudioError::Format
                        }
                        // Not by this thread: its own disconnections drop the listener first.
                        StreamState::Unconnected => AudioError::Failed("PipeWire closed the stream".into()),
                        _ => return,
                    };
                    if let Some(sessions) = sessions.upgrade() {
                        sessions.ended(serial, error);
                    }
                }
            })
            .param_changed({
                let cycle = cycle.clone();
                move |_, (), id, pod| {
                    if id != ParamType::Format.as_raw() {
                        return;
                    }
                    let mut info = AudioInfoRaw::new();
                    let rate = match pod {
                        Some(pod) if info.parse(pod).is_ok() => info.rate(),
                        _ => 0,
                    };
                    cycle.rate.store(rate, Ordering::Relaxed);
                    if rate != 0 {
                        cycle.negotiated.store(true, Ordering::Relaxed);
                    }
                }
            })
            .process({
                let (cycle, commands) = (cycle.clone(), self.commands.clone());
                move |stream, ()| {
                    let Some(mut buffer) = stream.dequeue_buffer() else {
                        return;
                    };
                    let rate = cycle.rate.load(Ordering::Relaxed);
                    let requested = usize::try_from(buffer.requested()).unwrap_or(usize::MAX);
                    let Some(data) = buffer.datas_mut().first_mut() else {
                        return;
                    };
                    if side == Side::Input {
                        let (offset, size) = (data.chunk().offset() as usize, data.chunk().size() as usize);
                        let Some(bytes) = data.data() else { return };
                        let end = offset.saturating_add(size).min(bytes.len());
                        let bytes = &bytes[offset.min(end)..end];
                        let frames = bytes.len() / stride;
                        if rate == 0 || frames == 0 {
                            return;
                        }
                        scratch.clear();
                        let samples_le = bytes[..frames * stride].as_chunks::<4>().0;
                        scratch.extend(samples_le.iter().map(|sample| f32::from_le_bytes(*sample)));
                        samples.on_input(&scratch, format(rate));
                        cycle.ran(frames, rate, serial, &commands);
                    } else {
                        let frames = {
                            let Some(bytes) = data.data() else { return };
                            let room = bytes.len() / stride;
                            // 0 says the stream does not know how much the graph wants.
                            let frames = if requested == 0 { room } else { requested.min(room) };
                            scratch.clear();
                            scratch.resize(frames * usize::from(channels), 0.0);
                            if rate != 0 {
                                samples.fill(&mut scratch, format(rate));
                            }
                            let samples_le = bytes[..frames * stride].as_chunks_mut::<4>().0;
                            for (out, sample) in samples_le.iter_mut().zip(&scratch) {
                                *out = sample.to_le_bytes();
                            }
                            frames
                        };
                        let chunk = data.chunk_mut();
                        *chunk.offset_mut() = 0;
                        *chunk.stride_mut() = stride as i32;
                        *chunk.size_mut() = (frames * stride) as u32;
                        if rate != 0 && frames != 0 {
                            cycle.ran(frames, rate, serial, &commands);
                        }
                    }
                }
            })
            .register()
            .map_err(failed)?;

        let flags = StreamFlags::AUTOCONNECT | StreamFlags::MAP_BUFFERS | StreamFlags::RT_PROCESS;
        let mut params = [Pod::from_bytes(&pod).ok_or(AudioError::Format)?];
        stream.connect(direction, None, flags, &mut params).map_err(failed)?;
        Ok(Stream {
            serial,
            device,
            channels,
            cycle,
            listener,
            stream,
        })
    }

    /// The session and side of the stream with this serial.
    fn find(list: &mut BTreeMap<u64, Session>, serial: u64) -> Option<(u64, Side, &mut Session)> {
        list.iter_mut().find_map(|(number, session)| {
            let side = [Side::Input, Side::Output]
                .into_iter()
                .find(|side| session.side(*side).stream.as_ref().is_some_and(|s| s.serial == serial))?;
            Some((*number, side, session))
        })
    }

    /// A stream ran its first cycle: its side is open.
    fn running(&self, serial: u64) {
        let (on_event, event) = {
            let mut list = self.list.borrow_mut();
            let Some((number, side, session)) = Self::find(&mut list, serial) else {
                return;
            };
            let on_event = session.on_event.clone();
            let Some(stream) = session.side(side).stream.as_mut() else {
                return;
            };
            let cycle = &stream.cycle;
            let rate = cycle.rate.load(Ordering::Relaxed);
            if cycle.reported.load(Ordering::Relaxed) || rate == 0 {
                return;
            }
            // The delay to or from the device, in the time's own rate.
            let latency = stream.stream.time().ok().and_then(|time| {
                let rate = time.rate();
                (rate.denom != 0).then(|| time.delay() as f64 * 1000.0 * f64::from(rate.num) / f64::from(rate.denom))
            });
            // No delay yet: the stream tells once more, and then it is what it is.
            if latency.is_none_or(|ms| ms == 0.0) && !cycle.settled.load(Ordering::Relaxed) {
                return;
            }
            cycle.reported.store(true, Ordering::Relaxed);
            let period = cycle.period.load(Ordering::Relaxed);
            let event = SessionEvent::Opened {
                number,
                side,
                mode: StreamMode::Shared,
                format: stream.device.unwrap_or(Format {
                    rate,
                    channels: stream.channels,
                    sample: SampleFormat::F32,
                }),
                period_ms: f64::from(period) * 1000.0 / f64::from(rate),
                reported_latency_ms: latency,
            };
            (on_event, event)
        };
        on_event(event);
    }

    /// A stream stopped by itself, reported from its own callback: the side closes with this
    /// error. The stream is destroyed at the next flush.
    fn ended(&self, serial: u64, error: AudioError) {
        let found = {
            let mut list = self.list.borrow_mut();
            Self::find(&mut list, serial).and_then(|(number, side, session)| {
                let stream = session.side(side).stream.take()?;
                Some((number, side, session.on_event.clone(), stream))
            })
        };
        let Some((number, side, on_event, stream)) = found else {
            return;
        };
        self.ended.borrow_mut().push(stream);
        if let Some(state) = self.state.borrow().upgrade() {
            state.sync();
        }
        on_event(SessionEvent::Error { number, side, error });
    }

    /// A node left: the sides playing it by name are gone. The default output is PipeWire's to
    /// follow.
    fn node_removed(&self, name: &str, flow: Flow) {
        let side = match flow {
            Flow::Input => Side::Input,
            Flow::Output => Side::Output,
        };
        let serials: Vec<u64> = self
            .list
            .borrow_mut()
            .values_mut()
            .filter_map(|session| {
                let wanted = session.side(side);
                let stream = wanted.stream.as_ref()?;
                (wanted.target.as_deref() == Some(name)).then_some(stream.serial)
            })
            .collect();
        for serial in serials {
            self.ended(serial, AudioError::Gone);
        }
    }

    /// The server has answered everything asked so far: destroys the streams that ended and
    /// opens the sides that waited for the first list.
    fn settle(self: &Rc<Self>) {
        for stream in self.ended.take() {
            stream.close();
        }
        let waiting: Vec<(u64, Side)> = self
            .list
            .borrow_mut()
            .iter_mut()
            .flat_map(|(number, session)| {
                [Side::Input, Side::Output]
                    .into_iter()
                    .filter(|side| session.side(*side).waiting)
                    .map(|side| (*number, side))
                    .collect::<Vec<_>>()
            })
            .collect();
        for (number, side) in waiting {
            self.open_side(number, side);
        }
    }

    /// The connection closed: every stream died with it, and the sides say so.
    fn lost(&self) {
        self.ended.borrow_mut().clear();
        let mut failed = Vec::new();
        for (number, session) in self.list.borrow_mut().iter_mut() {
            for side in [Side::Input, Side::Output] {
                let wanted = session.side(side);
                let open = wanted.stream.take().is_some();
                if std::mem::take(&mut wanted.waiting) || open {
                    failed.push((*number, side, session.on_event.clone()));
                }
            }
        }
        for (number, side, on_event) in failed {
            let error = AudioError::Failed("the connection to PipeWire was lost".into());
            on_event(SessionEvent::Error { number, side, error });
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

/// What a node's `EnumFormat` says the device runs in, with its channels' positions, when it is
/// raw audio in a sample format the platform names. A property left open (the formats a device
/// accepts, a range of rates) is read at its default, the device's own preference.
fn raw_format(pod: &Pod) -> Option<(Format, Vec<u32>)> {
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
    let format = Format {
        rate: u32::try_from(int_of(property(spa::sys::SPA_FORMAT_AUDIO_rate)?)?).ok()?,
        channels: u16::try_from(int_of(property(spa::sys::SPA_FORMAT_AUDIO_channels)?)?).ok()?,
        sample,
    };
    let positions = match property(spa::sys::SPA_FORMAT_AUDIO_position) {
        Some(Value::ValueArray(ValueArray::Id(ids))) => ids.iter().map(|id| id.0).collect(),
        _ => Vec::new(),
    };
    Some((format, positions))
}

/// The `EnumFormat` a stream offers: F32 interleaved in these channels and nothing else, the
/// rate left out so the stream runs at the graph's. `positions` are used when there is one per
/// channel; without them one channel is mono, two are left and right, and more have no position.
/// None when the channel count is not one a stream can have.
fn stream_format(channels: u16, positions: &[u32]) -> Option<Vec<u8>> {
    let count = usize::from(channels);
    if count == 0 || count > MAX_CHANNELS {
        return None;
    }
    let mut info = AudioInfoRaw::new();
    info.set_format(AudioFormat::F32LE);
    info.set_channels(u32::from(channels));
    let mut position = [0; MAX_CHANNELS];
    if positions.len() == count {
        position[..count].copy_from_slice(positions);
    } else {
        match count {
            1 => position[0] = spa::sys::SPA_AUDIO_CHANNEL_MONO,
            2 => {
                position[0] = spa::sys::SPA_AUDIO_CHANNEL_FL;
                position[1] = spa::sys::SPA_AUDIO_CHANNEL_FR;
            }
            _ => info.set_flags(AudioInfoRawFlags::UNPOSITIONED),
        }
    }
    info.set_position(position);
    let object = Object {
        type_: spa::sys::SPA_TYPE_OBJECT_Format,
        id: spa::sys::SPA_PARAM_EnumFormat,
        properties: info.into(),
    };
    let pod = PodSerializer::serialize(std::io::Cursor::new(Vec::new()), &Value::Object(object)).ok()?;
    Some(pod.0.into_inner())
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
    use pw::spa::pod::Property;
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
        let (format, positions) = raw_format(Pod::from_bytes(&pod).unwrap()).unwrap();
        assert_eq!(format.to_string(), "48000/2/i16");
        assert!(positions.is_empty());
    }

    fn info_of(pod: &[u8]) -> AudioInfoRaw {
        let mut info = AudioInfoRaw::new();
        info.parse(Pod::from_bytes(pod).unwrap()).unwrap();
        info
    }

    #[test]
    fn a_stream_holds_the_nodes_channels_and_leaves_the_rate_open() {
        let (fl, fr) = (spa::sys::SPA_AUDIO_CHANNEL_FL, spa::sys::SPA_AUDIO_CHANNEL_FR);
        let (rl, rr) = (spa::sys::SPA_AUDIO_CHANNEL_RL, spa::sys::SPA_AUDIO_CHANNEL_RR);
        let pod = stream_format(4, &[fl, fr, rl, rr]).unwrap();
        let info = info_of(&pod);
        assert_eq!(info.format(), AudioFormat::F32LE);
        assert_eq!((info.rate(), info.channels()), (0, 4));
        assert_eq!(info.position()[..4], [fl, fr, rl, rr]);
        // Without positions: left and right, mono, or none.
        let pod = stream_format(2, &[]).unwrap();
        assert_eq!(info_of(&pod).position()[..2], [fl, fr]);
        let pod = stream_format(1, &[fl, fr]).unwrap();
        assert_eq!(info_of(&pod).position()[0], spa::sys::SPA_AUDIO_CHANNEL_MONO);
        let pod = stream_format(6, &[]).unwrap();
        let info = info_of(&pod);
        assert_eq!(info.channels(), 6);
        assert!(info.flags().contains(AudioInfoRawFlags::UNPOSITIONED));
        assert_eq!(stream_format(0, &[]), None);
        assert_eq!(stream_format(u16::MAX, &[]), None);
    }

    #[test]
    fn a_formats_positions_are_read() {
        let (fl, fr) = (spa::sys::SPA_AUDIO_CHANNEL_FL, spa::sys::SPA_AUDIO_CHANNEL_FR);
        let pod = format_pod(
            spa::sys::SPA_MEDIA_SUBTYPE_raw,
            vec![
                Property::new(spa::sys::SPA_FORMAT_AUDIO_format, Value::Id(sample(AudioFormat::S16LE))),
                Property::new(spa::sys::SPA_FORMAT_AUDIO_rate, Value::Int(48000)),
                Property::new(spa::sys::SPA_FORMAT_AUDIO_channels, Value::Int(2)),
                Property::new(
                    spa::sys::SPA_FORMAT_AUDIO_position,
                    Value::ValueArray(ValueArray::Id(vec![Id(fl), Id(fr)])),
                ),
            ],
        );
        let (_, positions) = raw_format(Pod::from_bytes(&pod).unwrap()).unwrap();
        assert_eq!(positions, [fl, fr]);
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
        let (format, _) = raw_format(Pod::from_bytes(&pod).unwrap()).unwrap();
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
