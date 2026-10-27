// Draws one textured rectangle; used twice per frame: the video, then the UI layer.
// Textures hold premultiplied RGBA, blended with (One, OneMinusSrcAlpha).

struct Rect {
    // Left, top, right, bottom in clip space.
    bounds: vec4<f32>,
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
    return textureSample(tex, samp, in.uv);
}
