//! Nine-slice shadow images (research R8), a port of the stack spike's `tools/make_shadow.py`.
//!
//! Slint's software renderer does not draw `drop-shadow-*`, so raised surfaces put these images
//! under them, stretched as nine-slice. Each image is a black rounded rectangle blurred with a
//! Gaussian; the margin around it is 3 sigma, so the shadow fades out inside the image. The shape
//! itself is cut out: the renderer applies opacity to each element on its own, so under a
//! semi-transparent surface a filled shadow would show through and darken it.
//!
//! The light comes from above, so the shadow is moved down inside the image while the hole stays
//! where the surface is. In Slint the shadow element is the surface grown by the margin on every
//! side, plus the offset at the bottom.
//!
//! The images are made at 1×: Slint draws a nine-slice corner at its edge size times the window's
//! scale, so it scales them itself.

use slint::{Image, Rgba8Pixel, SharedPixelBuffer};

/// One shadow layer, in logical pixels. The image reaches 3 sigma past the surface on every side,
/// plus the offset at the bottom.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Preset {
    /// Corner radius of the surface the shadow is under.
    pub radius: u32,
    pub sigma: u32,
    /// How far the shadow is moved down.
    pub offset: u32,
    pub opacity: f32,
}

/// A wide soft layer for elevation; with [`TIGHT`] it makes the layered shadow.
pub const SOFT: Preset = Preset {
    radius: 16,
    sigma: 12,
    offset: 10,
    opacity: 0.45,
};

/// A tight layer for the contact edge.
pub const TIGHT: Preset = Preset {
    radius: 16,
    sigma: 3,
    offset: 2,
    opacity: 0.35,
};

/// A generated shadow image.
pub struct Shadow {
    /// Black with the shadow in the alpha channel (straight alpha).
    pub pixels: SharedPixelBuffer<Rgba8Pixel>,
    /// Nine-slice edges in image pixels: top, right, bottom, left.
    pub edges: [u16; 4],
}

impl Shadow {
    /// The image for Slint, with its nine-slice edges.
    pub fn image(&self) -> Image {
        let mut image = Image::from_rgba8(self.pixels.clone());
        let [top, right, bottom, left] = self.edges;
        image.set_nine_slice_edges(top, right, bottom, left);
        image
    }
}

/// Makes the shadow image of `preset`.
pub fn make(preset: Preset) -> Shadow {
    let Preset {
        radius,
        sigma,
        offset,
        opacity,
    } = preset;
    let margin = 3 * sigma;
    let border = margin + radius;
    // Straight edge between the corners, so the stretched middle slices are uniform.
    let width = 2 * border + 8;
    let height = width + offset;
    let (w, h) = (width as usize, height as usize);

    let hole = RoundedRect {
        left: margin as f32,
        top: margin as f32,
        right: (width - margin) as f32,
        bottom: (height - margin - offset) as f32,
        radius: radius as f32,
    };
    let shape = RoundedRect {
        top: hole.top + offset as f32,
        bottom: hole.bottom + offset as f32,
        ..hole
    };

    let mut mask = vec![0f32; w * h];
    for y in 0..h {
        for x in 0..w {
            mask[y * w + x] = opacity * shape.coverage(x, y);
        }
    }
    let mask = blur(&mask, w, h, sigma as f32);

    let mut pixels = SharedPixelBuffer::<Rgba8Pixel>::new(width, height);
    for (i, pixel) in pixels.make_mut_slice().iter_mut().enumerate() {
        let alpha = mask[i] * (1.0 - hole.coverage(i % w, i / w));
        *pixel = Rgba8Pixel::new(0, 0, 0, (alpha * 255.0).round().clamp(0.0, 255.0) as u8);
    }
    let border = border as u16;
    Shadow {
        pixels,
        edges: [border, border, border + offset as u16, border],
    }
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
        let qx = (px - cx).abs() - ((self.right - self.left) / 2.0 - self.radius);
        let qy = (py - cy).abs() - ((self.bottom - self.top) / 2.0 - self.radius);
        let outside = qx.max(0.0).hypot(qy.max(0.0));
        let distance = outside + qx.max(qy).min(0.0) - self.radius;
        (0.5 - distance).clamp(0.0, 1.0)
    }
}

