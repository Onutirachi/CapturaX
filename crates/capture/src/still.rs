//! Still images from frames (spec 20261121-134830-screenshot, research R2, data-model.md "Still
//! image"): a frame converted to 8-bit RGB with the same rows and the same chroma placement as the
//! preview's shader (app/src/render/composite.wgsl), so a screenshot matches what is on screen,
//! and encoded as PNG.

use crate::frame::{Frame, Matrix, PixelFormat, Range};

/// The rows that turn normalized `(y, u, v, 1)` into RGB for a matrix and range (device preview
/// research R4): `R = Y' + 2(1 − Kr)·Cr`, `B = Y' + 2(1 − Kb)·Cb`, `G = (Y' − Kr·R − Kb·B) / Kg`,
/// with `Y' = (Y − 16/255)·255/219` and `C = (C − 128/255)·255/224` in limited range, `Y' = Y` and
/// `C = C − 128/255` in full range. The shader and [`still`] both use them.
pub fn conversion(matrix: Matrix, range: Range) -> [[f32; 4]; 3] {
    let (kr, kb) = match matrix {
        Matrix::Bt601 => (0.299_f64, 0.114_f64),
        Matrix::Bt709 => (0.2126, 0.0722),
    };
    let kg = 1.0 - kr - kb;
    let (ys, yo, cs) = match range {
        Range::Limited => (255.0 / 219.0, -16.0 / 219.0, 255.0 / 224.0),
        Range::Full => (1.0, 0.0, 1.0),
    };
    let co = -128.0 / 255.0 * cs;
    let (rv, bu) = (2.0 * (1.0 - kr), 2.0 * (1.0 - kb));
    let (gu, gv) = (-kb * bu / kg, -kr * rv / kg);
    let row = |u: f64, v: f64| [ys as f32, (u * cs) as f32, (v * cs) as f32, (yo + (u + v) * co) as f32];
    [row(0.0, rv), row(gu, gv), row(bu, 0.0)]
}

/// A frame as 8-bit sRGB: rows of `width × 3` bytes, no padding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Still {
    pub width: u32,
    pub height: u32,
    pub rgb: Vec<u8>,
}

impl Still {
    /// The pixels as RGBA with alpha 255 (what the clipboard takes).
    pub fn rgba(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.rgb.len() / 3 * 4);
        for [r, g, b] in self.rgb.as_chunks::<3>().0 {
            out.extend_from_slice(&[*r, *g, *b, 255]);
        }
        out
    }

    /// The image as a PNG file: RGB 8-bit, sRGB chunk, fast compression with the adaptive filter
    /// (research R2).
    pub fn png(&self) -> Result<Vec<u8>, String> {
        let mut bytes = Vec::new();
        let mut encoder = png::Encoder::new(&mut bytes, self.width, self.height);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_compression(png::Compression::Fast);
        // After the compression, which sets its own filter.
        encoder.set_filter(png::Filter::Adaptive);
        encoder.set_source_srgb(png::SrgbRenderingIntent::Perceptual);
        let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
        writer.write_image_data(&self.rgb).map_err(|e| e.to_string())?;
        writer.finish().map_err(|e| e.to_string())?;
        Ok(bytes)
    }
}

