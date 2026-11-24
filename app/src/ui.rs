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
use winit::event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta, Touch, TouchPhase, WindowEvent};
use winit::keyboard::{Key, NamedKey};

use crate::shadows;

slint::include_modules!();

/// What the video area shows for a preview state (data-model.md "Preview state", research R13):
/// the state id of `main.slint`, the title and the detail. `name` is the state's device name;
/// `windows` picks Blocked's texts (the privacy switch on Windows, the device node on Linux).
pub fn state_message(
    state: &crate::preview::PreviewState,
    name: &str,
    windows: bool,
    texts: &Texts,
) -> (&'static str, String, String) {
    use crate::preview::PreviewState as S;
    let name = slint::SharedString::from(name);
    let (id, title, detail) = match state {
        S::Playing => ("playing", Default::default(), Default::default()),
        S::NoSource => ("no-source", Default::default(), Default::default()),
        // Its title is main.slint's, as the shell's `no-source` (preview extras FR-008).
        S::Hidden => ("hidden", Default::default(), Default::default()),
        S::NoDevices => (
            "no-devices",
            texts.get_preview_no_devices(),
            texts.get_preview_no_devices_detail(),
        ),
        // No text: the video area fades, then shows the progress ring (research R17).
        S::Opening(_) => ("opening", Default::default(), Default::default()),
        S::Disconnected(_) => (
            "disconnected",
            texts.invoke_preview_disconnected(name),
            texts.get_preview_disconnected_detail(),
        ),
        S::Blocked { .. } if windows => (
            "blocked",
            texts.get_preview_blocked(),
            texts.get_preview_blocked_detail(),
        ),
        // Linux: the node that could not be opened, else the device's name.
        S::Blocked { node, .. } => (
            "blocked",
            texts.invoke_preview_cannot_open(node.as_deref().map_or(name, Into::into)),
            texts.get_preview_no_permission(),
        ),
        S::InUse(_) => (
            "in-use",
            texts.get_preview_in_use(),
            texts.invoke_preview_in_use_detail(name),
        ),
        S::NoSignal(_) => (
            "no-signal",
            texts.get_preview_no_signal(),
            texts.get_preview_no_signal_detail(),
        ),
        S::Failed(_) => ("failed", texts.invoke_preview_cannot_open(name), Default::default()),
    };
    (id, title.into(), detail.into())
}

/// A row of the device or mode menu as Slint shows it, its named texts read from the catalog.
pub fn menu_item(item: &crate::preview::Item, texts: &Texts) -> MenuItem {
    use crate::preview::Label;
    let text = match &item.text {
        Label::Plain(text) => text.as_str().into(),
        Label::NoDevices => texts.get_preview_no_devices(),
        Label::Automatic => texts.get_modes_automatic(),
        Label::Modes => texts.get_modes_native(),
        Label::Auto(choice) => auto_name(choice, texts),
    };
    let detail = item
        .detail
        .as_ref()
        .map_or_else(Default::default, |d| match &d.automatic {
            Some(choice) => texts.invoke_mode_auto_detail(auto_name(choice, texts), d.mode.as_str().into()),
            None => d.mode.as_str().into(),
        });
    let (tag, tag_kind, aspect) = item.tags.map_or_else(Default::default, |t| {
        let aspect = t.aspect.to_string().into();
        match t.name {
            Some(name) => (
                name.into(),
                t.family.map_or_else(|| "custom".into(), |f| f.to_string().into()),
                aspect,
            ),
            None => (texts.get_custom_resolution(), "custom".into(), aspect),
        }
    });
    MenuItem {
        text,
        kind: if item.text.is_header() {
            "header".into()
        } else {
            Default::default()
        },
        detail,
        rate: item
            .rate
            .as_ref()
            .map_or_else(Default::default, |r| texts.invoke_rate(r.as_str().into())),
        tag,
        tag_kind,
        aspect,
        disabled: item.disabled,
        ..Default::default()
    }
}

