//! The audio monitor (research R6): the active endpoints from `IMMDeviceEnumerator`, then their
//! changes from an `IMMNotificationClient`.
//!
//! Everything runs on one thread, `audio-monitor`, in the multithreaded COM apartment. The
//! notification callbacks come on system threads and must not block (Microsoft's rules for
//! `IMMNotificationClient`), so they only copy the id and post a message; the monitor's thread
//! reads the endpoint and reports the event. The notifications are registered before the first
//! list is read, so no change is lost; the thread keeps the ids it reported, so a change already
//! in the list is not reported twice.

use std::collections::HashSet;
use std::sync::mpsc::{self, Receiver, Sender, SyncSender};

use windows::Win32::Devices::FunctionDiscovery::{PKEY_Device_ContainerId, PKEY_Device_FriendlyName};
use windows::Win32::Foundation::PROPERTYKEY;
use windows::Win32::Media::Audio::{
    DEVICE_STATE, DEVICE_STATE_ACTIVE, EDataFlow, ERole, IAudioClient, IMMDevice, IMMDeviceEnumerator, IMMEndpoint,
    IMMNotificationClient, IMMNotificationClient_Impl, MMDeviceEnumerator, eAll, eCapture, eConsole, eRender,
};
use windows::Win32::System::Com::StructuredStorage::PropVariantToStringAlloc;
use windows::Win32::System::Com::{CLSCTX_ALL, CoCreateInstance, CoTaskMemFree, STGM_READ};
use windows::core::{Interface, PCWSTR, implement};

use super::{Com, container, format};
use crate::audio::{AudioError, AudioEvent, Endpoint, Flow, OnAudioEvent, numbered};

/// What the notification callbacks and the owner post to the monitor's thread.
enum Message {
    Added(String),
    Removed(String),
    State(String, DEVICE_STATE),
    DefaultOutput(Option<String>),
    Stop,
}

/// The running monitor; its thread stops when this is dropped, without being waited for.
pub struct Monitor {
    messages: Sender<Message>,
}

impl Monitor {
    /// Starts the monitor's thread and waits only for its setup (the enumerator and the
    /// notification registration, no device); the first list follows as an event.
    pub fn start(on_event: OnAudioEvent) -> Result<Self, AudioError> {
        let (messages, receiver) = mpsc::channel();
        let (ready, setup) = mpsc::sync_channel(1);
        let notifications = messages.clone();
        std::thread::Builder::new()
            .name("audio-monitor".into())
            .spawn(move || run(notifications, receiver, on_event, ready))
            .map_err(|e| AudioError::Failed(format!("audio monitor thread: {e}")))?;
        match setup.recv() {
            Ok(Ok(())) => Ok(Self { messages }),
            Ok(Err(error)) => Err(error),
            Err(_) => Err(AudioError::Failed("the audio monitor stopped at start".into())),
        }
    }
}

impl Drop for Monitor {
    fn drop(&mut self) {
        let _ = self.messages.send(Message::Stop);
    }
}

fn run(
    notifications: Sender<Message>,
    messages: Receiver<Message>,
    on_event: OnAudioEvent,
    ready: SyncSender<Result<(), AudioError>>,
) {
    let _com = Com::init();
    // SAFETY: COM is initialized on this thread; the enumerator is used on it only (the client
    // is called by the system on its own threads and only posts).
    let setup = unsafe {
        CoCreateInstance::<_, IMMDeviceEnumerator>(&MMDeviceEnumerator, None, CLSCTX_ALL).and_then(|enumerator| {
            let client: IMMNotificationClient = Notifications {
                messages: notifications,
            }
            .into();
            enumerator.RegisterEndpointNotificationCallback(&client)?;
            Ok((enumerator, client))
        })
    };
    let (enumerator, client) = match setup {
        Ok(objects) => {
            let _ = ready.send(Ok(()));
            objects
        }
        Err(error) => {
            log::warn!("audio monitor: {error}");
            let _ = ready.send(Err(AudioError::Failed(error.to_string())));
            return;
        }
    };

    let mut reported = HashSet::new();
    let listed = list(&enumerator, &mut reported);
    let mut default_output = match &listed {
        AudioEvent::Listed { default_output, .. } => default_output.clone(),
        _ => None,
    };
    on_event(listed);
    for message in messages {
        match message {
            Message::Added(id) | Message::State(id, DEVICE_STATE_ACTIVE) => {
                if !reported.contains(&id)
                    && let Some(endpoint) = active_endpoint(&enumerator, &id)
                {
                    reported.insert(id);
                    on_event(AudioEvent::Added(endpoint));
                }
            }
            Message::Removed(id) | Message::State(id, _) => {
                if reported.remove(&id) {
                    on_event(AudioEvent::Removed(id));
                }
            }
            // Windows calls back twice for one change (measured); only a new value is reported.
            Message::DefaultOutput(id) => {
                if id != default_output {
                    default_output.clone_from(&id);
                    on_event(AudioEvent::DefaultOutput(id));
                }
            }
            Message::Stop => break,
        }
    }
    // SAFETY: the client registered above, on the same enumerator.
    if let Err(error) = unsafe { enumerator.UnregisterEndpointNotificationCallback(&client) } {
        log::warn!("audio monitor: unregister: {error}");
    }
}

