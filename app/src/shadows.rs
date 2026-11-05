//! Raised surfaces with their shadow as one image (research R8), after the stack spike's
//! `tools/make_shadow.py`.
//!
//! Slint's software renderer does not draw `drop-shadow-*`, so a raised surface is drawn as an
//! image. Each image is made for one surface size at the window's scale, in physical
//! pixels, so Slint draws it 1:1. Its nearest-pixel scaling would otherwise make it blocky and move
//! the cut-out off the surface edge.
//!
//! The image is the surface grown by [`MARGIN`] on every side and [`BELOW`] more at the bottom. It
//! holds two layers, a wide soft one and a tight one for the contact edge. Each layer is the
//! rounded rectangle moved down (the light comes from above) and blurred; the Gaussian is
//! approximated by three box blurs. The surface's fill is drawn in the same image with the same
//! coverage that cuts the shadow out under it. Its edge and the shadow then always meet: no gap,
//! no dark outline, no teeth where two anti-aliasings disagree.

use slint::{Rgba8Pixel, SharedPixelBuffer};

/// How far the image reaches past the surface on every side, in logical pixels: 3 sigma of the
/// soft layer. `raised.slint` places the image by it.
pub const MARGIN: f32 = 36.0;
/// How much further the image reaches below, in logical pixels: the soft layer's offset.
pub const BELOW: f32 = 10.0;
/// Corner radius of the surfaces (`Theme.radius-m`), in logical pixels.
pub const RADIUS: f32 = 16.0;

/// One shadow layer, in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Layer {
    pub sigma: f32,
    /// How far the shadow is moved down.
    pub offset: f32,
    pub opacity: f32,
}

/// A wide soft layer for elevation.
pub const SOFT: Layer = Layer {
    sigma: 12.0,
    offset: 10.0,
    opacity: 0.45,
};

/// A tight layer for the contact edge.
pub const TIGHT: Layer = Layer {
    sigma: 3.0,
    offset: 2.0,
    opacity: 0.35,
};

/// The size in physical pixels of the image for a surface of `width` Ã— `height` logical pixels.
pub fn image_size(width: f32, height: f32, scale: f32) -> (u32, u32) {
    let margin = px(MARGIN, scale);
    (
        px(width, scale) + 2 * margin,
        px(height, scale) + 2 * margin + px(BELOW, scale),
    )
}

/// The image of a surface of `width` Ã— `height` logical pixels at `scale`: its `fill` (straight
/// alpha) over its shadow at `strength` (the theme's shadow opacity), straight alpha.
pub fn make(width: f32, height: f32, scale: f32, fill: Rgba8Pixel, strength: f32) -> SharedPixelBuffer<Rgba8Pixel> {
    let (w, h) = image_size(width, height, scale);
    let margin = px(MARGIN, scale) as f32;
    let surface = RoundedRect {
        left: margin,
        top: margin,
        right: margin + px(width, scale) as f32,
        bottom: margin + px(height, scale) as f32,
        radius: RADIUS * scale,
    };
    let (wu, hu) = (w as usize, h as usize);

    // What each pixel lets through: 1 where no layer darkens it.
    let mut clear = vec![1f32; wu * hu];
    for layer in [SOFT, TIGHT] {
        let offset = layer.offset * scale;
        let shape = RoundedRect {
            top: surface.top + offset,
            bottom: surface.bottom + offset,
            ..surface
        };
        let mut mask = vec![0f32; wu * hu];
        for y in 0..hu {
            for x in 0..wu {
                mask[y * wu + x] = layer.opacity * shape.coverage(x, y);
            }
        }
        let mask = blur(&mask, wu, hu, layer.sigma * scale);
        for (c, a) in clear.iter_mut().zip(mask) {
            *c *= 1.0 - a;
        }
    }

    let to_u8 = |v: f32| (v * 255.0).round().clamp(0.0, 255.0) as u8;
    let fill_alpha = f32::from(fill.a) / 255.0;
    let mut pixels = SharedPixelBuffer::<Rgba8Pixel>::new(w, h);
    for (i, pixel) in pixels.make_mut_slice().iter_mut().enumerate() {
        // One coverage for the fill and the cut-out, so they always meet.
        let covered = surface.coverage(i % wu, i / wu);
        let shadow = (1.0 - clear[i]) * strength * (1.0 - covered);
        let surface_alpha = fill_alpha * covered;
        // The fill over the black shadow.
        let alpha = surface_alpha + shadow * (1.0 - surface_alpha);
        *pixel = if alpha <= 0.0 {
            Rgba8Pixel::new(0, 0, 0, 0)
        } else {
            let k = surface_alpha / alpha;
            let channel = |c: u8| (f32::from(c) * k).round() as u8;
            Rgba8Pixel::new(channel(fill.r), channel(fill.g), channel(fill.b), to_u8(alpha))
        };
    }
    pixels
}

