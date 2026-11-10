//! XDG activation on Wayland (research R15, T077): `xdg_activation_v1.activate(token, surface)`
//! on the window's own `wl_surface`, sent on a queue of our own over winit's display.

use wayland_backend::client::{Backend, ObjectId};
use wayland_client::globals::{GlobalListContents, registry_queue_init};
use wayland_client::protocol::wl_registry::WlRegistry;
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle};
use wayland_protocols::xdg::activation::v1::client::xdg_activation_v1::XdgActivationV1;
use winit::raw_window_handle::{HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle};
use winit::window::Window;

/// Activates `window` with `token`. False when the window is not a Wayland one (X11 raises with
/// `focus_window`).
pub fn activate(window: &Window, token: &str) -> Result<bool, String> {
    let display = window.display_handle().map_err(|e| e.to_string())?;
    let handle = window.window_handle().map_err(|e| e.to_string())?;
    let (RawDisplayHandle::Wayland(display), RawWindowHandle::Wayland(handle)) = (display.as_raw(), handle.as_raw())
    else {
        return Ok(false);
    };
    // SAFETY: the display and the surface belong to winit's live window, borrowed for this
    // call; the backend made from the display does not close it when dropped.
    let backend = unsafe { Backend::from_foreign_display(display.display.as_ptr().cast()) };
    let connection = Connection::from_backend(backend);
    let (globals, mut queue) = registry_queue_init::<State>(&connection).map_err(|e| e.to_string())?;
    let activation: XdgActivationV1 = globals
        .bind(&queue.handle(), 1..=1, ())
        .map_err(|e| format!("no xdg_activation_v1: {e}"))?;
    // SAFETY: as above, the pointer is winit's `wl_surface` proxy.
    let id = unsafe { ObjectId::from_ptr(WlSurface::interface(), handle.surface.as_ptr().cast()) }
        .map_err(|e| e.to_string())?;
    let surface = WlSurface::from_id(&connection, id).map_err(|e| e.to_string())?;
    activation.activate(token.to_string(), &surface);
    activation.destroy();
    queue.roundtrip(&mut State).map_err(|e| e.to_string())?;
    Ok(true)
}

/// Nothing to keep: the registry and the activation global send no event this needs.
struct State;

impl Dispatch<WlRegistry, GlobalListContents> for State {
    fn event(
        _: &mut Self,
        _: &WlRegistry,
        _: <WlRegistry as Proxy>::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<XdgActivationV1, ()> for State {
    fn event(
        _: &mut Self,
        _: &XdgActivationV1,
        _: <XdgActivationV1 as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