/// The first list: every active endpoint, numbered per flow, and the default output.
fn list(enumerator: &IMMDeviceEnumerator, reported: &mut HashSet<String>) -> AudioEvent {
    let (mut inputs, mut outputs) = (Vec::new(), Vec::new());
    // SAFETY: COM calls on the monitor's thread with interfaces created on it.
    let devices = unsafe {
        enumerator
            .EnumAudioEndpoints(eAll, DEVICE_STATE_ACTIVE)
            .and_then(|collection| {
                (0..collection.GetCount()?)
                    .map(|i| collection.Item(i))
                    .collect::<Result<Vec<_>, _>>()
            })
    };
    match devices {
        Ok(devices) => {
            for endpoint in devices.iter().filter_map(endpoint) {
                reported.insert(endpoint.id.clone());
                match endpoint.flow {
                    Flow::Input => inputs.push(endpoint),
                    Flow::Output => outputs.push(endpoint),
                }
            }
        }
        Err(error) => log::warn!("audio monitor: list: {error}"),
    }
    numbered(&mut inputs);
    numbered(&mut outputs);
    // SAFETY: as above; no default output (none present) is an error, read as None.
    let default_output = unsafe { enumerator.GetDefaultAudioEndpoint(eRender, eConsole) }
        .ok()
        .and_then(|device| device_id(&device));
    AudioEvent::Listed {
        inputs,
        outputs,
        default_output,
    }
}

/// The endpoint with this id, if it is active.
fn active_endpoint(enumerator: &IMMDeviceEnumerator, id: &str) -> Option<Endpoint> {
    let id = windows::core::HSTRING::from(id);
    // SAFETY: COM calls on the monitor's thread; `id` outlives the call.
    let device = unsafe { enumerator.GetDevice(&id) }.ok()?;
    // SAFETY: as above.
    (unsafe { device.GetState() }.ok()? == DEVICE_STATE_ACTIVE).then(|| endpoint(&device))?
}

/// What the app knows of an endpoint; None when its id or flow cannot be read.
fn endpoint(device: &IMMDevice) -> Option<Endpoint> {
    let id = device_id(device)?;
    // SAFETY: COM call on the monitor's thread.
    let flow = match unsafe { device.cast::<IMMEndpoint>().and_then(|e| e.GetDataFlow()) } {
        Ok(f) if f == eCapture => Flow::Input,
        Ok(f) if f == eRender => Flow::Output,
        _ => return None,
    };
    let base_name = property(device, &PKEY_Device_FriendlyName).unwrap_or_else(|| id.clone());
    Some(Endpoint {
        name: base_name.clone(),
        base_name,
        flow,
        link: property(device, &PKEY_Device_ContainerId).and_then(|text| container::link(&text)),
        format: mix_format(device),
        id,
    })
}

fn device_id(device: &IMMDevice) -> Option<String> {
    // SAFETY: GetId returns a CoTaskMemAlloc'ed string, freed here after copying it.
    unsafe {
        let id = device.GetId().ok()?;
        let text = id.to_string().ok();
        CoTaskMemFree(Some(id.0 as _));
        text
    }
}

/// An endpoint property as text (a GUID as `{…}`).
fn property(device: &IMMDevice, key: &PROPERTYKEY) -> Option<String> {
    // SAFETY: the property store and value live through the calls; the text is freed after
    // copying it.
    unsafe {
        let store = device.OpenPropertyStore(STGM_READ).ok()?;
        let value = store.GetValue(key).ok()?;
        let text = PropVariantToStringAlloc(&value).ok()?;
        let result = text.to_string().ok();
        CoTaskMemFree(Some(text.0 as _));
        result
    }
}

/// The shared-mode mix format: an audio client is activated to read it, not initialized, so the
/// device is not opened.
fn mix_format(device: &IMMDevice) -> Option<crate::audio::Format> {
    // SAFETY: COM calls on the monitor's thread; the format is WASAPI's allocation, read then
    // freed.
    unsafe {
        let client: IAudioClient = device.Activate(CLSCTX_ALL, None).ok()?;
        let wave = client.GetMixFormat().ok()?;
        let format = format::format_of(wave);
        CoTaskMemFree(Some(wave as _));
        format
    }
}

/// The system's endpoint notifications; each callback copies what it gets and posts it.
#[implement(IMMNotificationClient)]
struct Notifications {
    messages: Sender<Message>,
}

impl Notifications_Impl {
    fn post(&self, message: Message) {
        let _ = self.messages.send(message);
    }
}

/// The id the system passes, None for a null pointer.
fn text(id: &PCWSTR) -> Option<String> {
    // SAFETY: a non-null id is a NUL-terminated string valid during the callback.
    (!id.is_null()).then(|| unsafe { id.to_string() }.ok()).flatten()
}

impl IMMNotificationClient_Impl for Notifications_Impl {
    fn OnDeviceStateChanged(&self, id: &PCWSTR, state: DEVICE_STATE) -> windows::core::Result<()> {
        if let Some(id) = text(id) {
            self.post(Message::State(id, state));
        }
        Ok(())
    }

    fn OnDeviceAdded(&self, id: &PCWSTR) -> windows::core::Result<()> {
        if let Some(id) = text(id) {
            self.post(Message::Added(id));
        }
        Ok(())
    }

    fn OnDeviceRemoved(&self, id: &PCWSTR) -> windows::core::Result<()> {
        if let Some(id) = text(id) {
            self.post(Message::Removed(id));
        }
        Ok(())
    }

    fn OnDefaultDeviceChanged(&self, flow: EDataFlow, role: ERole, id: &PCWSTR) -> windows::core::Result<()> {
        if flow == eRender && role == eConsole {
            self.post(Message::DefaultOutput(text(id)));
        }
        Ok(())
    }

    fn OnPropertyValueChanged(&self, _id: &PCWSTR, _key: &PROPERTYKEY) -> windows::core::Result<()> {
        Ok(())
    }
}