/// The frame converted as the shader draws it at its own size: luma per pixel; NV12 and I420
/// chroma bilinear between the half-size samples at the positions the shader's sampler reads
/// (edges clamped); YUY2 and UYVY with each pixel's own luma and its pair's chroma (the shader's
/// hand filter at pixel centres); BGRx and RGBA copied. The planes follow
/// [`PixelFormat::plane_sizes`], as for the upload.
pub fn still(frame: &Frame) -> Still {
    let (w, h) = (frame.width as usize, frame.height as usize);
    let rows = conversion(frame.matrix, frame.range);
    let planes = frame.planes();
    let mut rgb = vec![0; w * h * 3];
    if rgb.is_empty() {
        return Still {
            width: frame.width,
            height: frame.height,
            rgb,
        };
    }
    match frame.format {
        PixelFormat::Nv12 => {
            let c = &planes[1];
            planar(
                &mut rgb,
                w,
                h,
                planes[0].bytes.as_slice(),
                planes[0].stride,
                &rows,
                |x, y| {
                    let i = y * c.stride + x * 2;
                    let b = c.bytes.as_slice();
                    (b[i], b[i + 1])
                },
            );
        }
        PixelFormat::I420 => {
            let (u, v) = (&planes[1], &planes[2]);
            planar(
                &mut rgb,
                w,
                h,
                planes[0].bytes.as_slice(),
                planes[0].stride,
                &rows,
                |x, y| {
                    (
                        u.bytes.as_slice()[y * u.stride + x],
                        v.bytes.as_slice()[y * v.stride + x],
                    )
                },
            );
        }
        PixelFormat::Yuy2 | PixelFormat::Uyvy => {
            // Byte offsets in a pair of Y0, Y1, U, V.
            let [y0, y1, u, v] = if frame.format == PixelFormat::Yuy2 {
                [0, 2, 1, 3]
            } else {
                [1, 3, 0, 2]
            };
            let plane = &planes[0];
            let bytes = plane.bytes.as_slice();
            for (y, out) in rgb.chunks_exact_mut(w * 3).enumerate() {
                let row = &bytes[y * plane.stride..];
                for (x, px) in out.as_chunks_mut::<3>().0.iter_mut().enumerate() {
                    let pair = &row[x / 2 * 4..x / 2 * 4 + 4];
                    let luma = if x % 2 == 0 { pair[y0] } else { pair[y1] };
                    *px = apply(&rows, unit(luma), unit(pair[u]), unit(pair[v]));
                }
            }
        }
        PixelFormat::Bgrx | PixelFormat::Rgba => {
            let [r, g, b] = if frame.format == PixelFormat::Bgrx {
                [2, 1, 0]
            } else {
                [0, 1, 2]
            };
            let plane = &planes[0];
            let bytes = plane.bytes.as_slice();
            for (y, out) in rgb.chunks_exact_mut(w * 3).enumerate() {
                let row = &bytes[y * plane.stride..];
                for (px, src) in out.as_chunks_mut::<3>().0.iter_mut().zip(row.as_chunks::<4>().0) {
                    *px = [src[r], src[g], src[b]];
                }
            }
        }
    }
    Still {
        width: frame.width,
        height: frame.height,
        rgb,
    }
}

/// A 4:2:0 picture: luma per pixel, chroma from `chroma(cx, cy)` (U, V) interpolated.
fn planar(
    rgb: &mut [u8],
    w: usize,
    h: usize,
    luma: &[u8],
    stride: usize,
    rows: &[[f32; 4]; 3],
    chroma: impl Fn(usize, usize) -> (u8, u8),
) {
    let columns = taps(w, w.div_ceil(2));
    let lines = taps(h, h.div_ceil(2));
    for (y, out) in rgb.chunks_exact_mut(w * 3).enumerate() {
        let ty = lines[y];
        for (x, px) in out.as_chunks_mut::<3>().0.iter_mut().enumerate() {
            let tx = columns[x];
            let (a, b, c, d) = (
                chroma(tx.a, ty.a),
                chroma(tx.b, ty.a),
                chroma(tx.a, ty.b),
                chroma(tx.b, ty.b),
            );
            let mix = |a: u8, b: u8, c: u8, d: u8| {
                let top = lerp(unit(a), unit(b), tx.f);
                let bottom = lerp(unit(c), unit(d), tx.f);
                lerp(top, bottom, ty.f)
            };
            let (u, v) = (mix(a.0, b.0, c.0, d.0), mix(a.1, b.1, c.1, d.1));
            *px = apply(rows, unit(luma[y * stride + x]), u, v);
        }
    }
}

/// The two half-size samples a pixel reads and the weight of the second.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Tap {
    a: usize,
    b: usize,
    f: f32,
}

/// For each of `full` pixels, the samples of a `half`-size plane that a linear sampler with
/// clamped edges reads at the pixel's centre: position `(x + 0.5)·half/full − 0.5`.
fn taps(full: usize, half: usize) -> Vec<Tap> {
    let last = half.saturating_sub(1) as f32;
    (0..full)
        .map(|x| {
            let pos = (x as f32 + 0.5) * half as f32 / full as f32 - 0.5;
            let base = pos.floor();
            Tap {
                a: base.clamp(0.0, last) as usize,
                b: (base + 1.0).clamp(0.0, last) as usize,
                f: pos - base,
            }
        })
        .collect()
}

