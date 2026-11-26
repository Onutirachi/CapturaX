//! The ambient light around the video (preview extras research R1–R4a, data-model.md "Glow"):
//! the math and the GPU passes. The glow eases a small color field of the video over time, draws
//! it only in the bars between the letterboxed video and its area, and fades with the toggle.

use std::time::{Duration, Instant};

use wgpu::util::DeviceExt as _;

use super::gpu::{OPACITY, Shaders, UNIFORM_LEN, placement};

// The look's tune values (research R2): they start from the mockup and are set with the
// maintainer on the ezcap with the PS5 (T008). A change in feel goes into the spec's FR-002 only
// if the variant changes.

/// How much the field is enlarged on each side of the video area: 5 % (the mockup's −5 %, ×1.1).
pub const EDGE_GROW: f32 = 0.05;
/// The Gaussian blur's deviation on the 32 × 18 field, in texels.
pub const BLUR_SIGMA: f32 = 2.0;
/// The saturation applied when drawing.
pub const SATURATION: f32 = 1.5;
/// The lift after the saturation: each channel to this power, so dark colours brighten a lot,
/// bright ones little, and none passes 1 (no clipping, the hue kept).
pub const LIFT: f32 = 0.5;
/// The glow's alpha before the video's opacity and the toggle fade. No luminance gate: dark scenes
/// show their dark colours (FR-003).
pub const ALPHA: f32 = 0.9;
/// How long the toggle takes to fade the glow in or out.
pub const FADE: Duration = Duration::from_millis(250);

/// The easing's time constant (research R1, the mockup's τ).
pub const TAU: Duration = Duration::from_millis(150);
/// The longest time step the easing takes; a longer pause fills the history instead (R4a).
pub const MAX_STEP: Duration = Duration::from_millis(100);
/// The shortest time between two updates (research R1): every frame at 60 fps even with the
/// frames' arrival jitter, every other frame at 120 fps.
pub const UPDATE_INTERVAL: Duration = Duration::from_millis(15);

/// How much of the new field one update blends into the history: `1 − exp(−dt / τ)`, with `dt`
/// clamped to [`MAX_STEP`], so the easing does not depend on the frame rate.
pub fn ease_factor(dt: Duration) -> f32 {
    let dt = dt.min(MAX_STEP);
    1.0 - (-dt.as_secs_f32() / TAU.as_secs_f32()).exp()
}

/// The bars between the letterboxed `video` (x, y, width, height in physical pixels, as the
/// letterbox gives it) and its `area` (the video area's scissor rectangle): two side bars, or a
/// top and a bottom bar, as whole-pixel rectangles (research R3). A bar not wider than 1 physical
/// pixel is none, so a video that fills the area has no bar and costs nothing.
pub fn bars(area: [u32; 4], video: [f32; 4]) -> [Option<[u32; 4]>; 2] {
    let [ax, ay, aw, ah] = area.map(i64::from);
    let (ax1, ay1) = (ax + aw, ay + ah);
    // The video's edges, whole pixels, inside the area.
    let edge = |v: f32, low: i64, high: i64| (v.round() as i64).clamp(low, high);
    let vx0 = edge(video[0], ax, ax1);
    let vx1 = edge(video[0] + video[2], vx0, ax1);
    let vy0 = edge(video[1], ay, ay1);
    let vy1 = edge(video[1] + video[3], vy0, ay1);
    let bar = |x: i64, y: i64, w: i64, h: i64, thickness: i64| {
        (thickness > 1 && w > 0 && h > 0).then(|| [x, y, w, h].map(|v| v as u32))
    };
    let side = (vx0 - ax).max(ax1 - vx1);
    let top = (vy0 - ay).max(ay1 - vy1);
    if side >= top {
        [
            bar(ax, ay, vx0 - ax, ah, vx0 - ax),
            bar(vx1, ay, ax1 - vx1, ah, ax1 - vx1),
        ]
    } else {
        [
            bar(ax, ay, aw, vy0 - ay, vy0 - ay),
            bar(ax, vy1, aw, ay1 - vy1, ay1 - vy1),
        ]
    }
}

