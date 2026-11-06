//! The Slint bridge (research R3), ported from the stack spike: a Slint platform with one software
//! window whose pixels the video thread composites over the video, the forwarding of winit events
//! to Slint, and the UI image handed to the video thread when Slint reports a change.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use slint::ComponentHandle;
use slint::platform::software_renderer::{MinimalSoftwareWindow, PremultipliedRgbaColor, RepaintBufferType};
use slint::platform::{PointerEventButton, WindowAdapter, WindowEvent as SlintEvent};
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent};
use winit::keyboard::{Key, NamedKey};

use crate::shadows;

slint::include_modules!();

/// A finished UI image: premultiplied RGBA, rows packed, the size of the surface it was drawn for.
pub struct UiImage {
    pub pixels: Arc<[u8]>,
    pub width: u32,
    pub height: u32,
    /// Draw and copy time on the main thread, for the stats.
    pub draw_ms: f64,
}

/// Slint platform: one software-rendered window whose pixels the app composites itself.
struct Platform {
    window: Rc<MinimalSoftwareWindow>,
    start: Instant,
}

impl slint::platform::Platform for Platform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
        Ok(self.window.clone())
    }

    fn duration_since_start(&self) -> Duration {
        self.start.elapsed()
    }
}

pub struct Ui {
    window: Rc<MinimalSoftwareWindow>,
    pub app: AppWindow,
    buffer: Vec<PremultipliedRgbaColor>,
    size: PhysicalSize<u32>,
    scale: f32,
    cursor: slint::LogicalPosition,
    /// The next draw repaints the whole buffer: Slint repaints only what changed in logical
    /// pixels, but after a resize the buffer is new and after a scale change every physical pixel
    /// moved (moving between monitors changes both, the logical size staying the same).
    full_repaint: bool,
}

impl Ui {
    /// Installs the Slint platform, creates the window's component and selects `language` (a
    /// code from `i18n::LANGUAGES`). Must run once, on the main thread, before any other Slint
    /// call.
    pub fn new(language: &str) -> anyhow::Result<Self> {
        let window = MinimalSoftwareWindow::new(RepaintBufferType::ReusedBuffer);
        slint::platform::set_platform(Box::new(Platform {
            window: window.clone(),
            start: Instant::now(),
        }))
        .map_err(|e| anyhow::anyhow!("Slint platform: {e:?}"))?;
        let app = AppWindow::new()?;
        // Slint picks a catalog from the system's locale when the component is created; the
        // app's own rule (setting, `--lang`, system) replaces that choice.
        select_language(language);
        // Surface images by physical size, scale, fill and shadow strength; a few surfaces, so
        // the cache stays small, and it is dropped when it grows (tooltips vary with their text).
        let cache = RefCell::new(HashMap::<(u32, u32, u32, u32, u32), slint::Image>::new());
        app.global::<Theme>()
            .on_surface(move |width, height, scale, fill, strength| {
                let (w, h) = shadows::image_size(width, height, scale);
                let fill = slint::RgbaColor::<u8>::from(fill);
                let fill = slint::Rgba8Pixel::new(fill.red, fill.green, fill.blue, fill.alpha);
                let key = (
                    w,
                    h,
                    (scale * 100.0).round() as u32,
                    u32::from_be_bytes([fill.r, fill.g, fill.b, fill.a]),
                    strength.to_bits(),
                );
                let mut cache = cache.borrow_mut();
                if cache.len() > 64 {
                    cache.clear();
                }
                cache
                    .entry(key)
                    .or_insert_with(|| slint::Image::from_rgba8(shadows::make(width, height, scale, fill, strength)))
                    .clone()
            });
        // The state message's actions (FR-033); the `no-source` state has none in this spec.
        app.on_state_action(|id| log::warn!("state message: unknown action {id:?}"));
        app.show()?;
        Ok(Self {
            window,
            app,
            buffer: Vec::new(),
            size: PhysicalSize::new(0, 0),
            scale: 1.0,
            cursor: slint::LogicalPosition::new(0.0, 0.0),
            full_repaint: true,
        })
    }