/// Logical pixels to whole physical pixels.
fn px(logical: f32, scale: f32) -> u32 {
    (logical * scale).round().max(0.0) as u32
}

/// A rounded rectangle; `right` and `bottom` are exclusive.
#[derive(Clone, Copy)]
struct RoundedRect {
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
    radius: f32,
}

impl RoundedRect {
    /// How much of pixel `(x, y)` the shape covers, 0 to 1, from its signed distance at the
    /// pixel's centre.
    fn coverage(&self, x: usize, y: usize) -> f32 {
        let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
        let (cx, cy) = ((self.left + self.right) / 2.0, (self.top + self.bottom) / 2.0);
        let radius = self
            .radius
            .min((self.right - self.left) / 2.0)
            .min((self.bottom - self.top) / 2.0);
        let qx = (px - cx).abs() - ((self.right - self.left) / 2.0 - radius);
        let qy = (py - cy).abs() - ((self.bottom - self.top) / 2.0 - radius);
        let outside = qx.max(0.0).hypot(qy.max(0.0));
        let distance = outside + qx.max(qy).min(0.0) - radius;
        (0.5 - distance).clamp(0.0, 1.0)
    }
}

/// A Gaussian blur approximated by three box blurs per direction; outside the image counts as
/// transparent.
fn blur(source: &[f32], w: usize, h: usize, sigma: f32) -> Vec<f32> {
    let mut image = source.to_vec();
    for radius in box_radii(sigma) {
        image = box_pass(&image, w, h, radius, true);
        image = box_pass(&image, w, h, radius, false);
    }
    image
}

/// The radii of three box blurs whose sum approximates a Gaussian of `sigma`.
fn box_radii(sigma: f32) -> [usize; 3] {
    const N: f32 = 3.0;
    let ideal = (12.0 * sigma * sigma / N + 1.0).sqrt();
    let mut lower = ideal.floor() as i32;
    if lower % 2 == 0 {
        lower -= 1;
    }
    let lower = lower.max(1);
    let upper = lower + 2;
    let (l, n) = (lower as f32, N);
    let lower_count = ((12.0 * sigma * sigma - n * l * l - 4.0 * n * l - 3.0 * n) / (-4.0 * l - 4.0)).round() as i32;
    std::array::from_fn(|i| {
        let width = if (i as i32) < lower_count { lower } else { upper };
        (width as usize - 1) / 2
    })
}

