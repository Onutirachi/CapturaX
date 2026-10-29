//! Ambient light (T034): each video frame is shrunk to a small texture, blurred in two passes and
//! blended into the previous glow, which the compositor draws stretched behind the video. Follows
//! the previous app's AmbientLight.tsx: 96 × 54, blur 6 px, saturation 1.4, 35% of each new frame
//! at 30 Hz.

use std::time::Instant;

/// Size of the glow texture.
pub const WIDTH: u32 = 96;
pub const HEIGHT: u32 = 54;
/// Share of a new frame in the glow at 30 Hz, scaled to the real interval between frames.
const BLEND: f64 = 0.35;
const BLEND_INTERVAL: f64 = 1.0 / 30.0;
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

pub struct Ambient {
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    shrink: wgpu::RenderPipeline,
    blur_x: wgpu::RenderPipeline,
    /// Blends into the glow with the blend constant.
    blur_y: wgpu::RenderPipeline,
    small: wgpu::TextureView,
    small_group: wgpu::BindGroup,
    blurred_x: wgpu::TextureView,
    blurred_x_group: wgpu::BindGroup,
    /// When the glow was last updated; none until the first frame after a reset.
    last: Option<Instant>,
}

impl Ambient {
    pub fn new(device: &wgpu::Device) -> Self {
        let shader = device.create_shader_module(wgpu::include_wgsl!("ambient.wgsl"));
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ambient"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("ambient"),
            bind_group_layouts: &[Some(&layout)],
            ..Default::default()
        });
        let pipeline = |entry: &str, blend: Option<wgpu::BlendState>| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(entry),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(entry),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState { format: FORMAT, blend, write_mask: wgpu::ColorWrites::ALL })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let mix = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::Constant,
            dst_factor: wgpu::BlendFactor::OneMinusConstant,
            operation: wgpu::BlendOperation::Add,
        };
        let shrink = pipeline("fs_shrink", None);
        let blur_x = pipeline("fs_blur_x", None);
        let blur_y = pipeline("fs_blur_y", Some(wgpu::BlendState { color: mix, alpha: mix }));
        // Clamps to the edge by default.
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let small = target(device, "ambient small");
        let small_group = group(device, &layout, &sampler, &small);
        let blurred_x = target(device, "ambient blurred x");
        let blurred_x_group = group(device, &layout, &sampler, &blurred_x);
        Self { layout, sampler, shrink, blur_x, blur_y, small, small_group, blurred_x, blurred_x_group, last: None }
    }

    /// The next frame replaces the glow instead of blending into it (after the light was off).
    pub fn reset(&mut self) {
        self.last = None;
    }

    /// Whether the glow holds a frame since the last reset.
    pub fn ready(&self) -> bool {
        self.last.is_some()
    }

    /// Updates `glow` (WIDTH × HEIGHT, Rgba8Unorm, a render attachment) from the video frame.
    pub fn update(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        video: &wgpu::TextureView,
        glow: &wgpu::TextureView,
    ) {
        let now = Instant::now();
        let share = match self.last {
            None => 1.0,
            Some(last) => 1.0 - (1.0 - BLEND).powf(now.duration_since(last).as_secs_f64() / BLEND_INTERVAL),
        };
        self.last = Some(now);
        let video_group = group(device, &self.layout, &self.sampler, video);
        pass(encoder, &self.shrink, &video_group, &self.small, None);
        pass(encoder, &self.blur_x, &self.small_group, &self.blurred_x, None);
        pass(encoder, &self.blur_y, &self.blurred_x_group, glow, Some(share));
    }
}

fn target(device: &wgpu::Device, label: &str) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d { width: WIDTH, height: HEIGHT, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: FORMAT,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        })
        .create_view(&Default::default())
}

fn group(device: &wgpu::Device, layout: &wgpu::BindGroupLayout, sampler: &wgpu::Sampler, view: &wgpu::TextureView) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("ambient"),
        layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(view) },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(sampler) },
        ],
    })
}

/// One full-target pass; with `share`, blends into what the target holds by that much.
fn pass(
    encoder: &mut wgpu::CommandEncoder,
    pipeline: &wgpu::RenderPipeline,
    input: &wgpu::BindGroup,
    target: &wgpu::TextureView,
    share: Option<f64>,
) {
    let load = if share.is_some() { wgpu::LoadOp::Load } else { wgpu::LoadOp::Clear(wgpu::Color::BLACK) };
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("ambient"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: target,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations { load, store: wgpu::StoreOp::Store },
        })],
        ..Default::default()
    });
    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, input, &[]);
    if let Some(share) = share {
        pass.set_blend_constant(wgpu::Color { r: share, g: share, b: share, a: share });
    }
    pass.draw(0..3, 0..1);
}