/// The rows of the Audio menu as Slint shows them; the choices in use are checked (the menu marks
/// an input and an output). The state's notice is a `state` row (its title, the device's name, its
/// detail), its action an `action` row drawn inside the notice's card; both carry the notice's
/// tone (FR-017).
pub fn audio_items(items: &[crate::audio::Item], texts: &Texts) -> Vec<MenuItem> {
    use crate::audio::Label;
    let mut tone = "";
    items
        .iter()
        .map(|item| {
            let (kind, name, detail) = match &item.text {
                Label::Notice(notice) => {
                    tone = audio_notice_tone(notice);
                    (
                        "state",
                        audio_notice_name(notice, texts),
                        audio_notice_detail(notice, texts),
                    )
                }
                Label::Action(_) => ("action", Default::default(), Default::default()),
                label if label.is_header() => ("header", Default::default(), Default::default()),
                _ if item.missing => ("", Default::default(), texts.get_audio_tag_disconnected()),
                _ => ("", Default::default(), Default::default()),
            };
            MenuItem {
                text: audio_label(&item.text, texts),
                kind: kind.into(),
                name,
                detail,
                tone: if kind == "state" || kind == "action" { tone } else { "" }.into(),
                checked: item.current,
                tag: if item.exclusive {
                    texts.get_audio_tag_exclusive()
                } else {
                    Default::default()
                },
                tag_kind: if item.exclusive { "accent" } else { "" }.into(),
                ..Default::default()
            }
        })
        .collect()
}

/// The tone of the Audio menu's notice card: the tag's (`warn`, `bad`), none for No paired audio.
fn audio_notice_tone(notice: &crate::audio::Notice) -> &'static str {
    use crate::audio::Notice;
    match notice {
        Notice::NoPair => "",
        Notice::Disconnected(_) | Notice::InUse(_) | Notice::Held(_) | Notice::Fallback(_) => "warn",
        Notice::Blocked | Notice::Failed(_) => "bad",
    }
}

/// The device line of the Audio menu's notice (FR-022): the device at fault, none when no one
/// device is.
fn audio_notice_name(notice: &crate::audio::Notice, texts: &Texts) -> slint::SharedString {
    use crate::audio::Notice;
    match notice {
        Notice::NoPair | Notice::Blocked => Default::default(),
        Notice::Failed(name) if name.is_empty() => texts.get_audio_system_default_unnamed(),
        Notice::Disconnected(name)
        | Notice::InUse(name)
        | Notice::Held(name)
        | Notice::Failed(name)
        | Notice::Fallback(name) => name.as_str().into(),
    }
}

/// The detail line of the Audio menu's notice (FR-022); the actions are rows of their own.
fn audio_notice_detail(notice: &crate::audio::Notice, texts: &Texts) -> slint::SharedString {
    use crate::audio::Notice;
    match notice {
        Notice::NoPair => texts.get_audio_choose_input(),
        Notice::Disconnected(_) => texts.get_preview_disconnected_detail(),
        Notice::Blocked => texts.get_preview_blocked_detail(),
        Notice::InUse(_) => texts.get_audio_exclusive_unavailable(),
        Notice::Held(_) | Notice::Failed(_) => Default::default(),
        Notice::Fallback(_) => texts.get_audio_using_default(),
    }
}

/// The tag before the Audio select's value and its kind (tag.slint): `warn` or `bad` for a
/// state, `rate` (outlined) for Exclusive, which is no problem.
pub fn audio_tag(tag: Option<crate::audio::StateTag>, texts: &Texts) -> (slint::SharedString, &'static str) {
    use crate::audio::StateTag;
    match tag {
        None => (Default::default(), ""),
        Some(StateTag::Disconnected) => (texts.get_audio_tag_disconnected(), "warn"),
        Some(StateTag::InUse) => (texts.get_preview_in_use(), "warn"),
        Some(StateTag::Blocked) => (texts.get_audio_tag_blocked(), "bad"),
        Some(StateTag::Failed) => (texts.get_audio_tag_failed(), "bad"),
    }
}

/// The rows of the settings panel's Audio category (preview audio FR-014).
pub fn endpoint_rows(rows: &[crate::audio::ExclusiveRow]) -> Vec<AudioEndpointRow> {
    rows.iter()
        .map(|row| AudioEndpointRow {
            id: row.id.as_str().into(),
            name: row.name.as_str().into(),
            exclusive: row.exclusive,
            in_use: row.in_use,
        })
        .collect()
}

/// Shows `rows` in `model`: a model of the same length is changed in place, row by row, so the
/// rows keep their instances (and the keyboard focus on a checkbox); else `set` gets a new one.
pub fn sync_rows<T: Clone + PartialEq + 'static>(
    model: &slint::ModelRc<T>,
    rows: Vec<T>,
    set: impl FnOnce(slint::ModelRc<T>),
) {
    use slint::Model;
    if let Some(vec) = model.as_any().downcast_ref::<slint::VecModel<T>>()
        && vec.row_count() == rows.len()
    {
        for (index, row) in rows.into_iter().enumerate() {
            if vec.row_data(index).as_ref() != Some(&row) {
                vec.set_row_data(index, row);
            }
        }
        return;
    }
    set(slint::ModelRc::new(slint::VecModel::from(rows)));
}

