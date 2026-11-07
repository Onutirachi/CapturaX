//! NVIDIA's PowerMizer request through NVML, loaded at run time so the app starts on machines
//! without the NVIDIA driver (research R7). Since driver 590 a normal user may set the mode, and
//! it holds only while the requesting process lives.

use std::ffi::{CStr, CString, c_char, c_int, c_uint, c_void};
use std::sync::atomic::{AtomicBool, Ordering};

use libloading::Library;

const LIBRARY: &str = "libnvidia-ml.so.1";

type Device = *mut c_void;
type Return = c_int;

const SUCCESS: Return = 0;

/// `nvmlPowerMizerMode_t` values (nvml.h).
const ADAPTIVE: c_uint = 0;
const PREFER_MAXIMUM_PERFORMANCE: c_uint = 1;
const AUTO: c_uint = 2;
const PREFER_CONSISTENT_PERFORMANCE: c_uint = 3;

/// `nvmlDevicePowerMizerModes_v1_t` (nvml.h).
#[repr(C)]
#[derive(Default)]
struct PowerMizerModes {
    /// Out: the mode in use.
    current_mode: c_uint,
    /// In: the mode to set.
    mode: c_uint,
    /// Out: bitmask of the supported modes.
    supported: c_uint,
}

/// Whether a failure was already logged this run: a machine without the driver would otherwise
/// log it each time the preview starts.
static LOGGED: AtomicBool = AtomicBool::new(false);

/// Holds the request until it drops; None when nothing was requested.
pub struct PerformanceGuard(#[allow(dead_code, reason = "kept for its Drop")] Option<Held>);

impl PerformanceGuard {
    pub fn new(pci_bus_id: Option<&str>) -> Self {
        match request(pci_bus_id) {
            Ok(held) => Self(held),
            Err(failure) => {
                let first = !LOGGED.swap(true, Ordering::Relaxed);
                match (&failure, first) {
                    (Failure::Missing(reason), true) => {
                        log::info!("gpu performance: no NVIDIA control ({reason})")
                    }
                    (Failure::Refused(reason), true) => {
                        log::warn!("gpu performance: request refused ({reason})")
                    }
                    (_, false) => log::debug!("gpu performance: not requested ({failure})"),
                }
                Self(None)
            }
        }
    }
}

enum Failure {
    /// No NVIDIA driver, or one too old for the call.
    Missing(String),
    /// The driver answered with an error.
    Refused(String),
}

impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Missing(reason) | Self::Refused(reason) => f.write_str(reason),
        }
    }
}

/// The NVML entry points the guard uses; valid while `_library` is loaded.
struct Api {
    init: unsafe extern "C" fn() -> Return,
    shutdown: unsafe extern "C" fn() -> Return,
    error_string: unsafe extern "C" fn(Return) -> *const c_char,
    handle_by_pci_bus_id: unsafe extern "C" fn(*const c_char, *mut Device) -> Return,
    handle_by_index: unsafe extern "C" fn(c_uint, *mut Device) -> Return,
    get_mode: unsafe extern "C" fn(Device, *mut PowerMizerModes) -> Return,
    set_mode: unsafe extern "C" fn(Device, *mut PowerMizerModes) -> Return,
    // Last, so the entry points above are never used after the library unloads.
    _library: Library,
}

impl Api {
    fn load() -> Result<Self, Failure> {
        // SAFETY: loading the driver's NVML only runs its own initializers, which ask nothing of
        // the caller.
        let library = unsafe { Library::new(LIBRARY) }.map_err(|err| Failure::Missing(err.to_string()))?;
        macro_rules! symbol {
            ($name:literal) => {
                // SAFETY: the type matches the function's declaration in nvml.h, and the pointer
                // is kept in `Api` next to the library that owns it.
                *unsafe { library.get(concat!($name, "\0").as_bytes()) }
                    .map_err(|err| Failure::Missing(format!("{}: {err}", $name)))?
            };
        }
        Ok(Self {
            init: symbol!("nvmlInit_v2"),
            shutdown: symbol!("nvmlShutdown"),
            error_string: symbol!("nvmlErrorString"),
            handle_by_pci_bus_id: symbol!("nvmlDeviceGetHandleByPciBusId_v2"),
            handle_by_index: symbol!("nvmlDeviceGetHandleByIndex_v2"),
            get_mode: symbol!("nvmlDeviceGetPowerMizerMode_v1"),
            set_mode: symbol!("nvmlDeviceSetPowerMizerMode_v1"),
            _library: library,
        })
    }

    /// `Ok` on success, else the call's name and NVML's text for the code.
    fn check(&self, call: &str, code: Return) -> Result<(), Failure> {
        if code == SUCCESS {
            return Ok(());
        }
        // SAFETY: nvmlErrorString takes any code and returns a static, NUL-terminated string.
        let text = unsafe { (self.error_string)(code) };
        let text = if text.is_null() {
            format!("error {code}")
        } else {
            // SAFETY: not null, and NVML's strings live as long as the library.
            unsafe { CStr::from_ptr(text) }.to_string_lossy().into_owned()
        };
        Err(Failure::Refused(format!("{call}: {text}")))
    }
}