fn unit(v: u8) -> f32 {
    f32::from(v) / 255.0
}

fn lerp(a: f32, b: f32, f: f32) -> f32 {
    a + (b - a) * f
}

/// `rows · (y, u, v, 1)`, clamped and rounded to 8 bits as the surface stores it.
fn apply(rows: &[[f32; 4]; 3], y: f32, u: f32, v: f32) -> [u8; 3] {
    rows.map(|r| {
        let c = r[0] * y + r[1] * u + r[2] * v + r[3];
        (c.clamp(0.0, 1.0) * 255.0).round() as u8
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::Plane;
    use Matrix::{Bt601, Bt709};
    use Range::{Full, Limited};
    use std::time::Instant;

    const FORMATS: [PixelFormat; 6] = [
        PixelFormat::Nv12,
        PixelFormat::I420,
        PixelFormat::Yuy2,
        PixelFormat::Uyvy,
        PixelFormat::Bgrx,
        PixelFormat::Rgba,
    ];

    /// What the shader gives for 8-bit `y`, `u`, `v`: RGB from 0 to 1, clamped. Mirrors
    /// composite.wgsl.
    fn yuv_to_rgb(y: u8, u: u8, v: u8, matrix: Matrix, range: Range) -> [f32; 3] {
        let p = [unit(y), unit(u), unit(v), 1.0];
        conversion(matrix, range).map(|row| row.iter().zip(p).map(|(a, b)| a * b).sum::<f32>().clamp(0.0, 1.0))
    }

    fn close(actual: [f32; 3], expected: [f32; 3]) {
        for (a, e) in actual.iter().zip(expected) {
            assert!((a - e).abs() < 0.01, "{actual:?} != {expected:?}");
        }
    }

    fn within_one(actual: &[u8], expected: [f32; 3], what: &str) {
        for (a, e) in actual.iter().zip(expected) {
            let e = e * 255.0;
            assert!(
                (f32::from(*a) - e).abs() <= 1.0,
                "{what}: {actual:?} != {expected:?}×255"
            );
        }
    }

    fn frame(
        width: u32,
        height: u32,
        format: PixelFormat,
        planes: Vec<Vec<u8>>,
        matrix: Matrix,
        range: Range,
    ) -> Frame {
        let sizes = format.plane_sizes(width, height);
        let planes = planes
            .into_iter()
            .zip(sizes)
            .map(|(bytes, (stride, rows))| {
                assert_eq!(bytes.len(), stride * rows, "{format:?} plane size");
                Plane {
                    bytes: bytes.into(),
                    stride,
                }
            })
            .collect();
        let now = Instant::now();
        Frame::new(width, height, format, planes, matrix, range, now, now, 1)
    }

    /// A frame of one color: `(y, u, v)` for the YUV formats, `(r, g, b)` for the RGB ones.
    fn flat(
        width: u32,
        height: u32,
        format: PixelFormat,
        (a, b, c): (u8, u8, u8),
        matrix: Matrix,
        range: Range,
    ) -> Frame {
        let sizes = format.plane_sizes(width, height);
        let fill = |i: usize, pattern: &[u8]| -> Vec<u8> {
            let (stride, rows) = sizes[i];
            pattern.iter().copied().cycle().take(stride * rows).collect()
        };
        let planes = match format {
            PixelFormat::Nv12 => vec![fill(0, &[a]), fill(1, &[b, c])],
            PixelFormat::I420 => vec![fill(0, &[a]), fill(1, &[b]), fill(2, &[c])],
            PixelFormat::Yuy2 => vec![fill(0, &[a, b, a, c])],
            PixelFormat::Uyvy => vec![fill(0, &[b, a, c, a])],
            PixelFormat::Bgrx => vec![fill(0, &[c, b, a, 0])],
            PixelFormat::Rgba => vec![fill(0, &[a, b, c, 0])],
        };
        frame(width, height, format, planes, matrix, range)
    }

    #[test]
    fn yuv_to_rgb_in_both_matrices_and_ranges() {
        for matrix in [Bt601, Bt709] {
            close(yuv_to_rgb(16, 128, 128, matrix, Limited), [0.0, 0.0, 0.0]);
            close(yuv_to_rgb(235, 128, 128, matrix, Limited), [1.0, 1.0, 1.0]);
            close(yuv_to_rgb(0, 128, 128, matrix, Full), [0.0, 0.0, 0.0]);
            close(yuv_to_rgb(255, 128, 128, matrix, Full), [1.0, 1.0, 1.0]);
            // Limited black read as full is grey, not black.
            assert!(yuv_to_rgb(16, 128, 128, matrix, Full)[0] > 0.05);
        }
        // Red, from the standards' reference values.
        close(yuv_to_rgb(63, 102, 240, Bt709, Limited), [1.0, 0.0, 0.0]);
        close(yuv_to_rgb(81, 90, 240, Bt601, Limited), [1.0, 0.0, 0.0]);
        close(yuv_to_rgb(54, 99, 255, Bt709, Full), [1.0, 0.0, 0.0]);
        close(yuv_to_rgb(76, 85, 255, Bt601, Full), [1.0, 0.0, 0.0]);
        // Green and blue in BT.709 limited.
        close(yuv_to_rgb(173, 42, 26, Bt709, Limited), [0.0, 1.0, 0.0]);
        close(yuv_to_rgb(32, 240, 118, Bt709, Limited), [0.0, 0.0, 1.0]);
    }

    #[test]
    fn flat_colors_match_the_shader_in_every_format() {
        let colors = [
            (16, 128, 128),
            (235, 128, 128),
            (63, 102, 240),
            (173, 42, 26),
            (120, 180, 70),
            (0, 0, 255),
            (255, 255, 0),
        ];
        for format in FORMATS {
            for matrix in [Bt601, Bt709] {
                for range in [Full, Limited] {
                    for color in colors {
                        let still = still(&flat(6, 4, format, color, matrix, range));
                        assert_eq!((still.width, still.height, still.rgb.len()), (6, 4, 6 * 4 * 3));
                        let expected = match format {
                            PixelFormat::Bgrx | PixelFormat::Rgba => [unit(color.0), unit(color.1), unit(color.2)],
                            _ => yuv_to_rgb(color.0, color.1, color.2, matrix, range),
                        };
                        for px in still.rgb.as_chunks::<3>().0 {
                            within_one(px, expected, &format!("{format:?} {matrix:?} {range:?} {color:?}"));
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn chroma_taps_follow_the_sampler() {
        // 4 pixels over 2 samples: −0.25, 0.25, 0.75, 1.25, clamped at the edges.
        let t = taps(4, 2);
        assert_eq!((t[0].a, t[0].b), (0, 0));
        assert_eq!((t[1].a, t[1].b, t[1].f), (0, 1, 0.25));
        assert_eq!((t[2].a, t[2].b, t[2].f), (0, 1, 0.75));
        assert_eq!((t[3].a, t[3].b), (1, 1));
        // Odd: 3 pixels over 2 samples: (x + 0.5)·2/3 − 0.5.
        let t = taps(3, 2);
        assert_eq!((t[0].a, t[0].b), (0, 0));
        assert_eq!((t[1].a, t[1].b), (0, 1));
        assert!((t[1].f - 0.5).abs() < 1e-6);
        assert_eq!((t[2].a, t[2].b), (1, 1));
        // One sample.
        assert!(taps(1, 1).iter().all(|t| (t.a, t.b) == (0, 0)));
    }

    #[test]
    fn a_two_by_two_chroma_pattern_is_interpolated() {
        // 4×4 at full range BT.601, luma flat, U a 2×2 pattern, V flat.
        let (luma, v) = (128, 128);
        let u_plane = [40, 200, 120, 60];
        let expected_u = |x: usize, y: usize| -> f32 {
            let (tx, ty) = (taps(4, 2)[x], taps(4, 2)[y]);
            let at = |cx: usize, cy: usize| unit(u_plane[cy * 2 + cx]);
            lerp(
                lerp(at(tx.a, ty.a), at(tx.b, ty.a), tx.f),
                lerp(at(tx.a, ty.b), at(tx.b, ty.b), tx.f),
                ty.f,
            )
        };
        let rows = conversion(Bt601, Full);
        let nv12 = frame(
            4,
            4,
            PixelFormat::Nv12,
            vec![vec![luma; 16], u_plane.iter().flat_map(|&u| [u, v]).collect()],
            Bt601,
            Full,
        );
        let i420 = frame(
            4,
            4,
            PixelFormat::I420,
            vec![vec![luma; 16], u_plane.to_vec(), vec![v; 4]],
            Bt601,
            Full,
        );
        for f in [nv12, i420] {
            let still = still(&f);
            for y in 0..4 {
                for x in 0..4 {
                    let p = [unit(luma), expected_u(x, y), unit(v), 1.0];
                    let expected = rows.map(|r| r.iter().zip(p).map(|(a, b)| a * b).sum::<f32>().clamp(0.0, 1.0));
                    let i = (y * 4 + x) * 3;
                    within_one(&still.rgb[i..i + 3], expected, &format!("{:?} ({x}, {y})", f.format));
                }
            }
            // The corners take their own sample, the middle a blend.
            let blue = |x: usize, y: usize| still.rgb[(y * 4 + x) * 3 + 2];
            assert!(blue(0, 0) < blue(1, 0) && blue(1, 0) < blue(3, 0));
        }
    }

    #[test]
    fn packed_pixels_take_their_own_luma_and_their_pair_chroma() {
        // 2×1: Y0 = 50, Y1 = 200, one U and V.
        for (format, bytes) in [
            (PixelFormat::Yuy2, vec![50, 90, 200, 240]),
            (PixelFormat::Uyvy, vec![90, 50, 240, 200]),
        ] {
            let still = still(&frame(2, 1, format, vec![bytes], Bt709, Limited));
            within_one(&still.rgb[0..3], yuv_to_rgb(50, 90, 240, Bt709, Limited), "first");
            within_one(&still.rgb[3..6], yuv_to_rgb(200, 90, 240, Bt709, Limited), "second");
        }
    }

    #[test]
    fn odd_sizes_in_every_format() {
        for format in FORMATS {
            let still = still(&flat(5, 3, format, (235, 128, 128), Bt709, Limited));
            assert_eq!(
                (still.width, still.height, still.rgb.len()),
                (5, 3, 5 * 3 * 3),
                "{format:?}"
            );
            let expected = match format {
                PixelFormat::Bgrx | PixelFormat::Rgba => [unit(235), unit(128), unit(128)],
                _ => [1.0, 1.0, 1.0],
            };
            for px in still.rgb.as_chunks::<3>().0 {
                within_one(px, expected, &format!("{format:?}"));
            }
        }
    }

    #[test]
    fn rgb_formats_keep_each_pixel() {
        // 2×1 BGRx and RGBA with two different pixels.
        let bgrx = still(&frame(
            2,
            1,
            PixelFormat::Bgrx,
            vec![vec![3, 2, 1, 0, 30, 20, 10, 0]],
            Bt709,
            Full,
        ));
        assert_eq!(bgrx.rgb, [1, 2, 3, 10, 20, 30]);
        let rgba = still(&frame(
            2,
            1,
            PixelFormat::Rgba,
            vec![vec![1, 2, 3, 9, 10, 20, 30, 9]],
            Bt709,
            Full,
        ));
        assert_eq!(rgba.rgb, [1, 2, 3, 10, 20, 30]);
        assert_eq!(rgba.rgba(), [1, 2, 3, 255, 10, 20, 30, 255]);
    }

    #[test]
    fn png_round_trip() {
        let rgb: Vec<u8> = (0..7 * 5 * 3).map(|i| (i * 37 % 256) as u8).collect();
        let still = Still {
            width: 7,
            height: 5,
            rgb,
        };
        let bytes = still.png().expect("encodes");
        let mut reader = png::Decoder::new(std::io::Cursor::new(bytes))
            .read_info()
            .expect("decodes");
        let info = reader.info();
        assert_eq!((info.width, info.height), (7, 5));
        assert_eq!(
            (info.color_type, info.bit_depth),
            (png::ColorType::Rgb, png::BitDepth::Eight)
        );
        assert!(info.srgb.is_some());
        let mut pixels = vec![0; reader.output_buffer_size().expect("size")];
        let out = reader.next_frame(&mut pixels).expect("frame");
        assert_eq!(&pixels[..out.buffer_size()], &still.rgb[..]);
    }
}
