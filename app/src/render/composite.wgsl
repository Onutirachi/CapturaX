// Draws one textured rectangle; used for each layer per present: the video (letterboxed, corners
// rounded) then the UI. The UI texture holds premultiplied RGBA, blended with (One,
// OneMinusSrcAlpha). The video arrives in the device's own format and is converted to RGB here
// (device preview research R4): `to_r`, `to_g`, `to_b` hold the matrix and the range, so
// `rgb = rows · (y, u, v, 1)`; `conversion` in crates/capture/src/still.rs makes the rows and
// mirrors this for still images.

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
    // The screenshot flash on video layers: opacity, the line's width and the inner glow's
    // deviation in pixels.
    flash: vec4<f32>,
};

// The glow's passes (glow.rs, preview extras research R1) use the same uniform: their bounds cover
// the whole target and `size` is the target's size. `glow_convert` reads the video's planes with
// the video's kind, picture size and rows; `glow_ease` takes its factor in `to_r.x`; `glow_blur`
// takes its direction in `to_r.xy` and its deviation in texels in `to_r.z`.

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
    // The screenshot flash (screenshot FR-010a, research R11): a white line along the video's
    // rounded edge with a faint glow just inside it.
    if kind >= 1u && kind <= 5u && rect.flash.x > 0.0 {
        let inside = max(-d, 0.0);
        let line = clamp(rect.flash.y - inside + 0.5, 0.0, 1.0);
        let deviation = max(rect.flash.z, 0.001);
        let glow = 0.35 * exp(-inside * inside / (2.0 * deviation * deviation));
        color = vec4<f32>(mix(color.rgb, vec3<f32>(1.0), rect.flash.x * max(line, glow)), color.a);
    }
    return color * (clamp(0.5 - d, 0.0, 1.0) * rect.opacity);
}

// The video's RGB at `uv`, any of the 5 formats. Explicit level samples, so it may branch.
fn video_rgb(uv: vec2<f32>) -> vec3<f32> {
    let kind = u32(rect.picture.x + 0.5);
    switch kind {
        case 1u: {
            return textureSampleLevel(tex0, samp, uv, 0.0).rgb;
        }
        case 2u: {
            let y = textureSampleLevel(tex0, samp, uv, 0.0).r;
            let uv_plane = textureSampleLevel(tex1, samp, uv, 0.0);
            return convert(vec3<f32>(y, uv_plane.r, uv_plane.g)).rgb;
        }
        case 3u: {
            let y = textureSampleLevel(tex0, samp, uv, 0.0).r;
            let u = textureSampleLevel(tex1, samp, uv, 0.0).r;
            let v = textureSampleLevel(tex2, samp, uv, 0.0).r;
            return convert(vec3<f32>(y, u, v)).rgb;
        }
        default: {
            return convert(packed(uv, kind == 5u)).rgb;
        }
    }
}

// The glow's convert: the video into the first work target, 4 bilinear taps per texel (a box of
// about 4 × 4 source pixels at 1080p).
@fragment
fn glow_convert(in: VertexOut) -> @location(0) vec4<f32> {
    let d = vec2<f32>(0.25) / rect.size;
    let sum = video_rgb(in.uv + vec2<f32>(-d.x, -d.y)) + video_rgb(in.uv + vec2<f32>(d.x, -d.y))
        + video_rgb(in.uv + vec2<f32>(-d.x, d.y)) + video_rgb(in.uv + vec2<f32>(d.x, d.y));
    return vec4<f32>(sum * 0.25, 1.0);
}

// A 2× box: the texel center falls on the corner of 4 source texels, so one bilinear tap is their
// average.
@fragment
fn glow_box(in: VertexOut) -> @location(0) vec4<f32> {
    return textureSampleLevel(tex0, samp, in.uv, 0.0);
}

// The easing: the history (`tex1`) moves towards the field (`tex0`) by the factor.
@fragment
fn glow_ease(in: VertexOut) -> @location(0) vec4<f32> {
    let p = vec2<i32>(in.position.xy);
    return mix(textureLoad(tex1, p, 0), textureLoad(tex0, p, 0), rect.to_r.x);
}

// One direction of the separable Gaussian blur, edges clamped.
@fragment
fn glow_blur(in: VertexOut) -> @location(0) vec4<f32> {
    let p = vec2<i32>(in.position.xy);
    let dir = vec2<i32>(rect.to_r.xy);
    let deviation = max(rect.to_r.z, 0.001);
    let radius = min(i32(ceil(deviation * 3.0)), 16);
    let last = vec2<i32>(textureDimensions(tex0)) - vec2<i32>(1);
    var sum = vec4<f32>(0.0);
    var total = 0.0;
    for (var i = -radius; i <= radius; i++) {
        let w = exp(-f32(i * i) / (2.0 * deviation * deviation));
        sum += w * textureLoad(tex0, clamp(p + dir * i, vec2<i32>(0), last), 0);
        total += w;
    }
    return sum / total;
}

// The history's average luminance (Rec. 709 weights) into the 1 × 1 target, for the gate.
@fragment
fn glow_average(in: VertexOut) -> @location(0) vec4<f32> {
    let size = vec2<i32>(textureDimensions(tex0));
    var sum = 0.0;
    for (var y = 0; y < size.y; y++) {
        for (var x = 0; x < size.x; x++) {
            sum += dot(textureLoad(tex0, vec2<i32>(x, y), 0).rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
        }
    }
    let l = sum / f32(size.x * size.y);
    return vec4<f32>(l, l, l, 1.0);
}
