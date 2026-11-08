//! The device monitor (research R1), ported from the stack spike's `capture/src/devices.rs`: one
//! `gst::DeviceMonitor` on `Video/Source` for the app's lifetime, its add and remove messages
//! read on a thread of its own.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use gst::prelude::*;

use crate::Unavailable;
use crate::device::{self, Device};
use crate::mode;

/// What the monitor reports.
#[derive(Debug, Clone)]
pub enum MonitorEvent {
    /// The devices present at start; sent once, before any other event.
    Listed(Vec<Device>),
    Added(Device),
    /// The id of a device that left.
    Removed(String),
}

/// The running device monitor; stops on [`Monitor::stop`] or drop.
pub struct Monitor {
    monitor: gst::DeviceMonitor,
    known: Arc<Mutex<Vec<Device>>>,
    stopping: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Monitor {
    /// Starts the monitor; `on_event` runs on the monitor's thread.
    pub fn start(on_event: impl Fn(MonitorEvent) + Send + 'static) -> Result<Self, Unavailable> {
        gst::init().map_err(|e| Unavailable(format!("GStreamer: {e}")))?;
        let monitor = gst::DeviceMonitor::new();
        monitor.add_filter(Some("Video/Source"), None);
        // Providers hidden by another one (V4L2 behind PipeWire) are listed too.
        monitor.set_show_all_devices(true);
        monitor
            .start()
            .map_err(|e| Unavailable(format!("device monitor: {e}")))?;
        let known = Arc::new(Mutex::new(Vec::new()));
        let stopping = Arc::new(AtomicBool::new(false));
        let thread = {
            let monitor = monitor.clone();
            let known = Arc::clone(&known);
            let stopping = Arc::clone(&stopping);
            std::thread::Builder::new()
                .name("device-monitor".into())
                .spawn(move || run(&monitor, &known, &stopping, &on_event))
                .map_err(|e| Unavailable(format!("device monitor thread: {e}")))?
        };
        Ok(Self {
            monitor,
            known,
            stopping,
            thread: Some(thread),
        })
    }

    /// The devices listed now, read again from the providers (the 3 s re-read while waiting,
    /// research R1).
    pub fn devices(&self) -> Vec<Device> {
        let devices = described(self.monitor.devices());
        if let Ok(mut known) = self.known.lock() {
            known.clone_from(&devices);
        }
        devices
    }

    /// Stops the thread and the monitor; idempotent.
    pub fn stop(&mut self) {
        self.stopping.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            if thread.join().is_err() {
                log::error!("device monitor thread panicked");
            }
            self.monitor.stop();
            log::info!("device monitor stopped");
        }
    }
}

impl Drop for Monitor {
    fn drop(&mut self) {
        self.stop();
    }
}

fn run(
    monitor: &gst::DeviceMonitor,
    known: &Mutex<Vec<Device>>,
    stopping: &AtomicBool,
    on_event: &dyn Fn(MonitorEvent),
) {
    let listed = described(monitor.devices());
    for device in &listed {
        log::info!("device listed: {} ({})", device.name, device.id);
    }
    if let Ok(mut known) = known.lock() {
        known.clone_from(&listed);
    }
    on_event(MonitorEvent::Listed(listed));

    let bus = monitor.bus();
    while !stopping.load(Ordering::SeqCst) {
        let Some(message) = bus.timed_pop(gst::ClockTime::from_mseconds(200)) else {
            continue;
        };
        match message.view() {
            gst::MessageView::DeviceAdded(added) => {
                let Some(device) = describe(&added.device()) else {
                    continue;
                };
                let Ok(mut known) = known.lock() else { continue };
                if device::find(&device.id, &known).is_some() {
                    // Windows re-probes every device on each arrival.
                    log::debug!("device added again, ignored: {}", device.id);
                    continue;
                }
                known.push(device);
                device::numbered(&mut known);
                let device = known.last().cloned().expect("just pushed");
                drop(known);
                log::info!("device added: {} ({})", device.name, device.id);
                on_event(MonitorEvent::Added(device));
            }
            gst::MessageView::DeviceRemoved(removed) => {
                let gone = removed.device();
                if !device::is_preferred(&gone) {
                    continue;
                }
                let Some(id) = device::identity(&device::properties(&gone)) else {
                    continue;
                };
                let Ok(mut known) = known.lock() else { continue };
                let before = known.len();
                known.retain(|d| d.id != id);
                if known.len() == before {
                    continue;
                }
                device::numbered(&mut known);
                drop(known);
                log::info!("device removed: {id}");
                on_event(MonitorEvent::Removed(id));
            }
            _ => {}
        }
    }
}

/// The devices the providers list now, from a probe of their own: a started monitor may still
/// list a device that has left (research R1, R6).
pub(crate) fn listed_now() -> Vec<Device> {
    let monitor = gst::DeviceMonitor::new();
    monitor.add_filter(Some("Video/Source"), None);
    monitor.set_show_all_devices(true);
    described(monitor.devices())
}

/// The preferred devices among those listed, numbered.
fn described(devices: impl IntoIterator<Item = gst::Device>) -> Vec<Device> {
    let mut found: Vec<Device> = Vec::new();
    for gst_device in devices {
        if let Some(device) = describe(&gst_device)
            && device::find(&device.id, &found).is_none()
        {
            found.push(device);
        }
    }
    device::numbered(&mut found);
    found
}

/// A device description, or `None` when it is not from the preferred provider or has no stable
/// identity.
fn describe(gst_device: &gst::Device) -> Option<Device> {
    if !device::is_preferred(gst_device) {
        return None;
    }
    let props = device::properties(gst_device);
    let name = gst_device.display_name().to_string();
    let Some(id) = device::identity(&props) else {
        log::warn!("device without a stable identity, not listed: {name}");
        return None;
    };
    let api = props.iter().find(|(k, _)| k == "device.api").map(|(_, v)| v.as_str());
    let modes = gst_device
        .caps()
        .map(|caps| {
            let frame_server = api == Some("mediafoundation") && mode::nv12_declared_full(&caps);
            mode::modes_from_caps(&caps, frame_server)
        })
        .unwrap_or_default();
    Some(Device {
        id,
        name: name.clone(),
        base_name: name,
        modes,
        signal: None,
        gst: Some(gst_device.clone()),
    })
}
