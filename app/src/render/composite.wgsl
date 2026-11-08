// Draws one textured rectangle; used for each layer per present: the video (letterboxed, corners
// rounded) then the UI. The UI texture holds premultiplied RGBA, blended with (One,
// OneMinusSrcAlpha). The video arrives in the device's own format and is converted to RGB here
// (device preview research R4): `to_r`, `to_g`, `to_b` hold the matrix and the range, so
// `rgb = rows · (y, u, v, 1)`; `yuv_to_rgb` in gpu.rs mirrors it for the tests.

struct Rect {
    // Left, top, right, bottom in clip space.
    bounds: vec4<f32>,
    // Size in pixels and corner radius in pixels (0: square corners).
    size: vec2<f32>,
    radius: f32,
    opacity: f32,
    // Layer kind, then the picture's width and height in pixels. Kinds: 0 premultiplied RGBA
    // (UI), 1 opaque RGB, 2 NV12, 3 I420, 4 YUY2, 5 UYVY, 6 coverage mask tinted with `to_r`'s
    // RGB (the watermark), 7 the video's shadow (`to_r`: margin, then strength, spread, blur of
    // the near shadow; `to_g`: strength, spread, blur of the far one, in pixels).
    picture: vec4<f32>,
    to_r: vec4<f32>,
    to_g: vec4<f32>,
    to_b: vec4<f32>,
};

@group(0) @binding(0) var tex0: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;
@group(0) @binding(2) var<uniform> rect: Rect;
@group(0) @binding(3) var tex1: texture_2d<f32>;
@group(0) @binding(4) var tex2: texture_2d<f32>;

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

fn convert(yuv: vec3<f32>) -> vec4<f32> {
    let p = vec4<f32>(yuv, 1.0);
    let rgb = vec3<f32>(dot(rect.to_r, p), dot(rect.to_g, p), dot(rect.to_b, p));
    return vec4<f32>(clamp(rgb, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
}

// One pixel of a packed 4:2:2 picture: each texel holds two pixels, Y0 U Y1 V (YUY2) or U Y0 V Y1
// (UYVY).
fn packed_pixel(p: vec2<i32>, uyvy: bool) -> vec3<f32> {
    let last = vec2<i32>(i32(rect.picture.y) - 1, i32(rect.picture.z) - 1);
    let q = clamp(p, vec2<i32>(0), last);
    let t = textureLoad(tex0, vec2<i32>(q.x / 2, q.y), 0);
    let odd = (q.x & 1) == 1;
    if uyvy {
        return vec3<f32>(select(t.g, t.a, odd), t.r, t.b);
    }
    return vec3<f32>(select(t.r, t.b, odd), t.g, t.a);
}

// Packed pixels filtered by hand (the sampler would blend neighbouring pairs).
fn packed(uv: vec2<f32>, uyvy: bool) -> vec3<f32> {
    let pos = uv * rect.picture.yz - vec2<f32>(0.5);
    let base = floor(pos);
    let f = pos - base;
    let p = vec2<i32>(base);
    let top = mix(packed_pixel(p, uyvy), packed_pixel(p + vec2<i32>(1, 0), uyvy), f.x);
    let bottom = mix(packed_pixel(p + vec2<i32>(0, 1), uyvy), packed_pixel(p + vec2<i32>(1, 1), uyvy), f.x);
    return mix(top, bottom, f.y);
}

// One soft shadow at `d` pixels outside the video: full strength within `spread`, then a Gaussian
// fall-off of `blur` / 2 deviation, as a CSS box shadow.
fn soft(d: f32, strength: f32, spread: f32, blur: f32) -> f32 {
    let deviation = max(blur * 0.5, 0.001);
    let x = max(d - spread, 0.0);
    return strength * exp(-x * x / (2.0 * deviation * deviation));
}

// The video's shadow: the distance to the video's rounded rectangle, which sits `margin` pixels
// inside this layer's rectangle; nothing under the video itself.
fn shadow(uv: vec2<f32>) -> vec4<f32> {
    let margin = rect.to_r.x;
    let half_size = rect.size * 0.5;
    let inner = half_size - vec2<f32>(margin);
    let q = abs(uv * rect.size - half_size) - (inner - vec2<f32>(rect.radius));
    let d = length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - rect.radius;
    let a = soft(d, rect.to_r.y, rect.to_r.z, rect.to_r.w) + soft(d, rect.to_g.x, rect.to_g.y, rect.to_g.z);
    return vec4<f32>(0.0, 0.0, 0.0, min(a, 1.0) * clamp(d + 0.5, 0.0, 1.0));
}

@fragment
fn fs(in: VertexOut) -> @location(0) vec4<f32> {
    // Every plane is sampled before branching, so the samples stay in uniform control flow.
    let s0 = textureSample(tex0, samp, in.uv);
    let s1 = textureSample(tex1, samp, in.uv);
    let s2 = textureSample(tex2, samp, in.uv);
    let kind = u32(rect.picture.x + 0.5);
    var color: vec4<f32>;
    switch kind {
        case 0u: {
            color = s0;
        }
        case 1u: {
            color = vec4<f32>(s0.rgb, 1.0);
        }
        case 2u: {
            color = convert(vec3<f32>(s0.r, s1.r, s1.g));
        }
        case 3u: {
            color = convert(vec3<f32>(s0.r, s1.r, s2.r));
        }
        case 6u: {
            color = vec4<f32>(rect.to_r.rgb * s0.r, s0.r);
        }
        case 7u: {
            color = shadow(in.uv);
        }
        default: {
            color = convert(packed(in.uv, kind == 5u));
        }
    }
    // Signed distance to the rounded rectangle, in pixels; one pixel of anti-aliased edge.
    let half_size = rect.size * 0.5;
    let q = abs(in.uv * rect.size - half_size) - (half_size - vec2<f32>(rect.radius));
    let d = length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - rect.radius;
    return color * (clamp(0.5 - d, 0.0, 1.0) * rect.opacity);
}