/// The text of an Audio menu label (also the Audio select's value) from the catalog.
pub fn audio_label(label: &crate::audio::Label, texts: &Texts) -> slint::SharedString {
    use crate::audio::{Action, Label};
    match label {
        Label::Inputs => texts.get_audio_inputs(),
        Label::Outputs => texts.get_audio_outputs(),
        Label::Auto(Some(name)) => texts.invoke_audio_auto(name.as_str().into()),
        Label::Auto(None) => texts.get_audio_auto_none(),
        Label::NoAudio => texts.get_audio_no_audio(),
        Label::SystemDefault(Some(name)) => texts.invoke_audio_system_default(name.as_str().into()),
        Label::SystemDefault(None) => texts.get_audio_system_default_unnamed(),
        Label::Plain(text) => text.as_str().into(),
        Label::Notice(notice) => {
            use crate::audio::Notice;
            match notice {
                Notice::NoPair => texts.get_audio_no_pair(),
                Notice::Disconnected(_) | Notice::Fallback(_) => texts.get_audio_device_disconnected(),
                Notice::Blocked => texts.get_preview_blocked(),
                Notice::InUse(_) | Notice::Held(_) => texts.get_audio_device_in_use(),
                Notice::Failed(_) => texts.get_audio_cannot_open(),
            }
        }
        Label::Action(Action::TryAgain) => texts.get_try_again(),
        Label::Action(Action::OpenPrivacy) => texts.get_open_privacy(),
    }
}

/// The name of an automatic mode choice; a manual one has none.
fn auto_name(choice: &capture::mode::ModeChoice, texts: &Texts) -> slint::SharedString {
    use capture::mode::ModeChoice as C;
    match choice {
        C::AutoSource => texts.get_mode_auto_source(),
        C::AutoMaxResolution => texts.get_mode_auto_max_resolution(),
        C::AutoMaxFps => texts.get_mode_auto_max_fps(),
        C::Manual { .. } => Default::default(),
    }
}

/// The kinds of menu rows, by height (menu.slint).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuRow {
    Header,
    Item,
    WithDetail,
    /// A state's notice with its name or detail line (a notice with neither is as tall as an item);
    /// as tall as its lines, measured by Slint (`Menus.notice-height`).
    State,
    Action,
}

impl MenuRow {
    fn height(self, notice: f32) -> f32 {
        match self {
            Self::Header => 30.0,
            Self::Item => 36.0,
            Self::WithDetail => 48.0,
            Self::State => notice,
            Self::Action => 42.0,
        }
    }

    /// The row of a menu item (menu.slint's `row` heights).
    fn of(item: &MenuItem) -> Self {
        match (item.kind.as_str(), item.detail.is_empty()) {
            ("header", _) => Self::Header,
            ("action", _) => Self::Action,
            ("state", true) if !item.name.is_empty() => Self::State,
            (_, true) => Self::Item,
            ("state", false) => Self::State,
            (_, false) => Self::WithDetail,
        }
    }
}

/// The space between menu rows (menu.slint).
const MENU_SPACING: f32 = 2.0;

/// The top of row `index` in a menu's list, in logical pixels; for `index` = the count, the
/// list's height plus one spacing. Out of range clamps. `notice`: the height of a state row.
pub fn item_top(rows: &[MenuRow], index: i32, notice: f32) -> f32 {
    let end = usize::try_from(index.max(0)).unwrap_or(0).min(rows.len());
    rows[..end].iter().map(|r| r.height(notice) + MENU_SPACING).sum()
}

/// The next selectable item of a menu from `from` (excluded) in direction `step` (1 or -1), or
/// `from` when there is none (the highlight stays). `from` may be -1 or `len` to start at an end.
pub fn next_selectable(selectable: &[bool], from: i32, step: i32) -> i32 {
    let step = step.signum();
    if step == 0 {
        return from;
    }
    let mut i = from + step;
    while i >= 0 && (i as usize) < selectable.len() {
        if selectable[i as usize] {
            return i;
        }
        i += step;
    }
    from
}