    pub fn set_size(&mut self, size: PhysicalSize<u32>) {
        if size != self.size {
            self.full_repaint = true;
        }
        self.size = size;
        self.window.set_size(slint::PhysicalSize::new(size.width, size.height));
    }

    pub fn size(&self) -> PhysicalSize<u32> {
        self.size
    }

    pub fn set_scale(&mut self, scale: f32) {
        if scale != self.scale {
            self.full_repaint = true;
        }
        self.scale = scale;
        self.window
            .window()
            .dispatch_event(SlintEvent::ScaleFactorChanged { scale_factor: scale });
    }

    /// Forwards a winit event to Slint. Returns true when the UI may have changed.
    pub fn window_event(&mut self, event: &WindowEvent) -> bool {
        let slint_event = match event {
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor =
                    slint::LogicalPosition::new(position.x as f32 / self.scale, position.y as f32 / self.scale);
                SlintEvent::PointerMoved { position: self.cursor }
            }
            WindowEvent::CursorLeft { .. } => SlintEvent::PointerExited,
            WindowEvent::MouseInput { state, button, .. } => {
                let button = match button {
                    MouseButton::Left => PointerEventButton::Left,
                    MouseButton::Right => PointerEventButton::Right,
                    MouseButton::Middle => PointerEventButton::Middle,
                    _ => PointerEventButton::Other,
                };
                let position = self.cursor;
                // The focus ring shows only while the last input was the keyboard (FR-020).
                if *state == ElementState::Pressed {
                    self.app.global::<Input>().set_keyboard(false);
                }
                match state {
                    ElementState::Pressed => SlintEvent::PointerPressed { position, button },
                    ElementState::Released => SlintEvent::PointerReleased { position, button },
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                // Lines as 60 logical pixels, as Slint's own winit backend does.
                let (delta_x, delta_y) = match delta {
                    MouseScrollDelta::LineDelta(x, y) => (x * 60.0, y * 60.0),
                    MouseScrollDelta::PixelDelta(p) => (p.x as f32 / self.scale, p.y as f32 / self.scale),
                };
                SlintEvent::PointerScrolled {
                    position: self.cursor,
                    delta_x,
                    delta_y,
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let Some(text) = slint_key(event) else { return false };
                let modifier = matches!(
                    event.logical_key,
                    Key::Named(NamedKey::Shift | NamedKey::Control | NamedKey::Alt | NamedKey::Super)
                );
                if event.state == ElementState::Pressed && !modifier {
                    self.app.global::<Input>().set_keyboard(true);
                    // Slint's focus chain skips controls outside the window, as the hidden docks:
                    // show the chrome at once, before the key moves the focus.
                    if event.logical_key == Key::Named(NamedKey::Tab) {
                        self.app.invoke_reveal_chrome();
                    }
                }
                match event.state {
                    ElementState::Pressed => SlintEvent::KeyPressed { text },
                    ElementState::Released => SlintEvent::KeyReleased { text },
                }
            }
            WindowEvent::Focused(active) => {
                self.app.global::<WindowState>().set_active(*active);
                SlintEvent::WindowActiveChanged(*active)
            }
            _ => return false,
        };
        self.window.window().dispatch_event(slint_event);
        true
    }

    /// The window was destroyed (hidden in the tray on Wayland): no pointer, no focus, and the
    /// next image is drawn whole for the new window's video thread.
    pub fn window_gone(&mut self) {
        let window = self.window.window();
        window.dispatch_event(SlintEvent::PointerExited);
        self.app.global::<WindowState>().set_active(false);
        window.dispatch_event(SlintEvent::WindowActiveChanged(false));
        self.full_repaint = true;
    }

    /// Advances Slint's timers and animations.
    pub fn update_timers(&self) {
        slint::platform::update_timers_and_animations();
    }

    pub fn has_active_animations(&self) -> bool {
        self.window.window().has_active_animations()
    }

    /// When Slint's next timer is due, for `ControlFlow::WaitUntil`.
    pub fn next_timer(&self) -> Option<Instant> {
        slint::platform::duration_until_next_timer_update().map(|wait| Instant::now() + wait)
    }

    /// Draws the UI if Slint reports a change and returns the image for the video thread.
    pub fn draw_if_needed(&mut self) -> Option<UiImage> {
        let (w, h) = (self.size.width.max(1), self.size.height.max(1));
        if self.buffer.len() != (w * h) as usize {
            self.buffer = vec![PremultipliedRgbaColor::default(); (w * h) as usize];
        }
        let started = Instant::now();
        let buffer = &mut self.buffer;
        let full = std::mem::take(&mut self.full_repaint);
        if full {
            self.window.request_redraw();
        }
        let drawn = self.window.draw_if_needed(|renderer| {
            if full {
                // A new buffer is repainted whole; going back to a reused buffer clears Slint's
                // cache, so the next draw is whole too (one frame, after a resize or scale change).
                renderer.set_repaint_buffer_type(RepaintBufferType::NewBuffer);
                renderer.render(buffer, w as usize);
                renderer.set_repaint_buffer_type(RepaintBufferType::ReusedBuffer);
            } else {
                renderer.render(buffer, w as usize);
            }
        });
        drawn.then(|| UiImage {
            pixels: Arc::from(bytemuck::cast_slice::<_, u8>(&self.buffer)),
            width: w,
            height: h,
            draw_ms: started.elapsed().as_secs_f64() * 1e3,
        })
    }

    /// Where the UI wants the video, in physical pixels: x, y, width, height, corner radius.
    pub fn video_area(&self) -> [f32; 5] {
        let a = &self.app;
        [
            a.get_video_x(),
            a.get_video_y(),
            a.get_video_width(),
            a.get_video_height(),
            a.get_video_radius(),
        ]
        .map(|v| v * self.scale)
    }

    /// The theme's deepest tone, the ground around the video: RGB from 0 to 1.
    pub fn backdrop(&self) -> [f64; 3] {
        let deep = self.app.global::<Theme>().get_c().deep;
        [deep.red(), deep.green(), deep.blue()].map(|v| f64::from(v) / 255.0)
    }
}

/// Selects the bundled catalog for `language` (FR-022); every `@tr` text, `Texts` included, is
/// evaluated again on its next read. Needs the component to exist.
pub fn select_language(  language: &str) {
    if let Err(err) = slint::select_bundled_translation(language) {
        log::warn!("language {language}: {err}");
    }
}

/// Slint's key text for a winit key, if Slint knows it.
fn slint_key(event: &KeyEvent) -> Option<slint::SharedString> {
    use slint::platform::Key as K;
    let key = match &event.logical_key {
        Key::Character(text) => return Some(text.as_str().into()),
        Key::Named(NamedKey::Escape) => K::Escape,
        Key::Named(NamedKey::Enter) => K::Return,
        Key::Named(NamedKey::Tab) => K::Tab,
        Key::Named(NamedKey::Space) => K::Space,
        Key::Named(NamedKey::Backspace) => K::Backspace,
        Key::Named(NamedKey::Delete) => K::Delete,
        Key::Named(NamedKey::ArrowLeft) => K::LeftArrow,
        Key::Named(NamedKey::ArrowRight) => K::RightArrow,
        Key::Named(NamedKey::ArrowUp) => K::UpArrow,
        Key::Named(NamedKey::ArrowDown) => K::DownArrow,
        Key::Named(NamedKey::Home) => K::Home,
        Key::Named(NamedKey::End) => K::End,
        Key::Named(NamedKey::PageUp) => K::PageUp,
        Key::Named(NamedKey::PageDown) => K::PageDown,
        Key::Named(NamedKey::Shift) => K::Shift,
        Key::Named(NamedKey::Control) => K::Control,
        Key::Named(NamedKey::Alt) => K::Alt,
        Key::Named(NamedKey::Super) => K::Meta,
        Key::Named(NamedKey::F11) => K::F11,
        _ => return None,
    };
    Some(key.into())
}
