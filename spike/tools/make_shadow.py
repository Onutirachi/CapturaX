# /// script
# requires-python = ">=3.11"
# dependencies = ["pillow"]
# ///
"""Makes pre-blurred shadow images for nine-slice use (research R14, T048).

Slint's software renderer does not draw drop-shadow-*, so raised surfaces put one of these images
under them, stretched as nine-slice. Each image is a black rounded rectangle blurred with a
Gaussian; the margin around it is 3 sigma, so the shadow fades out inside the image. The shape
itself is cut out: the renderer applies opacity to each element on its own, so under a
semi-transparent surface a filled shadow would show through and darken it.

The light comes from above, so the shadow is moved down inside the image while the hole stays
where the surface is. Prints the nine-slice borders (top, right, bottom, left) and the margin of
each image: in Slint the shadow element is the surface grown by the margin on every side, plus
the offset at the bottom.

Usage: uv run tools/make_shadow.py [--out layered-preview/ui/assets/shadows]
"""

import argparse
import pathlib

from PIL import Image, ImageDraw, ImageFilter

# name: (corner radius, blur sigma, offset down, opacity). Two layers make the "layered shadows":
# a wide soft one for elevation and a tight one for the contact edge.
PRESETS = {
    "soft": (16, 12, 10, 0.45),
    "tight": (16, 3, 2, 0.35),
}


def make(radius: int, sigma: int, offset: int, opacity: float) -> tuple[Image.Image, tuple[int, ...], int]:
    margin = 3 * sigma
    border = margin + radius
    # Straight edge between the corners, so the stretched middle slices are uniform.
    width = 2 * border + 8
    height = width + offset
    hole = (margin, margin, width - margin - 1, height - margin - offset - 1)
    shadow = (hole[0], hole[1] + offset, hole[2], hole[3] + offset)
    mask = Image.new("L", (width, height), 0)
    ImageDraw.Draw(mask).rounded_rectangle(shadow, radius=radius, fill=round(255 * opacity))
    mask = mask.filter(ImageFilter.GaussianBlur(sigma))
    ImageDraw.Draw(mask).rounded_rectangle(hole, radius=radius, fill=0)
    image = Image.new("RGBA", (width, height), (0, 0, 0, 0))
    image.putalpha(mask)
    return image, (border, border, border + offset, border), margin


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--out", type=pathlib.Path, default=pathlib.Path("layered-preview/ui/assets/shadows"))
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    for name, (radius, sigma, offset, opacity) in PRESETS.items():
        image, borders, margin = make(radius, sigma, offset, opacity)
        path = args.out / f"{name}.png"
        image.save(path)
        print(f"{path}: {image.width}x{image.height}, nine-slice{borders}, margin {margin}, offset {offset}")


if __name__ == "__main__":
    main()