/// NVML initialized; shut down on drop (NVML counts its inits).
struct Session(Api);

impl Drop for Session {
    fn drop(&mut self) {
        // SAFETY: matches the successful nvmlInit_v2 that created this session.
        let code = unsafe { (self.0.shutdown)() };
        if let Err(failure) = self.0.check("nvmlShutdown", code) {
            log::debug!("gpu performance: {failure}");
        }
    }
}

/// The request in place; the previous mode is set back on drop.
struct Held {
    // Drops after `Held::drop` restored the mode.
    session: Session,
    device: Device,
    previous: c_uint,
}

// SAFETY: NVML is thread-safe and its device handles may be used from any thread.
unsafe impl Send for Held {}

impl Drop for Held {
    fn drop(&mut self) {
        let api = &self.session.0;
        // The driver keeps one request per process: this withdraws ours and leaves other
        // processes' requests (nvidia-settings, another app) as they are (research R7).
        let mut modes = PowerMizerModes {
            mode: self.previous,
            ..Default::default()
        };
        // SAFETY: `device` came from this NVML session, which is still initialized, and `modes`
        // outlives the call.
        let code = unsafe { (api.set_mode)(self.device, &raw mut modes) };
        match api.check("nvmlDeviceSetPowerMizerMode_v1", code) {
            Ok(()) => log::info!("gpu performance: PowerMizer back to {}", mode_name(self.previous)),
            Err(failure) => log::warn!(
                "gpu performance: cannot restore {} ({failure})",
                mode_name(self.previous)
            ),
        }
    }
}

/// Sets PowerMizer to prefer maximum performance. `Ok(None)` when it already does (left as it
/// is, nothing to restore).
fn request(pci_bus_id: Option<&str>) -> Result<Option<Held>, Failure> {
    let api = Api::load()?;
    // SAFETY: no precondition; balanced by `Session::drop`.
    let code = unsafe { (api.init)() };
    api.check("nvmlInit_v2", code)?;
    let session = Session(api);
    let api = &session.0;

    let (device, name) = find_device(api, pci_bus_id)?;
    let mut modes = PowerMizerModes::default();
    // SAFETY: `device` is a handle of this session, `modes` outlives the call.
    let code = unsafe { (api.get_mode)(device, &raw mut modes) };
    api.check("nvmlDeviceGetPowerMizerMode_v1", code)?;
    let previous = modes.current_mode;
    if previous == PREFER_MAXIMUM_PERFORMANCE {
        log::info!("gpu performance: PowerMizer already prefers maximum performance");
        return Ok(None);
    }

    modes.mode = PREFER_MAXIMUM_PERFORMANCE;
    // SAFETY: as above.
    let code = unsafe { (api.set_mode)(device, &raw mut modes) };
    api.check("nvmlDeviceSetPowerMizerMode_v1", code)?;
    log::info!(
        "gpu performance: PowerMizer prefers maximum performance on {} (was {})",
        name,
        mode_name(previous)
    );
    Ok(Some(Held {
        session,
        device,
        previous,
    }))
}

/// The GPU at `pci_bus_id`, else the first one NVML lists, with how the log names it.
fn find_device(api: &Api, pci_bus_id: Option<&str>) -> Result<(Device, String), Failure> {
    let mut device: Device = std::ptr::null_mut();
    if let Some(id) = pci_bus_id {
        match CString::new(id) {
            Ok(text) => {
                // SAFETY: `text` is NUL-terminated and outlives the call; `device` is writable.
                let code = unsafe { (api.handle_by_pci_bus_id)(text.as_ptr(), &raw mut device) };
                match api.check("nvmlDeviceGetHandleByPciBusId_v2", code) {
                    Ok(()) => return Ok((device, id.to_owned())),
                    Err(failure) => log::debug!("gpu performance: {id} not found ({failure}), using the first GPU"),
                }
            }
            Err(_) => log::debug!("gpu performance: bad PCI bus id {id:?}, using the first GPU"),
        }
    }
    // SAFETY: `device` is writable for the call.
    let code = unsafe { (api.handle_by_index)(0, &raw mut device) };
    api.check("nvmlDeviceGetHandleByIndex_v2", code)?;
    Ok((device, "the first GPU".to_owned()))
}

fn mode_name(mode: c_uint) -> String {
    match mode {
        ADAPTIVE => "Adaptive".into(),
        PREFER_MAXIMUM_PERFORMANCE => "Prefer maximum performance".into(),
        AUTO => "Auto".into(),
        PREFER_CONSISTENT_PERFORMANCE => "Prefer consistent performance".into(),
        other => format!("mode {other}"),
    }
}
