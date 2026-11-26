//! Compiles the Slint UI with its bundled translations and rasterizes the app icon.

use std::path::{Path, PathBuf};

/// Icon sizes for the window, taskbar and tray (research R12).
const ICON_SIZES: [u32; 7] = [16, 24, 32, 48, 64, 128, 256];

fn main() {
    println!("cargo:rerun-if-changed=ui");
    println!("cargo:rerun-if-changed=lang");
    println!("cargo:rerun-if-changed=../brand/X.svg");

    // Translations are bundled and picked at runtime (research R7); `.po` entries carry no context.
    let config = slint_build::CompilerConfiguration::new()
        .with_bundled_translations("lang")
        .with_default_translation_context(slint_build::DefaultTranslationContext::None);
    slint_build::compile_with_config("ui/main.slint", config).expect("Slint UI must compile");

    let out = PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR"));
    rasterize_icon(Path::new("../brand/X.svg"), &out);
    rasterize_watermark(Path::new("../brand/X.svg"), &out);
    for kind in ["grab", "grabbing"] {
        rasterize_cursor(&Path::new("ui/assets/cursors").join(format!("{kind}.svg")), kind, &out);
    }
}

/// The grab cursors' sides in pixels, one per common display scale (100 % to 300 %); the app
/// picks the nearest (spec FR-015, research R8). Keep in step with `cursor::SIZES`.
const CURSOR_SIZES: [u32; 8] = [32, 40, 48, 56, 64, 72, 80, 96];

/// Writes `cursor-<kind>-<size>.rgba` (straight RGBA, rows packed) for each size: the drawing's
/// square takes 22 of every 32 pixels, centred, as the maintainer drew it for a 32 px cursor.
fn rasterize_cursor(svg: &Path, kind: &str, out: &Path) {
    use resvg::{tiny_skia, usvg};
    let data = std::fs::read(svg).expect("cursor svg");
    let tree = usvg::Tree::from_data(&data, &usvg::Options::default()).expect("cursor svg must parse");
    for size in CURSOR_SIZES {
        let mut pixmap = tiny_skia::Pixmap::new(size, size).expect("cursor size");
        let side = size as f32 * 22.0 / 32.0;
        let scale = side / tree.size().width();
        let offset = (size as f32 - side) / 2.0;
        let transform = tiny_skia::Transform::from_scale(scale, scale).post_translate(offset, offset);
        resvg::render(&tree, transform, &mut pixmap.as_mut());
        let rgba: Vec<u8> = pixmap
            .pixels()
            .iter()
            .flat_map(|p| {
                let c = p.demultiply();
                [c.red(), c.green(), c.blue(), c.alpha()]
            })
            .collect();
        std::fs::write(out.join(format!("cursor-{kind}-{size}.rgba")), rgba).expect("write cursor");
    }
}

/// The watermark's side in pixels (device preview research R18).
const WATERMARK_SIZE: u32 = 1024;

/// Writes `watermark.a8`: the X's coverage, one byte per pixel, `WATERMARK_SIZE` square.
fn rasterize_watermark(svg: &Path, out: &Path) {
    use resvg::{tiny_skia, usvg};
    let data = std::fs::read(svg).expect("brand/X.svg");
    let tree = usvg::Tree::from_data(&data, &usvg::Options::default()).expect("brand/X.svg must parse");
    let size = WATERMARK_SIZE;
    let mut pixmap = tiny_skia::Pixmap::new(size, size).expect("watermark size");
    let tree_size = tree.size();
    let scale = (size as f32 / tree_size.width()).min(size as f32 / tree_size.height());
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    let alpha: Vec<u8> = pixmap.pixels().iter().map(|p| p.alpha()).collect();
    std::fs::write(out.join("watermark.a8"), alpha).expect("write watermark");
}

/// Writes `icon-<size>.rgba` (straight RGBA, rows packed) for each size into `out`.
fn rasterize_icon(svg: &Path, out: &Path) {
    use resvg::{tiny_skia, usvg};
    let data = std::fs::read(svg).expect("brand/X.svg");
    let tree = usvg::Tree::from_data(&data, &usvg::Options::default()).expect("brand/X.svg must parse");
    let tree_size = tree.size();
    for size in ICON_SIZES {
        let mut pixmap = tiny_skia::Pixmap::new(size, size).expect("icon size");
        // Fit the drawing in the square, centered.
        let scale = (size as f32 / tree_size.width()).min(size as f32 / tree_size.height());
        let (dx, dy) = (
            (size as f32 - tree_size.width() * scale) / 2.0,
            (size as f32 - tree_size.height() * scale) / 2.0,
        );
        let transform = tiny_skia::Transform::from_scale(scale, scale).post_translate(dx, dy);
        resvg::render(&tree, transform, &mut pixmap.as_mut());
        // tiny-skia stores premultiplied pixels; window and tray icons take straight alpha.
        let rgba: Vec<u8> = pixmap
            .pixels()
            .iter()
            .flat_map(|p| {
                let c = p.demultiply();
                [c.red(), c.green(), c.blue(), c.alpha()]
            })
            .collect();
        std::fs::write(out.join(format!("icon-{size}.rgba")), rgba).expect("write icon");
    }
}