/// Separable Gaussian blur; outside the image counts as transparent.
fn blur(source: &[f32], w: usize, h: usize, sigma: f32) -> Vec<f32> {
    let reach = (3.0 * sigma).ceil() as isize;
    let mut kernel: Vec<f32> = (-reach..=reach)
        .map(|i| (-((i * i) as f32) / (2.0 * sigma * sigma)).exp())
        .collect();
    let sum: f32 = kernel.iter().sum();
    kernel.iter_mut().for_each(|k| *k /= sum);

    let pass = |source: &[f32], step: usize, len: usize, lines: usize, line_step: usize| {
        let mut out = vec![0f32; source.len()];
        for line in 0..lines {
            for i in 0..len {
                let mut total = 0.0;
                for (k, weight) in kernel.iter().enumerate() {
                    let j = i as isize + k as isize - reach;
                    if (0..len as isize).contains(&j) {
                        total += weight * source[line * line_step + j as usize * step];
                    }
                }
                out[line * line_step + i * step] = total;
            }
        }
        out
    };
    let rows = pass(source, 1, w, h, w);
    pass(&rows, w, h, w, 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn alpha(shadow: &Shadow, x: u32, y: u32) -> u8 {
        shadow.pixels.as_slice()[(y * shadow.pixels.width() + x) as usize].a
    }

    #[test]
    fn the_spike_sizes_edges_and_margins() {
        // The spike's soft.png and tight.png: nine-slice(52 52 62 52) and (25 25 27 25).
        let soft = make(SOFT);
        assert_eq!((soft.pixels.width(), soft.pixels.height()), (112, 122));
        assert_eq!(soft.edges, [52, 52, 62, 52]);
        let tight = make(TIGHT);
        assert_eq!((tight.pixels.width(), tight.pixels.height()), (58, 60));
        assert_eq!(tight.edges, [25, 25, 27, 25]);
    }

    #[test]
    fn the_edges_leave_a_middle_to_stretch() {
        for preset in [SOFT, TIGHT] {
            let shadow = make(preset);
            let [top, right, bottom, left] = shadow.edges.map(u32::from);
            assert_eq!(shadow.pixels.width() - left - right, 8);
            assert_eq!(shadow.pixels.height() - top - bottom, 8);
        }
    }

    #[test]
    fn left_and_right_are_mirrored() {
        for preset in [SOFT, TIGHT] {
            let shadow = make(preset);
            let (w, h) = (shadow.pixels.width(), shadow.pixels.height());
            for y in 0..h {
                for x in 0..w / 2 {
                    let (a, b) = (alpha(&shadow, x, y), alpha(&shadow, w - 1 - x, y));
                    assert!(a.abs_diff(b) <= 1, "({x}, {y}): {a} vs {b}");
                }
            }
        }
    }

    #[test]
    fn the_centre_is_transparent_and_the_rim_fades_out() {
        for preset in [SOFT, TIGHT] {
            let shadow = make(preset);
            let margin = 3 * preset.sigma;
            let (w, h) = (shadow.pixels.width(), shadow.pixels.height());
            // Inside the hole.
            assert_eq!(alpha(&shadow, w / 2, h / 2), 0);
            assert_eq!(alpha(&shadow, margin + preset.radius, margin + preset.radius), 0);
            // The outer rim: 3 sigma past a shape blurred with that sigma.
            for x in 0..w {
                assert!(alpha(&shadow, x, 0) <= 2, "top row at {x}");
                assert!(alpha(&shadow, x, h - 1) <= 2, "bottom row at {x}");
            }
            assert!(shadow.pixels.as_slice().iter().all(|p| (p.r, p.g, p.b) == (0, 0, 0)));
        }
    }

    #[test]
    fn the_shadow_is_stronger_below_than_above() {
        for preset in [SOFT, TIGHT] {
            let shadow = make(preset);
            let margin = 3 * preset.sigma;
            let (w, h) = (shadow.pixels.width(), shadow.pixels.height());
            let x = w / 2;
            // Just outside the hole, above and below it.
            let above = alpha(&shadow, x, margin - 2);
            let below = alpha(&shadow, x, h - margin - preset.offset + 1);
            assert!(below > above, "{preset:?}: below {below}, above {above}");
        }
    }

    #[test]
    fn the_image_carries_the_nine_slice_size() {
        let shadow = make(TIGHT);
        let image = shadow.image();
        assert_eq!((image.size().width, image.size().height), (58, 60));
    }
}
