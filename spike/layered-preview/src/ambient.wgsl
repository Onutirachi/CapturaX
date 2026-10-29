// Ambient light (T034): the video shrunk to a small texture, then blurred in two passes. The last
// pass blends into the previous glow (blend constant set by ambient.rs), so the colors change
// smoothly.

@group(0) @binding(0) var tex: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;

// Colors 40% more saturated, as the previous app.
const SATURATION: f32 = 1.4;
// Blur in texels of the small texture: the previous app's blur(6px) on its 96 × 54 canvas.
const SIGMA: f32 = 6.0;
const RADIUS: i32 = 12;

struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs(@builtin(vertex_index) index: u32) -> VertexOut {
    // One triangle covering the target: (0,0) (2,0) (0,2) in UV space.
    let uv = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    var out: VertexOut;
    out.position = vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, 0.0, 1.0);
    out.uv = uv;
    return out;
}

@fragment
fn fs_shrink(in: VertexOut) -> @location(0) vec4<f32> {
    // 4 × 4 taps spread over the video area this texel covers, each a bilinear average of 2 × 2
    // pixels, so small details do not flicker in and out.
    let footprint = vec2<f32>(dpdx(in.uv.x), dpdy(in.uv.y));
    var sum = vec3<f32>(0.0);
    for (var j = 0; j < 4; j++) {
        for (var i = 0; i < 4; i++) {
            let offset = (vec2<f32>(f32(i), f32(j)) + 0.5) / 4.0 - 0.5;
            sum += textureSampleLevel(tex, samp, in.uv + offset * footprint, 0.0).rgb;
        }
    }
    let color = sum / 16.0;
    let gray = dot(color, vec3<f32>(0.2126, 0.7152, 0.0722));
    return vec4<f32>(clamp(mix(vec3<f32>(gray), color, SATURATION), vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
}

// Gaussian blur along `direction`; the sampler clamps to the edge, so the borders keep their color.
fn blur(uv: vec2<f32>, direction: vec2<f32>) -> vec4<f32> {
    let step = direction / vec2<f32>(textureDimensions(tex));
    var sum = vec3<f32>(0.0);
    var total = 0.0;
    for (var i = -RADIUS; i <= RADIUS; i++) {
        let weight = exp(-f32(i * i) / (2.0 * SIGMA * SIGMA));
        sum += weight * textureSampleLevel(tex, samp, uv + f32(i) * step, 0.0).rgb;
        total += weight;
    }
    return vec4<f32>(sum / total, 1.0);
}

@fragment
fn fs_blur_x(in: VertexOut) -> @location(0) vec4<f32> {
    return blur(in.uv, vec2<f32>(1.0, 0.0));
}

@fragment
fn fs_blur_y(in: VertexOut) -> @location(0) vec4<f32> {
    return blur(in.uv, vec2<f32>(0.0, 1.0));
}