/// What an update does now (research R1, R4a).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Due {
    /// Too soon after the last update: skip it.
    Wait,
    /// Ease the history towards the field with the real time step since the last update.
    Ease(Duration),
    /// Set the history to the field directly (factor 1): the first update, or the first after a
    /// pause longer than [`MAX_STEP`], so a stale field never shows.
    Fill,
}

/// Whether an update runs at `now`, given the `last` one: at most every [`UPDATE_INTERVAL`] (at
/// 120 fps every other frame), with the real time step, or a fill after a pause.
pub fn due(last: Option<Instant>, now: Instant) -> Due {
    let Some(last) = last else {
        return Due::Fill;
    };
    let dt = now.saturating_duration_since(last);
    if dt > MAX_STEP {
        Due::Fill
    } else if dt < UPDATE_INTERVAL {
        Due::Wait
    } else {
        Due::Ease(dt)
    }
}

/// The work targets' sizes (research R1): the convert target, then each 2× box pass down to the
/// 32 × 18 field.
const CHAIN: [(u32, u32); 5] = [(512, 288), (256, 144), (128, 72), (64, 36), (32, 18)];
/// The field's size: the history, the blurred copy and the last box target.
const FIELD: (u32, u32) = CHAIN[CHAIN.len() - 1];
/// Every glow texture's format: an easing step this small (about 0.05 at 120 fps) would stall in
/// 8 bits.
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
/// The draw's layer kind in the composite shader (composite.wgsl).
const KIND_GLOW: f32 = 8.0;

/// The video layer the convert pass reads: its planes' views, the shader's kind, the picture's
/// size in pixels and its YUV to RGB rows.
pub struct Source<'a> {
    pub views: &'a [wgpu::TextureView],
    pub kind: f32,
    pub size: (u32, u32),
    pub rows: [[f32; 4]; 3],
}

/// One pipeline per pass, all into [`FORMAT`] without blending.
struct Pipelines {
    convert: wgpu::RenderPipeline,
    reduce: wgpu::RenderPipeline,
    ease: wgpu::RenderPipeline,
    blur: wgpu::RenderPipeline,
}

/// The ambient light's GPU side (research R1), made once per GPU: the passes run after a present,
/// on the planes already uploaded, and leave the blurred field for the next present. Nothing is read back.
pub struct Glow {
    pipelines: Pipelines,
    /// The convert and box targets, 512 × 288 down to 32 × 18. The last one is the field, and is
    /// reused as the blur's middle step once the history has taken it.
    chain: [wgpu::TextureView; 5],
    /// The eased field, two textures used in turn: `current` is the last one written.
    history: [wgpu::TextureView; 2],
    current: usize,
    blurred: wgpu::TextureView,
    /// The convert pass's uniform (the video's kind, size and rows) and its bind group on the
    /// video layer's planes, made again when the layer is.
    convert_rect: wgpu::Buffer,
    convert: Option<wgpu::BindGroup>,
    /// Box pass `i` reads chain target `i`.
    reduce: [wgpu::BindGroup; 4],
    /// The easing's uniform (its factor) and, for each history written, its bind group.
    ease_rect: wgpu::Buffer,
    ease: [wgpu::BindGroup; 2],
    /// The horizontal blur of each history into the field, then the vertical one into `blurred`.
    blur_across: [wgpu::BindGroup; 2],
    blur_down: wgpu::BindGroup,
    /// The draw's uniform and its bind group on the blurred field.
    draw_rect: wgpu::Buffer,
    draw: wgpu::BindGroup,
    /// When the last update ran; `None` until the first, and after a reset, so it fills.
    last: Option<Instant>,
}

impl Glow {
    pub fn new(device: &wgpu::Device, shaders: &Shaders) -> Self {
        let pipeline = |entry| shaders.pipeline(device, entry, FORMAT, None);
        let pipelines = Pipelines {
            convert: pipeline("glow_convert"),
            reduce: pipeline("glow_box"),
            ease: pipeline("glow_ease"),
            blur: pipeline("glow_blur"),
        };
        let target = |(width, height): (u32, u32)| {
            device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some("glow"),
                    size: wgpu::Extent3d {
                        width,
                        height,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: FORMAT,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING
                        | wgpu::TextureUsages::RENDER_ATTACHMENT
                        | wgpu::TextureUsages::COPY_SRC,
                    view_formats: &[],
                })
                .create_view(&Default::default())
        };
        let chain = CHAIN.map(target);
        let history = [target(FIELD), target(FIELD)];
        let blurred = target(FIELD);