/// One box blur of `radius` along rows (`horizontal`) or columns, from running sums.
fn box_pass(source: &[f32], w: usize, h: usize, radius: usize, horizontal: bool) -> Vec<f32> {
    let (len, lines) = if horizontal { (w, h) } else { (h, w) };
    let index = |line: usize, i: usize| if horizontal { line * w + i } else { i * w + line };
    let width = (2 * radius + 1) as f32;
    let mut out = vec![0f32; source.len()];
    let mut sums = vec![0f32; len + 1];
    for line in 0..lines {
        for i in 0..len {
            sums[i + 1] = sums[i] + source[index(line, i)];
        }
        for i in 0..len {
            let from = i.saturating_sub(radius);
            let to = (i + radius + 1).min(len);
            out[index(line, i)] = (sums[to] - sums[from]) / width;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pixel(image: &SharedPixelBuffer<Rgba8Pixel>, x: u32, y: u32) -> Rgba8Pixel {
        image.as_slice()[(y * image.width() + x) as usize]
    }

    fn alpha(image: &SharedPixelBuffer<Rgba8Pixel>, x: u32, y: u32) -> u8 {
        pixel(image, x, y).a
    }

    /// The shadow alone: a transparent fill at full strength.
    fn shadow(width: f32, height: f32, scale: f32) -> SharedPixelBuffer<Rgba8Pixel> {
        make(width, height, scale, Rgba8Pixel::new(0, 0, 0, 0), 1.0)
    }

    #[test]
    fn the_image_is_the_surface_plus_the_margins_in_physical_pixels() {
        assert_eq!(image_size(56.0, 56.0, 1.0), (128, 138));
        assert_eq!(image_size(56.0, 56.0, 1.5), (192, 207));
        assert_eq!(image_size(56.0, 56.0, 1.25), (160, 173));
        let image = shadow(56.0, 56.0, 1.25);
        assert_eq!((image.width(), image.height()), (160, 173));
    }

    #[test]
    fn left_and_right_are_mirrored() {
        for scale in [1.0, 1.25, 1.5, 2.0] {
            let image = make(56.0, 40.0, scale, Rgba8Pixel::new(40, 50, 60, 200), 1.0);
            let (w, h) = (image.width(), image.height());
            for y in 0..h {
                for x in 0..w / 2 {
                    let (a, b) = (pixel(&image, x, y), pixel(&image, w - 1 - x, y));
                    assert!(a.a.abs_diff(b.a) <= 1, "{scale}: ({x}, {y}): {a:?} vs {b:?}");
                    assert!(a.r.abs_diff(b.r) <= 1, "{scale}: ({x}, {y}): {a:?} vs {b:?}");
                }
            }
        }
    }

    #[test]
    fn the_shadow_is_cut_out_under_the_surface() {
        for scale in [1.0, 1.25, 1.5, 1.75, 2.0] {
            let image = shadow(56.0, 56.0, scale);
            let margin = px(MARGIN, scale);
            let side = px(56.0, scale);
            let y = margin + side / 2;
            // Just outside the surface's left and right edges: shadow.
            assert!(alpha(&image, margin - 1, y) > 20, "{scale}: left outside");
            assert!(alpha(&image, margin + side, y) > 20, "{scale}: right outside");
            // The surface's own edge pixels and everything inside: clear.
            assert_eq!(alpha(&image, margin, y), 0, "{scale}: left edge");
            assert_eq!(alpha(&image, margin + side - 1, y), 0, "{scale}: right edge");
            assert_eq!(alpha(&image, margin + side / 2, y), 0, "{scale}: centre");
        }
    }

    #[test]
    fn the_fill_covers_the_surface_with_its_own_colour() {
        let fill = Rgba8Pixel::new(14, 15, 19, 184);
        let image = make(56.0, 56.0, 1.5, fill, 1.0);
        let margin = px(MARGIN, 1.5);
        let side = px(56.0, 1.5);
        let y = margin + side / 2;
        assert_eq!(pixel(&image, margin + side / 2, y), fill);
        assert_eq!(pixel(&image, margin, y), fill);
        // Outside: the black shadow only.
        let outside = pixel(&image, margin - 1, y);
        assert_eq!((outside.r, outside.g, outside.b), (0, 0, 0));
        assert!(outside.a > 20);
    }

    #[test]
    fn the_strength_scales_the_shadow_only() {
        let fill = Rgba8Pixel::new(30, 31, 37, 255);
        let (full, soft) = (make(56.0, 56.0, 1.0, fill, 1.0), make(56.0, 56.0, 1.0, fill, 0.35));
        let margin = px(MARGIN, 1.0);
        let below = margin + 56 + 4;
        let (a, b) = (alpha(&full, 64, below), alpha(&soft, 64, below));
        assert!((f32::from(b) - f32::from(a) * 0.35).abs() <= 1.0, "{a} vs {b}");
        assert_eq!(pixel(&soft, 64, margin + 28), fill);
    }

    #[test]
    fn the_rim_fades_out() {
        for scale in [1.0, 1.5] {
            let image = shadow(80.0, 40.0, scale);
            let (w, h) = (image.width(), image.height());
            for x in 0..w {
                assert!(alpha(&image, x, 0) <= 3, "{scale}: top row at {x}");
                assert!(alpha(&image, x, h - 1) <= 3, "{scale}: bottom row at {x}");
            }
            for y in 0..h {
                assert!(alpha(&image, 0, y) <= 3, "{scale}: left column at {y}");
            }
        }
    }

    #[test]
    fn the_shadow_is_stronger_below_than_above() {
        let image = shadow(56.0, 56.0, 1.0);
        let margin = px(MARGIN, 1.0);
        let x = image.width() / 2;
        let above = alpha(&image, x, margin - 3);
        let below = alpha(&image, x, margin + 56 + 2);
        assert!(below > above, "below {below}, above {above}");
    }

    #[test]
    fn three_boxes_approximate_the_gaussian() {
        for sigma in [3.0, 12.0, 18.0] {
            let radii = box_radii(sigma);
            // The variance of a box of width 2r + 1 is ((2r + 1)^2 - 1) / 12; they add up.
            let variance: f32 = radii.iter().map(|&r| (((2 * r + 1).pow(2) - 1) as f32) / 12.0).sum();
            assert!((variance.sqrt() - sigma).abs() < 0.6, "sigma {sigma}: {radii:?}");
        }
    }

    #[test]
    fn the_blur_keeps_the_total_away_from_the_edges() {
        let (w, h) = (101, 101);
        let mut source = vec![0f32; w * h];
        source[50 * w + 50] = 1.0;
        let total: f32 = blur(&source, w, h, 6.0).iter().sum();
        assert!((total - 1.0).abs() < 1e-3, "total {total}");
    }
}
