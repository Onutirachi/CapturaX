# /// script
# requires-python = ">=3.11"
# dependencies = ["imageio-ffmpeg"]
# ///
"""Makes the A/V sync test video (research R2, T045).

1080p60 MP4 (H.264 + AAC, plays in the PS5 media gallery): black screen with a full-screen white
flash during the first 3 frames (50 ms) of every second, and a 1 kHz beep during the same 50 ms.

Usage: uv run tools/make_sync_video.py [--seconds 120] [--out ../clips/sync-test.mp4]
Copy the file to a USB drive (exFAT or FAT32) and open it from the PS5 media gallery.
"""

import argparse
import pathlib
import subprocess

import imageio_ffmpeg

FPS = 60
FLASH_FRAMES = 3  # 50 ms at 60 fps


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--seconds", type=int, default=120)
    parser.add_argument("--out", default=str(pathlib.Path(__file__).parents[2] / "clips" / "sync-test.mp4"))
    args = parser.parse_args()

    out = pathlib.Path(args.out)
    out.parent.mkdir(parents=True, exist_ok=True)
    flash_secs = FLASH_FRAMES / FPS
    flash = f"drawbox=x=0:y=0:w=iw:h=ih:color=white:t=fill:enable='lt(mod(n,{FPS}),{FLASH_FRAMES})'"
    # Evaluated per sample, so the beep starts exactly with the flash. A gate applied per audio frame
    # would snap it to 1024-sample blocks, up to 21 ms late.
    beep = f"0.5*sin(2*PI*1000*t)*lt(mod(t,1),{flash_secs})"
    command = [
        imageio_ffmpeg.get_ffmpeg_exe(), "-y", "-hide_banner", "-loglevel", "error",
        "-f", "lavfi", "-i", f"color=c=black:s=1920x1080:r={FPS}:d={args.seconds}",
        "-f", "lavfi", "-i", f"aevalsrc='{beep}|{beep}':s=48000:d={args.seconds}",
        "-vf", flash,
        "-c:v", "libx264", "-pix_fmt", "yuv420p", "-profile:v", "high", "-r", str(FPS),
        "-c:a", "aac", "-b:a", "192k", "-movflags", "+faststart", str(out),
    ]
    subprocess.run(command, check=True)
    print(f"Wrote {out} ({out.stat().st_size // 1024} KB, {args.seconds} s)")


if __name__ == "__main__":
    main()
