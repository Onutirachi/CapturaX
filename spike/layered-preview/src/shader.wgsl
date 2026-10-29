// Draws one textured rectangle; used twice per frame: the video, then the UI layer.
// Textures hold premultiplied RGBA, blended with (One, OneMinusSrcAlpha).

struct Rect {
    // Left, top, right, bottom in clip space.
    bounds: vec4<f32>,
    // Size in pixels and corner radius in pixels (0: square corners).
    size: vec2<f32>,
    radius: f32,
    _pad: f32,
};

@group(0) @binding(0) var tex: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;
@group(0) @binding(2) var<uniform> rect: Rect;

struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs(@builtin(vertex_index) index: u32) -> VertexOut {
    // Triangle strip: (0,0) (1,0) (0,1) (1,1) in UV space.
    let uv = vec2<f32>(f32(index & 1u), f32(index >> 1u));
    let x = mix(rect.bounds.x, rect.bounds.z, uv.x);
    let y = mix(rect.bounds.y, rect.bounds.w, uv.y);
    var out: VertexOut;
    out.position = vec4<f32>(x, y, 0.0, 1.0);
    out.uv = uv;
    return out;
}

@fragment
fn fs(in: VertexOut) -> @location(0) vec4<f32> {
    let color = textureSample(tex, samp, in.uv);
    // Signed distance to the rounded rectangle, in pixels; one pixel of anti-aliased edge.
    let half_size = rect.size * 0.5;
    let q = abs(in.uv * rect.size - half_size) - (half_size - vec2<f32>(rect.radius));
    let d = length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - rect.radius;
    return color * clamp(0.5 - d, 0.0, 1.0);
}