        let rect = |values: [f32; UNIFORM_LEN]| {
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("glow"),
                contents: bytemuck::cast_slice(&values),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            })
        };
        // The box passes only need the bounds; the blur adds its direction.
        let plain = rect(pass_uniform(FIELD));
        let blur = |step: [f32; 2]| {
            let mut values = pass_uniform(FIELD);
            values[TO_R..TO_R + 3].copy_from_slice(&[step[0], step[1], BLUR_SIGMA]);
            rect(values)
        };
        let (across, down) = (blur([1.0, 0.0]), blur([0.0, 1.0]));
        let convert_rect = rect(pass_uniform(CHAIN[0]));
        let ease_rect = rect(pass_uniform(FIELD));

        let bind = |views: &[&wgpu::TextureView], rect: &wgpu::Buffer| {
            let views: Vec<wgpu::TextureView> = views.iter().map(|&v| v.clone()).collect();
            shaders.bind(device, &views, rect)
        };
        let field = &chain[CHAIN.len() - 1];
        let reduce = std::array::from_fn(|i| bind(&[&chain[i]], &plain));
        // Writing history `n` reads the field and the other history.
        let ease = std::array::from_fn(|n| bind(&[field, &history[1 - n]], &ease_rect));
        let blur_across = std::array::from_fn(|n| bind(&[&history[n]], &across));
        let blur_down = bind(&[field], &down);
        let draw_rect = rect([0.0; UNIFORM_LEN]);
        let draw = bind(&[&blurred], &draw_rect);
        Self {
            pipelines,
            chain,
            history,
            current: 0,
            blurred,
            convert_rect,
            convert: None,
            reduce,
            ease_rect,
            ease,
            blur_across,
            blur_down,
            draw_rect,
            draw,
            last: None,
        }
    }

    /// The video layer was made again: the next update binds its new planes.
    pub fn forget_source(&mut self) {
        self.convert = None;
    }

    /// The video is gone: the next update fills the history from its own frame (research R4a).
    pub fn reset(&mut self) {
        self.convert = None;
        self.pause();
    }

    /// The updates stop (no bar): the next one fills the history, and nothing is drawn until then,
    /// so a stale field never shows (research R4a).
    pub fn pause(&mut self) {
        self.last = None;
    }

    /// The draw's bind group, its uniform written for the video `area` (x, y, width, height, corner
    /// radius in physical pixels) on a `surface` of that size, at `opacity` (research R2). The field is stretched over the area enlarged by [`EDGE_GROW`] on each
    /// side. `None` while there is no field: before the first update, and after a reset or a pause
    /// until the next one.
    pub fn draw(
        &self,
        queue: &wgpu::Queue,
        area: [f32; 5],
        surface: (f32, f32),
        opacity: f32,
    ) -> Option<&wgpu::BindGroup> {
        self.last?;
        let [x, y, w, h, _] = area;
        let (gx, gy) = (w * EDGE_GROW, h * EDGE_GROW);
        let mut values = [0.0; UNIFORM_LEN];
        values[..8].copy_from_slice(&placement([x - gx, y - gy, w + 2.0 * gx, h + 2.0 * gy], 0.0, surface));
        values[OPACITY] = opacity;
        values[KIND..KIND + 3].copy_from_slice(&[KIND_GLOW, FIELD.0 as f32, FIELD.1 as f32]);
        values[TO_R..TO_R + 2].copy_from_slice(&[SATURATION, LIFT]);
        queue.write_buffer(&self.draw_rect, 0, bytemuck::cast_slice(&values));
        Some(&self.draw)
    }

    /// Runs the passes on `source` in one command buffer when an update is [`due`] at `now`: true
    /// when it ran. A fill sets the history to the field (factor 1); an easing moves it by
    /// [`ease_factor`] of the real time step.
    pub fn update(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        shaders: &Shaders,
        source: &Source,
        now: Instant,
    ) -> bool {
        let factor = match due(self.last, now) {
            Due::Wait => return false,
            Due::Fill => 1.0,
            Due::Ease(dt) => ease_factor(dt),
        };
        self.last = Some(now);
        let convert = self
            .convert
            .get_or_insert_with(|| shaders.bind(device, source.views, &self.convert_rect));
        let mut values = pass_uniform(CHAIN[0]);
        values[KIND..KIND + 3].copy_from_slice(&[source.kind, source.size.0 as f32, source.size.1 as f32]);
        for (i, row) in source.rows.iter().enumerate() {
            values[TO_R + i * 4..TO_R + 4 + i * 4].copy_from_slice(row);
        }
        queue.write_buffer(&self.convert_rect, 0, bytemuck::cast_slice(&values));
        let mut values = pass_uniform(FIELD);
        values[TO_R] = factor;
        queue.write_buffer(&self.ease_rect, 0, bytemuck::cast_slice(&values));

        let next = 1 - self.current;
        let p = &self.pipelines;
        let mut passes: Vec<(&wgpu::RenderPipeline, &wgpu::BindGroup, &wgpu::TextureView)> =
            vec![(&p.convert, convert, &self.chain[0])];
        for (i, group) in self.reduce.iter().enumerate() {
            passes.push((&p.reduce, group, &self.chain[i + 1]));
        }
        let field = &self.chain[CHAIN.len() - 1];
        passes.extend([
            (&p.ease, &self.ease[next], &self.history[next]),
            (&p.blur, &self.blur_across[next], field),
            (&p.blur, &self.blur_down, &self.blurred),
        ]);
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("glow") });
        for (pipeline, group, target) in passes {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("glow"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, group, &[]);
            pass.draw(0..4, 0..1);
        }
        queue.submit([encoder.finish()]);
        self.current = next;
        true
    }
}