/// A finished UI image: premultiplied RGBA, rows packed, the size of the surface it was drawn for.
pub struct UiImage {
    pub pixels: Arc<[u8]>,
    pub width: u32,
    pub height: u32,
    /// Draw and copy time on the main thread, for the stats.
    pub draw_ms: f64,
    /// The first image after a language switch whose stages are timed (FR-024): the video
    /// thread logs its upload and present.
    pub language: bool,
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
    /// The finger forwarded as the pointer, until it lifts; other fingers are ignored.
    finger: Option<u64>,
    /// `Input.touch` as last set.
    touch: bool,
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
        // Surface images by physical size, corner radius, scale, fill and shadow strength; a few
        // surfaces, so the cache stays small, and it is dropped when it grows (tooltips vary with
        // their text).
        let cache = RefCell::new(HashMap::<(u32, u32, u32, u32, u32, u32), slint::Image>::new());
        app.global::<Menus>().on_item_top(|items, index, notice| {
            use slint::Model;
            let rows: Vec<MenuRow> = items.iter().map(|i| MenuRow::of(&i)).collect();
            item_top(&rows, index, notice)
        });
        app.global::<Menus>().on_next_selectable(|items, from, step| {
            use slint::Model;
            let selectable: Vec<bool> = items
                .iter()
                .map(|i| i.kind != "header" && i.kind != "state" && !i.disabled)
                .collect();
            next_selectable(&selectable, from, step)
        });
        app.global::<Menus>().on_with_header(|title, items| {
            use slint::Model;
            let header = MenuItem {
                text: title,
                kind: "header".into(),
                ..Default::default()
            };
            let rows: Vec<MenuItem> = std::iter::once(header).chain(items.iter()).collect();
            slint::ModelRc::new(slint::VecModel::from(rows))
        });
        app.global::<Theme>()
            .on_surface(move |width, height, radius, scale, fill, strength| {
                let (w, h) = shadows::image_size(width, height, scale);
                let fill = slint::RgbaColor::<u8>::from(fill);
                let fill = slint::Rgba8Pixel::new(fill.red, fill.green, fill.blue, fill.alpha);
                let key = (
                    w,
                    h,
                    radius.to_bits(),
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
                    .or_insert_with(|| {
                        slint::Image::from_rgba8(shadows::make(width, height, radius, scale, fill, strength))
                    })
                    .clone()
            });
        // The state message's actions (FR-033).
        let weak = app.as_weak();
        app.on_state_action(move |id| match id.as_str() {
            "open-devices" => {
                if let Some(app) = weak.upgrade() {
                    app.invoke_open_device_menu();
                }
            }
            "try-again" => {
                if let Some(app) = weak.upgrade() {
                    app.invoke_try_again();
                }
            }
            // Blocked on Windows (research R6).
            "open-privacy" => platform::open_privacy(platform::Capability::Camera),
            _ => log::warn!("state message: unknown action {id:?}"),
        });
        app.show()?;
        Ok(Self {
            window,
            app,
            buffer: Vec::new(),
            size: PhysicalSize::new(0, 0),
            scale: 1.0,
            cursor: slint::LogicalPosition::new(0.0, 0.0),
            finger: None,
            touch: false,
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
                self.set_touch(false);
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
                    self.set_touch(false);
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
            WindowEvent::Touch(touch) => return self.touch_event(touch),
            WindowEvent::Focused(active) => {
                self.app.global::<WindowState>().set_active(*active);
                SlintEvent::WindowActiveChanged(*active)
            }
            _ => return false,
        };
        self.window.window().dispatch_event(slint_event);
        true
    }

    /// A finger as the pointer with the left button (preview audio FR-011): Slint's public pointer
    /// events carry no finger, so `Input.touch` tells the UI the pointer is a finger. Neither
    /// system turns the app's touches into mouse events (winit handles Windows' pointer messages
    /// itself), so without this no touch reaches the UI. Only the first finger down acts.
    fn touch_event(&mut self, touch: &Touch) -> bool {
        let position = slint::LogicalPosition::new(
            touch.location.x as f32 / self.scale,
            touch.location.y as f32 / self.scale,
        );
        let button = PointerEventButton::Left;
        let events: &[SlintEvent] = match (self.finger, touch.phase) {
            (None, TouchPhase::Started) => {
                self.finger = Some(touch.id);
                self.app.global::<Input>().set_keyboard(false);
                self.set_touch(true);
                &[
                    SlintEvent::PointerMoved { position },
                    SlintEvent::PointerPressed { position, button },
                ]
            }
            (Some(id), TouchPhase::Moved) if id == touch.id => &[SlintEvent::PointerMoved { position }],
            (Some(id), TouchPhase::Ended) if id == touch.id => {
                self.finger = None;
                // No hover stays behind a lifted finger.
                &[
                    SlintEvent::PointerReleased { position, button },
                    SlintEvent::PointerExited,
                ]
            }
            (Some(id), TouchPhase::Cancelled) if id == touch.id => {
                self.finger = None;
                &[SlintEvent::PointerExited]
            }
            _ => return false,
        };
        self.cursor = position;
        for event in events {
            self.window.window().dispatch_event(event.clone());
        }
        true
    }

    fn set_touch(&mut self, touch: bool) {
        if touch != self.touch {
            self.touch = touch;
            self.app.global::<Input>().set_touch(touch);
        }
    }

    /// The window was destroyed (hidden in the tray on Wayland): no pointer, no focus, and the
    /// next image is drawn whole for the new window's video thread.
    pub fn window_gone(&mut self) {
        self.finger = None;
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
            language: false,
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

    /// Physical pixels per logical pixel.
    pub fn scale(&self) -> f32 {
        self.scale
    }

    /// The theme's deepest tone, the ground around the video: RGB from 0 to 1.
    pub fn backdrop(&self) -> [f64; 3] {
        let deep = self.app.global::<Theme>().get_c().deep;
        [deep.red(), deep.green(), deep.blue()].map(|v| f64::from(v) / 255.0)
    }
}

/// Selects the bundled catalog for `language` (FR-022); every `@tr` text, `Texts` included, is
/// evaluated again on its next read. Needs the component to exist.
pub fn select_language(language: &str) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_rows_stack_with_their_heights() {
        use MenuRow::*;
        let rows = [Header, Item, WithDetail, Item];
        assert_eq!(item_top(&rows, 0, 0.0), 0.0);
        assert_eq!(item_top(&rows, 1, 0.0), 32.0);
        assert_eq!(item_top(&rows, 2, 0.0), 70.0);
        assert_eq!(item_top(&rows, 3, 0.0), 120.0);
        // The list's height is the end minus one spacing.
        assert_eq!(item_top(&rows, 4, 0.0) - MENU_SPACING, 156.0);
        assert_eq!(item_top(&rows, 9, 0.0), item_top(&rows, 4, 0.0));
        assert_eq!(item_top(&rows, -1, 0.0), 0.0);
        // A notice is as tall as Slint measured it.
        let rows = [State, Action, Header];
        assert_eq!(item_top(&rows, 1, 58.0), 60.0);
        assert_eq!(item_top(&rows, 2, 58.0), 104.0);
    }

    #[test]
    fn menu_rows_follow_their_kind_and_detail() {
        let row = |kind: &str, detail: &str| {
            MenuRow::of(&MenuItem {
                kind: kind.into(),
                detail: detail.into(),
                ..Default::default()
            })
        };
        assert_eq!(row("header", ""), MenuRow::Header);
        assert_eq!(row("", ""), MenuRow::Item);
        assert_eq!(row("", "detail"), MenuRow::WithDetail);
        assert_eq!(row("state", "Waiting to reconnect"), MenuRow::State);
        // A notice with neither a name nor a detail line is as tall as an item; a name alone
        // (Cannot open / {}) makes it a full notice.
        assert_eq!(row("state", ""), MenuRow::Item);
        let named = MenuItem {
            kind: "state".into(),
            name: "HDMI (ezcap Live GAMER ULTRA)".into(),
            ..Default::default()
        };
        assert_eq!(MenuRow::of(&named), MenuRow::State);
        assert_eq!(row("action", ""), MenuRow::Action);
    }

    #[test]
    fn menu_keys_skip_headers_and_disabled_items() {
        // header, item, disabled, item, header, item
        let menu = [false, true, false, true, false, true];
        assert_eq!(next_selectable(&menu, -1, 1), 1, "Home");
        assert_eq!(next_selectable(&menu, 6, -1), 5, "End");
        assert_eq!(next_selectable(&menu, 1, 1), 3);
        assert_eq!(next_selectable(&menu, 3, 1), 5);
        assert_eq!(next_selectable(&menu, 5, 1), 5, "stays at the last");
        assert_eq!(next_selectable(&menu, 3, -1), 1);
        assert_eq!(
            next_selectable(&menu, 1, -1),
            1,
            "stays at the first, not on the header"
        );
        assert_eq!(next_selectable(&[], -1, 1), -1);
        assert_eq!(next_selectable(&[false, false], -1, 1), -1);
    }
}