/// Where the kind and the picture size, and the first row (`to_r`), are in the uniform.
const KIND: usize = 8;
const TO_R: usize = 12;

/// A pass's uniform: bounds over the whole target, `size` the target's size.
fn pass_uniform((width, height): (u32, u32)) -> [f32; UNIFORM_LEN] {
    let (w, h) = (width as f32, height as f32);
    let mut values = [0.0; UNIFORM_LEN];
    values[..8].copy_from_slice(&placement([0.0, 0.0, w, h], 0.0, (w, h)));
    values
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(v: f64) -> Duration {
        Duration::from_micros((v * 1000.0).round() as u64)
    }

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-5
    }

    #[test]
    fn ease_factor_follows_the_time_constant() {
        assert_eq!(ease_factor(Duration::ZERO), 0.0);
        assert!(close(ease_factor(MAX_STEP), 1.0 - (-100.0f32 / 150.0).exp()));
        assert!(close(ease_factor(ms(16.7)), 1.0 - (-16.7f32 / 150.0).exp()));
        // Two 120 fps steps give the same as one 60 fps step: the rate does not change the feel.
        let half = ease_factor(ms(8.35));
        assert!(close(1.0 - (1.0 - half) * (1.0 - half), ease_factor(ms(16.7))));
    }

    #[test]
    fn ease_factor_clamps_the_step() {
        assert_eq!(ease_factor(Duration::from_secs(5)), ease_factor(MAX_STEP));
        assert!(ease_factor(MAX_STEP) < 0.5);
    }

    #[test]
    fn one_white_frame_does_not_flash() {
        // FR-003: a black history meets one white field at 60 fps, then black again.
        let bound = 1.0 - (-16.7f32 / 150.0).exp();
        let mut history = 0.0f32;
        let mut highest = 0.0f32;
        for field in [1.0, 0.0, 0.0, 0.0] {
            history += (field - history) * ease_factor(ms(16.7));
            highest = highest.max(history);
        }
        assert!(highest <= bound + 1e-6, "{highest} > {bound}");
        assert!(highest > 0.1 && highest < 0.11, "{highest}");
    }

    #[test]
    fn no_bars_when_the_video_fills_the_area() {
        assert_eq!(bars([0, 0, 1920, 1080], [0.0, 0.0, 1920.0, 1080.0]), [None, None]);
        // A 1 pixel bar from rounding is none.
        assert_eq!(bars([0, 32, 1921, 1080], [0.0, 32.0, 1920.0, 1080.0]), [None, None]);
        assert_eq!(bars([0, 0, 1922, 1080], [1.0, 0.0, 1920.0, 1080.0]), [None, None]);
    }

    #[test]
    fn side_bars() {
        // 4:3 video in a 16:9 area with an offset (the letterbox test's case).
        assert_eq!(
            bars([0, 32, 1600, 900], [200.0, 32.0, 1200.0, 900.0]),
            [Some([0, 32, 200, 900]), Some([1400, 32, 200, 900])]
        );
        // Uneven by rounding: each bar keeps its own width.
        assert_eq!(
            bars([10, 0, 805, 600], [12.0, 0.0, 800.0, 600.0]),
            [Some([10, 0, 2, 600]), Some([812, 0, 3, 600])]
        );
        // Only one side wider than 1 pixel.
        assert_eq!(
            bars([0, 0, 803, 600], [1.0, 0.0, 800.0, 600.0]),
            [None, Some([801, 0, 2, 600])]
        );
    }

    #[test]
    fn top_and_bottom_bars() {
        // 16:9 video in a 4:3 area.
        assert_eq!(
            bars([0, 0, 800, 600], [0.0, 75.0, 800.0, 450.0]),
            [Some([0, 0, 800, 75]), Some([0, 525, 800, 75])]
        );
        // A 1 pixel side gap from rounding does not hide the top and bottom bars.
        assert_eq!(
            bars([0, 0, 801, 600], [0.0, 75.0, 800.0, 450.0]),
            [Some([0, 0, 801, 75]), Some([0, 525, 801, 75])]
        );
    }

    #[test]
    fn bars_stay_inside_the_area() {
        // A video larger than the area (it never is, but the scissor clips it anyway).
        assert_eq!(bars([0, 0, 800, 600], [-10.0, -10.0, 820.0, 620.0]), [None, None]);
        assert_eq!(bars([0, 0, 0, 0], [0.0, 0.0, 0.0, 0.0]), [None, None]);
    }

    #[test]
    fn pacing() {
        let start = Instant::now();
        let at = |v: f64| start + ms(v);
        assert_eq!(due(None, start), Due::Fill);
        // 120 fps: every other frame updates, with the real step.
        assert_eq!(due(Some(start), at(8.3)), Due::Wait);
        assert_eq!(due(Some(start), at(14.9)), Due::Wait);
        assert_eq!(due(Some(start), at(16.7)), Due::Ease(ms(16.7)));
        // 60 fps with arrival jitter: a frame a little early still updates.
        assert_eq!(due(Some(start), at(15.0)), Due::Ease(ms(15.0)));
        assert_eq!(due(Some(start), at(16.0)), Due::Ease(ms(16.0)));
        // 30 fps: every frame.
        assert_eq!(due(Some(start), at(33.3)), Due::Ease(ms(33.3)));
        // Up to the longest step it still eases; past it, it fills.
        assert_eq!(due(Some(start), at(100.0)), Due::Ease(MAX_STEP));
        assert_eq!(due(Some(start), at(100.1)), Due::Fill);
        assert_eq!(due(Some(start), at(60_000.0)), Due::Fill);
        // A clock that went back waits instead of easing by a negative step.
        assert_eq!(due(Some(at(20.0)), start), Due::Wait);
    }

    // The passes on a real GPU, without a surface. Only these tests read back; a machine without
    // an adapter skips them.

    use capture::{Matrix, Range, conversion};

    /// The shader's kinds (composite.wgsl).
    const RGB: f32 = 1.0;
    const NV12: f32 = 2.0;
    const I420: f32 = 3.0;
    const YUY2: f32 = 4.0;
    const UYVY: f32 = 5.0;

    /// Rec. 709 luminance weights.
    const WEIGHTS: [f32; 3] = [0.2126, 0.7152, 0.0722];

    struct Rig {
        device: wgpu::Device,
        queue: wgpu::Queue,
        shaders: Shaders,
        glow: Glow,
    }

    fn rig() -> Option<Rig> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let Ok(adapter) = pollster::block_on(instance.request_adapter(&Default::default())) else {
            eprintln!("no GPU adapter: skipped");
            return None;
        };
        let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).ok()?;
        let shaders = Shaders::new(&device);
        // The layers' pipeline comes from the same module and helper.
        shaders.pipeline(
            &device,
            "fs",
            wgpu::TextureFormat::Bgra8Unorm,
            Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
        );
        let glow = Glow::new(&device, &shaders);
        Some(Rig {
            device,
            queue,
            shaders,
            glow,
        })
    }

    impl Rig {
        /// A plane texture of `format`, `width` × `height` texels, holding `bytes`.
        fn plane(&self, format: wgpu::TextureFormat, width: u32, height: u32, bytes: &[u8]) -> wgpu::TextureView {
            let size = wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            };
            let texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: None,
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            self.queue.write_texture(
                texture.as_image_copy(),
                bytes,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(bytes.len() as u32 / height),
                    rows_per_image: None,
                },
                size,
            );
            texture.create_view(&Default::default())
        }

        fn update(&mut self, views: &[wgpu::TextureView], kind: f32, size: (u32, u32), now: Instant) -> bool {
            // The tests change textures between updates; the app uploads into the same layer.
            self.glow.forget_source();
            let source = Source {
                views,
                kind,
                size,
                rows: conversion(Matrix::Bt709, Range::Limited),
            };
            self.glow.update(&self.device, &self.queue, &self.shaders, &source, now)
        }

        /// A glow texture's texels, row by row.
        fn read(&self, view: &wgpu::TextureView) -> Vec<[f32; 4]> {
            let texture = view.texture();
            let (width, height) = (texture.width(), texture.height());
            let row = (width * 8).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
            let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: u64::from(row * height),
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            let mut encoder = self.device.create_command_encoder(&Default::default());
            encoder.copy_texture_to_buffer(
                texture.as_image_copy(),
                wgpu::TexelCopyBufferInfo {
                    buffer: &buffer,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(row),
                        rows_per_image: None,
                    },
                },
                texture.size(),
            );
            self.queue.submit([encoder.finish()]);
            buffer.slice(..).map_async(wgpu::MapMode::Read, |r| r.expect("mapped"));
            self.device.poll(wgpu::PollType::wait_indefinitely()).expect("polled");
            let bytes = buffer.slice(..).get_mapped_range().expect("mapped range");
            let mut texels = Vec::new();
            for y in 0..height {
                for x in 0..width {
                    let at = (y * row + x * 8) as usize;
                    texels.push(std::array::from_fn(|c| {
                        half(u16::from_le_bytes([bytes[at + c * 2], bytes[at + c * 2 + 1]]))
                    }));
                }
            }
            texels
        }

        /// The current history's average luminance (Rec. 709), to follow the easing.
        fn luminance(&self) -> f32 {
            let texels = self.read(&self.glow.history[self.glow.current]);
            let sum: f32 = texels
                .iter()
                .map(|t| (0..3).map(|c| t[c] * WEIGHTS[c]).sum::<f32>())
                .sum();
            sum / texels.len() as f32
        }

        /// The glow drawn at `opacity` with the layers' pipeline over a cleared 64 × 36 target
        /// that is the whole video area; `None` when the glow has no field to draw.
        fn draw(&self, opacity: f32) -> Option<Vec<[f32; 4]>> {
            let (w, h) = (64, 36);
            let pipeline = self.shaders.pipeline(
                &self.device,
                "fs",
                FORMAT,
                Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
            );
            let area = [0.0, 0.0, w as f32, h as f32, 0.0];
            let group = self.glow.draw(&self.queue, area, (w as f32, h as f32), opacity)?;
            let target = self
                .device
                .create_texture(&wgpu::TextureDescriptor {
                    label: None,
                    size: wgpu::Extent3d {
                        width: w,
                        height: h,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: FORMAT,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                    view_formats: &[],
                })
                .create_view(&Default::default());
            let mut encoder = self.device.create_command_encoder(&Default::default());
            {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: None,
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &target,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    ..Default::default()
                });
                pass.set_pipeline(&pipeline);
                pass.set_bind_group(0, group, &[]);
                pass.draw(0..4, 0..1);
            }
            self.queue.submit([encoder.finish()]);
            Some(self.read(&target))
        }
    }

    /// An IEEE half float's value (normal and subnormal numbers).
    fn half(bits: u16) -> f32 {
        let sign = if bits & 0x8000 != 0 { -1.0 } else { 1.0 };
        let exponent = i32::from((bits >> 10) & 0x1f);
        let fraction = f32::from(bits & 0x3ff);
        match exponent {
            0 => sign * fraction * 2f32.powi(-24),
            _ => sign * (1.0 + fraction / 1024.0) * 2f32.powi(exponent - 15),
        }
    }

    fn near(a: f32, b: f32) -> bool {
        (a - b).abs() < 4e-3
    }

    fn rgba(width: u32, height: u32, color: impl Fn(u32) -> [u8; 4]) -> Vec<u8> {
        (0..width * height).flat_map(|i| color(i % width)).collect()
    }

    #[test]
    fn the_passes_fill_ease_wait_and_reset() {
        let Some(mut rig) = rig() else { return };
        use wgpu::TextureFormat::Rgba8Unorm;
        let size = (64, 36);
        let red = [rig.plane(Rgba8Unorm, 64, 36, &rgba(64, 36, |_| [255, 0, 0, 255]))];
        let green = [rig.plane(Rgba8Unorm, 64, 36, &rgba(64, 36, |_| [0, 255, 0, 255]))];
        let start = Instant::now();

        // The first update fills: the field is the frame's color at once.
        assert!(rig.update(&red, RGB, size, start));
        assert!(near(rig.luminance(), WEIGHTS[0]), "{}", rig.luminance());
        for texel in rig.read(&rig.glow.blurred) {
            assert!(near(texel[0], 1.0) && near(texel[1], 0.0), "{texel:?}");
        }

        // A 60 fps step eases by its factor.
        let step = Duration::from_micros(16_700);
        assert!(rig.update(&green, RGB, size, start + step));
        let f = ease_factor(step);
        let expected = WEIGHTS[0] * (1.0 - f) + WEIGHTS[1] * f;
        assert!(near(rig.luminance(), expected), "{} != {expected}", rig.luminance());

        // Too soon: nothing runs and nothing changes.
        assert!(!rig.update(&green, RGB, size, start + step + Duration::from_millis(5)));
        assert!(near(rig.luminance(), expected));

        // After a reset the next frame fills again.
        rig.glow.reset();
        assert!(rig.update(&green, RGB, size, start + 2 * step));
        assert!(near(rig.luminance(), WEIGHTS[1]), "{}", rig.luminance());

        // A pause longer than the longest step fills too.
        assert!(rig.update(&red, RGB, size, start + 2 * step + Duration::from_millis(150)));
        assert!(near(rig.luminance(), WEIGHTS[0]), "{}", rig.luminance());
    }

    #[test]
    fn the_draw_saturates_brightens_and_waits_for_a_field() {
        let Some(mut rig) = rig() else { return };
        use wgpu::TextureFormat::Rgba8Unorm;
        let size = (64, 36);
        let solid = |rig: &Rig, c: [u8; 3]| [rig.plane(Rgba8Unorm, 64, 36, &rgba(64, 36, |_| [c[0], c[1], c[2], 255]))];
        // Nothing to draw before the first update.
        assert!(rig.draw(1.0).is_none());

        // A colour: the saturation pushes it away from its grey (red past 1 is clamped) and the
        // lift brightens it.
        let bytes = [230u8, 115, 51];
        let color = bytes.map(|b| f32::from(b) / 255.0);
        let l = (0..3).map(|c| color[c] * WEIGHTS[c]).sum::<f32>();
        let views = solid(&rig, bytes);
        assert!(rig.update(&views, RGB, size, Instant::now()));
        let shown = color.map(|c| (l + SATURATION * (c - l)).clamp(0.0, 1.0).powf(LIFT));
        assert!(shown[0] == 1.0 && shown[1] < 1.0 && shown[2] < 0.25, "{shown:?}");
        for texel in rig.draw(ALPHA).expect("a field") {
            for c in 0..3 {
                assert!(near(texel[c], shown[c] * ALPHA), "{texel:?} != {shown:?} × {ALPHA}");
            }
            assert!(near(texel[3], ALPHA), "{texel:?}");
        }
        // Half the intensity halves the drawn colour and alpha (FR-001a).
        for texel in rig.draw(ALPHA / 2.0).expect("a field") {
            for c in 0..3 {
                assert!(near(texel[c], shown[c] * ALPHA / 2.0), "{texel:?}");
            }
            assert!(near(texel[3], ALPHA / 2.0), "{texel:?}");
        }

        // A dark frame draws its own dark colour, opaque: the glow never fades out (FR-003).
        rig.glow.reset();
        let dark: f32 = 10.0 / 255.0;
        let views = solid(&rig, [10, 10, 10]);
        assert!(rig.update(&views, RGB, size, Instant::now()));
        for texel in rig.draw(1.0).expect("a field") {
            assert!(near(texel[0], dark.powf(LIFT)) && near(texel[3], 1.0), "{texel:?}");
        }

        // Paused (no bar): nothing is drawn until the next update fills.
        rig.glow.pause();
        assert!(rig.draw(1.0).is_none());
    }

    #[test]
    fn every_format_converts() {
        let Some(mut rig) = rig() else { return };
        use wgpu::TextureFormat::{Bgra8Unorm, R8Unorm, Rg8Unorm, Rgba8Unorm};
        let (w, h) = (64u32, 36u32);
        let (hw, hh) = (w / 2, h / 2);
        let px = |n: u32, v: &[u8]| v.repeat(n as usize);
        // Limited range Y 180 with neutral chroma is grey (180 − 16) / 219.
        let grey = (180.0 - 16.0) / 219.0;
        let cases: Vec<(&str, f32, Vec<wgpu::TextureView>, f32)> = vec![
            (
                "nv12",
                NV12,
                vec![
                    rig.plane(R8Unorm, w, h, &px(w * h, &[180])),
                    rig.plane(Rg8Unorm, hw, hh, &px(hw * hh, &[128, 128])),
                ],
                grey,
            ),
            (
                "i420",
                I420,
                vec![
                    rig.plane(R8Unorm, w, h, &px(w * h, &[180])),
                    rig.plane(R8Unorm, hw, hh, &px(hw * hh, &[128])),
                    rig.plane(R8Unorm, hw, hh, &px(hw * hh, &[128])),
                ],
                grey,
            ),
            (
                "yuy2",
                YUY2,
                vec![rig.plane(Rgba8Unorm, hw, h, &px(hw * h, &[180, 128, 180, 128]))],
                grey,
            ),
            (
                "uyvy",
                UYVY,
                vec![rig.plane(Rgba8Unorm, hw, h, &px(hw * h, &[128, 180, 128, 180]))],
                grey,
            ),
            // BGRx: the bytes are blue, green, red, so this is red.
            (
                "bgrx",
                RGB,
                vec![rig.plane(Bgra8Unorm, w, h, &px(w * h, &[0, 0, 255, 255]))],
                WEIGHTS[0],
            ),
        ];
        for (name, kind, views, expected) in cases {
            rig.glow.reset();
            assert!(rig.update(&views, kind, (w, h), Instant::now()));
            let l = rig.luminance();
            assert!(near(l, expected), "{name}: {l} != {expected}");
        }
    }

    #[test]
    fn the_field_keeps_the_sides_and_blurs_between() {
        let Some(mut rig) = rig() else { return };
        // Left half white, right half black.
        let frame = rgba(1920, 1080, |x| if x < 960 { [255; 4] } else { [0, 0, 0, 255] });
        let views = [rig.plane(wgpu::TextureFormat::Rgba8Unorm, 1920, 1080, &frame)];
        assert!(rig.update(&views, RGB, (1920, 1080), Instant::now()));
        assert!(near(rig.luminance(), 0.5), "{}", rig.luminance());
        let blurred = rig.read(&rig.glow.blurred);
        let at = |x: usize, y: usize| blurred[y * FIELD.0 as usize + x][0];
        for y in 0..FIELD.1 as usize {
            assert!(
                at(0, y) > 0.99 && at(31, y) < 0.01,
                "row {y}: {} {}",
                at(0, y),
                at(31, y)
            );
            // The blur crosses the edge, symmetrically.
            assert!(
                at(15, y) < 0.95 && at(16, y) > 0.05,
                "row {y}: {} {}",
                at(15, y),
                at(16, y)
            );
            assert!(near(at(15, y) + at(16, y), 1.0), "row {y}");
        }
    }
}
