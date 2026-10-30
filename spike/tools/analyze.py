# /// script
# requires-python = ">=3.11"
# dependencies = ["opencv-python", "numpy", "imageio-ffmpeg"]
# ///
"""Analyzes latency clips for the stack spike (research R2, R3, R12).

Usage (contracts/cli.md):
    uv run tools/analyze.py video <clip> --path <p> --variant <v> --mode <m> --platform <os>
    uv run tools/analyze.py audio <clip> --platform <os> --output <device> --minute <n>
    uv run tools/analyze.py summary

Video: the TV (passthrough reference) and the preview are filmed side by side in slow motion. On
the first run of a clip a window asks for the two regions (TV first, then preview); they are saved
next to the clip as <clip>.regions.json. Each sudden image change in a region (a white flash or a
PS5 menu screen change) is an event; each TV event is paired with the next preview event.

Audio: a recording where every sound is heard twice, first from the reference (TV or line input),
then from the preview. Each pair of sound onsets is one sample. With --stereo the reference is on
the left channel and the preview on the right, and the preview may come first (negative delay).

Re-running a clip replaces its rows, so a clip can be analyzed again with other settings.
"""

import argparse
import csv
import json
import math
import pathlib
import subprocess
import sys

import numpy as np

MEASUREMENTS = pathlib.Path(__file__).parents[2] / "specs" / "20261026-154808-stack-spike" / "measurements"
CLIPS = pathlib.Path(__file__).parents[2] / "clips"
VIDEO_CSV = MEASUREMENTS / "video-samples.csv"
AUDIO_CSV = MEASUREMENTS / "audio-samples.csv"
STATS_CSV = MEASUREMENTS / "pipeline-stats.csv"
SUMMARY_MD = MEASUREMENTS / "summary.md"

VIDEO_FIELDS = ["clip", "path", "variant", "mode", "platform", "flash_index", "delay_ms"]
AUDIO_FIELDS = ["clip", "platform", "output", "minute", "beep_index", "delay_ms"]

# Paths always listed in the summary, measured or not (data-model.md).
VIDEO_PATHS = ["native", "slint-cpu", "slint-gpu", "slint-layered", "browser", "obs"]

MIN_PAIRS = 5  # a clip with fewer pairs is rejected (data-model.md)
MAX_DELAY_MS = 500.0  # above this a pair is a detection error and is excluded


# ---------------------------------------------------------------- shared


def clip_key(clip: pathlib.Path) -> str:
    """The clip's name in the CSVs: its path under clips/ (e.g. linux/1-native-60.mp4), or its name."""
    try:
        return clip.resolve().relative_to(CLIPS.resolve()).as_posix()
    except ValueError:
        return clip.name


def write_rows(csv_path: pathlib.Path, fields: list[str], clip: str, rows: list[dict]) -> None:
    """Replaces the rows of `clip` in `csv_path` with `rows`."""
    kept = [r for r in read_rows(csv_path) if r["clip"] != clip]
    csv_path.parent.mkdir(parents=True, exist_ok=True)
    with csv_path.open("w", newline="", encoding="utf-8") as f:
        writer = csv.DictWriter(f, fieldnames=fields, lineterminator="\n")
        writer.writeheader()
        writer.writerows(kept + rows)


def read_rows(csv_path: pathlib.Path) -> list[dict]:
    if not csv_path.exists():
        return []
    with csv_path.open(newline="", encoding="utf-8") as f:
        return list(csv.DictReader(f))


def onsets(signal: np.ndarray, threshold: float, min_gap: int) -> list[int]:
    """Indices where `signal` rises to `threshold`, at least `min_gap` apart."""
    above = signal >= threshold
    rising = np.flatnonzero(above[1:] & ~above[:-1]) + 1
    found: list[int] = []
    for i in rising:
        if not found or i - found[-1] >= min_gap:
            found.append(int(i))
    return found


# ---------------------------------------------------------------- video


def video_fps(capture, override: float | None) -> float:
    import cv2

    if override:
        return override
    fps = capture.get(cv2.CAP_PROP_FPS)
    if not fps or fps < 100:
        sys.exit(
            f"the clip says {fps:.1f} fps; a slow-motion clip may store its playback rate instead "
            "of the recording rate: pass --fps 240"
        )
    return fps


def load_regions(clip: pathlib.Path, capture, given: str | None, remark: bool) -> dict:
    regions_file = clip.with_name(clip.name + ".regions.json")
    if given:
        boxes = [[int(v) for v in box.split(",")] for box in given.split()]
        if len(boxes) != 2 or any(len(b) != 4 for b in boxes):
            sys.exit('--regions takes two boxes "x,y,w,h x,y,w,h" (TV, then preview)')
        regions = {"tv": boxes[0], "preview": boxes[1]}
    elif regions_file.exists() and not remark:
        return json.loads(regions_file.read_text(encoding="utf-8"))
    else:
        regions = mark_regions(capture)
    regions_file.write_text(json.dumps(regions) + "\n", encoding="utf-8", newline="\n")
    print(f"regions saved to {regions_file.name}")
    return regions


def mark_regions(capture) -> dict:
    """Asks for the two rectangles on the brightest of the first 480 frames (both screens lit)."""
    import cv2

    best, best_level = None, -1.0
    for _ in range(480):
        ok, frame = capture.read()
        if not ok:
            break
        level = float(frame.mean())
        if level > best_level:
            best, best_level = frame, level
    capture.set(cv2.CAP_PROP_POS_FRAMES, 0)
    if best is None:
        sys.exit("the clip has no frames")

    # Fit the frame on a typical screen; the boxes are scaled back to the clip's pixels.
    scale = min(1.0, 1600 / best.shape[1], 900 / best.shape[0])
    shown = cv2.resize(best, None, fx=scale, fy=scale, interpolation=cv2.INTER_AREA)
    regions = {}
    for name in ("tv", "preview"):
        title = f"Mark the {'TV (reference)' if name == 'tv' else 'preview'} screen, then Enter"
        x, y, w, h = cv2.selectROI(title, shown, showCrosshair=False)
        cv2.destroyWindow(title)
        if w == 0 or h == 0:
            sys.exit("no region marked")
        regions[name] = [round(x / scale), round(y / scale), round(w / scale), round(h / scale)]
    return regions


def change_signals(capture, regions: dict, lag: int = 1) -> tuple[dict[str, np.ndarray], dict[str, np.ndarray]]:
    """Per region, the mean absolute difference between each frame and the one `lag` frames
    before, and the mean brightness of each frame (both 0–255). A lag above 1 sees the whole of a
    change that a screen spreads over several camera frames, for noisy clips (dim room)."""
    import cv2

    previous: dict[str, list[np.ndarray]] = {name: [] for name in regions}
    signals: dict[str, list[float]] = {name: [] for name in regions}
    levels: dict[str, list[float]] = {name: [] for name in regions}
    while True:
        ok, frame = capture.read()
        if not ok:
            break
        gray = cv2.cvtColor(frame, cv2.COLOR_BGR2GRAY)
        for name, (x, y, w, h) in regions.items():
            crop = gray[y : y + h, x : x + w]
            # A small image is enough to see a screen change and ignores sensor noise.
            small = cv2.resize(crop, (64, max(1, round(64 * h / w))), interpolation=cv2.INTER_AREA)
            small = small.astype(np.float32)
            levels[name].append(float(small.mean()))
            past = previous[name]
            signals[name].append(float(np.abs(small - past[0]).mean()) if len(past) == lag else 0.0)
            past.append(small)
            if len(past) > lag:
                past.pop(0)
    return (
        {name: np.array(values) for name, values in signals.items()},
        {name: np.array(values) for name, values in levels.items()},
    )


def change_threshold(signal: np.ndarray) -> float:
    """Above the noise of a still screen, and a fair part of the largest change."""
    floor = float(np.median(signal))
    noise = 1.4826 * float(np.median(np.abs(signal - floor)))
    return floor + max(6 * noise, 0.2 * (float(signal.max()) - floor), 1.0)


def refine(level: np.ndarray, i: int) -> tuple[float, bool]:
    """The moment, between frames, when the brightness crosses halfway from before the change at
    frame `i` to its fullest within the next frames, and whether it went from under 20% to over
    80% of the way within one camera frame. Two screens differ in how fast their pixels switch;
    halfway is the same point on both. A change that barely moves the brightness (a menu screen
    of the same tone) keeps frame `i`."""
    before = float(np.median(level[max(0, i - 8) : max(1, i - 1)]))
    window = level[max(0, i - 2) : i + 6]
    after = float(window[np.argmax(np.abs(window - before))])
    if abs(after - before) < 20:
        return float(i), False
    progress = (level - before) / (after - before)  # 0 before the change, 1 at its fullest
    for k in range(max(1, i - 2), min(len(level), i + 6)):
        if progress[k] >= 0.5 > progress[k - 1]:
            sudden = progress[k - 1] < 0.2 and progress[k] > 0.8
            return k - 1 + (0.5 - progress[k - 1]) / (progress[k] - progress[k - 1]), bool(sudden)
    return float(i), False


def pair_events(reference: list[int], preview: list[int]) -> list[tuple[int, int]]:
    """Each reference event with the first unused preview event at or after it and before the next
    reference event."""
    pairs = []
    j = 0
    for k, ref in enumerate(reference):
        limit = reference[k + 1] if k + 1 < len(reference) else math.inf
        while j < len(preview) and preview[j] < ref:
            j += 1
        if j < len(preview) and preview[j] < limit:
            pairs.append((ref, preview[j]))
            j += 1
    return pairs


def video(args: argparse.Namespace) -> None:
    import cv2

    clip = pathlib.Path(args.clip)
    capture = cv2.VideoCapture(str(clip))
    if not capture.isOpened():
        sys.exit(f"cannot open {clip}")
    fps = video_fps(capture, args.fps)
    regions = load_regions(clip, capture, args.regions, args.remark)
    signals, levels = change_signals(capture, regions, args.lag)
    frames = len(signals["tv"])
    print(f"{clip.name}: {frames} frames at {fps:g} fps ({frames / fps:.1f} s), one frame = {1000 / fps:.2f} ms")

    min_gap = max(1, round(args.min_gap / 1000 * fps))
    events = {}
    for name, signal in signals.items():
        threshold = change_threshold(signal)
        events[name] = onsets(signal, threshold, min_gap)
        print(f"  {name}: {len(events[name])} changes (threshold {threshold:.1f})")

    rows, flagged = [], 0
    print("  pair  tv frame  preview frame  camera frames  delay ms")
    for ref_frame, prev_frame in pair_events(events["tv"], events["preview"]):
        (ref, sudden), (prev, _) = refine(levels["tv"], ref_frame), refine(levels["preview"], prev_frame)
        delay = (prev - ref) / fps * 1000
        note = ""
        if delay > MAX_DELAY_MS:
            flagged += 1
            note = "  excluded: detection error"
        elif sudden:
            # The reference display always takes more than one camera frame to switch; doing it
            # in one means the camera skipped frames there, which hides part of the delay.
            flagged += 1
            note = "  excluded: camera skipped frames"
        else:
            rows.append(
                {
                    "clip": clip_key(clip), "path": args.path, "variant": args.variant, "mode": args.mode,
                    "platform": args.platform, "flash_index": len(rows) + 1, "delay_ms": f"{delay:.1f}",
                }
            )
        print(f"  {len(rows) + flagged:>4}  {ref:>8.2f}  {prev:>13.2f}  {prev - ref:>13.2f}  {delay:>8.1f}{note}")

    if len(rows) < MIN_PAIRS:
        sys.exit(f"rejected: {len(rows)} valid pairs, at least {MIN_PAIRS} needed; nothing written")
    write_rows(VIDEO_CSV, VIDEO_FIELDS, clip_key(clip), rows)
    delays = np.array([float(r["delay_ms"]) for r in rows])
    print(
        f"{len(rows)} samples written ({flagged} excluded): median {np.median(delays):.1f} ms, "
        f"p95 {np.percentile(delays, 95):.1f} ms"
    )


# ---------------------------------------------------------------- audio

AUDIO_RATE = 48000
HOP = 48  # 1 ms


def read_audio(clip: pathlib.Path, channels: int = 1) -> np.ndarray:
    """Any audio or video file, mixed to mono (or as `channels` columns), as float samples at 48 kHz."""
    import imageio_ffmpeg

    command = [
        imageio_ffmpeg.get_ffmpeg_exe(), "-hide_banner", "-loglevel", "error", "-i", str(clip),
        "-vn", "-ac", str(channels), "-ar", str(AUDIO_RATE), "-f", "s16le", "-",
    ]
    result = subprocess.run(command, capture_output=True)
    if result.returncode != 0:
        sys.exit(f"cannot read the audio of {clip}: {result.stderr.decode(errors='replace').strip()}")
    samples = np.frombuffer(result.stdout, dtype=np.int16).astype(np.float32) / 32768
    return samples if channels == 1 else samples.reshape(-1, channels)


def sound_onsets(samples: np.ndarray, rise_db: float, min_gap: int) -> tuple[list[int], float]:
    """Onsets in ms: the level over 2 ms rises `rise_db` above the 30 ms before it, well above
    the background; onsets closer than `min_gap` ms are one sound. Returns the onsets and the
    background level in dBFS."""
    hops = len(samples) // HOP
    energy = (samples[: hops * HOP].reshape(hops, HOP) ** 2).mean(axis=1)
    level = np.convolve(energy, np.ones(2) / 2, mode="same")
    before = np.convolve(energy, np.ones(30) / 30, mode="full")[: hops]
    before = np.concatenate([np.full(5, before[0]), before[:-5]])  # the 30 ms ending 5 ms earlier
    background = float(np.median(energy)) + 1e-12
    loudest = float(np.percentile(level, 99.9)) + 1e-12
    eps = 1e-12
    rise = 10 * np.log10((level + eps) / (before + eps))
    loud = level >= max(background * 100, loudest / 1000)  # 20 dB over background, 30 dB under peak
    found = onsets(((rise >= rise_db) & loud).astype(np.float32), 0.5, min_gap)
    found = [t for t in found if t >= 35]  # a sound cut by the start of the file has no onset
    return found, 10 * math.log10(background)


def stereo_pairs(samples: np.ndarray, args: argparse.Namespace) -> list[tuple[int, int]]:
    """Each onset on the left (reference) with the nearest one on the right (preview) within
    --max-delay either way."""
    left, background = sound_onsets(samples[:, 0], args.rise, round(args.min_gap))
    right, _ = sound_onsets(samples[:, 1], args.rise, round(args.min_gap))
    print(f"  {len(left)} onsets on the left, {len(right)} on the right, background {background:.0f} dBFS")
    pairs = []
    for ref in left:
        near = [t for t in right if abs(t - ref) <= args.max_delay]
        if near:
            pairs.append((ref, min(near, key=lambda t: abs(t - ref))))
    return pairs


def mono_pairs(samples: np.ndarray, args: argparse.Namespace) -> list[tuple[int, int]]:
    """An onset (reference) followed by another within --max-delay; a third onset in the same
    burst (an echo, a long click) is ignored."""
    found, background = sound_onsets(samples, args.rise, round(args.min_gap))
    print(f"  {len(found)} sound onsets, background {background:.0f} dBFS")
    pairs, extra = [], 0
    i = 0
    while i < len(found):
        ref = found[i]
        burst = [t for t in found[i + 1 :] if t - ref <= args.max_delay]
        if burst:
            pairs.append((ref, burst[0]))
            extra += len(burst) - 1
        i += 1 + len(burst)
    if extra:
        print(f"  {extra} extra onsets inside pairs ignored")
    return pairs


def audio(args: argparse.Namespace) -> None:
    clip = pathlib.Path(args.clip)
    samples = read_audio(clip, 2 if args.stereo else 1)
    print(f"{clip.name}: {len(samples) / AUDIO_RATE:.1f} s")
    pairs = stereo_pairs(samples, args) if args.stereo else mono_pairs(samples, args)

    rows = []
    print("  pair  reference s  preview s  delay ms")
    for ref, preview in pairs:
        delay = preview - ref
        rows.append(
            {
                "clip": clip_key(clip), "platform": args.platform, "output": args.output,
                "minute": args.minute, "beep_index": len(rows) + 1, "delay_ms": f"{delay:.0f}",
            }
        )
        print(f"  {len(rows):>4}  {ref / 1000:>11.3f}  {preview / 1000:>9.3f}  {delay:>8.0f}")

    if len(rows) < MIN_PAIRS:
        sys.exit(f"rejected: {len(rows)} pairs, at least {MIN_PAIRS} needed; nothing written")
    write_rows(AUDIO_CSV, AUDIO_FIELDS, clip_key(clip), rows)
    delays = np.array([float(r["delay_ms"]) for r in rows])
    print(f"{len(rows)} samples written: median {np.median(delays):.0f} ms, p95 {np.percentile(delays, 95):.0f} ms")


# ---------------------------------------------------------------- summary


def same_mode(mode: str) -> str:
    """The prototypes write `1920x1080@60:NV12`; samples may be typed `1920x1080@60 NV12`."""
    return " ".join(mode.replace(":", " ").split())


def stats_line(values: list[float]) -> dict[str, str]:
    if not values:
        return {"n": "n/a", "median": "n/a", "p95": "n/a", "min": "n/a", "max": "n/a"}
    a = np.array(values)
    return {
        "n": str(len(a)),
        "median": f"{np.median(a):.1f}",
        "p95": f"{np.percentile(a, 95):.1f}",
        "min": f"{a.min():.1f}",
        "max": f"{a.max():.1f}",
    }


def dropped_frames(rows: list[dict]) -> dict[tuple, str]:
    """Dropped-frame percentage per (platform, mode, path, variant) over all recorded runs. The
    counters are cumulative per run (and read 0 once the sink stops), so a run's totals are its
    largest values."""
    runs: dict[tuple, list[int]] = {}
    for r in rows:
        if r["run"] == "run":  # a header repeated by appending to a file
            continue
        t = runs.setdefault((r["run"], r["platform"], same_mode(r["mode"]), r["path"], r["variant"]), [0, 0])
        t[0] = max(t[0], int(r["rendered"] or 0))
        t[1] = max(t[1], int(r["dropped"] or 0))
    totals: dict[tuple, list[int]] = {}
    for (_, *key), (rendered, dropped) in runs.items():
        t = totals.setdefault(tuple(key), [0, 0])
        t[0] += rendered
        t[1] += dropped
    return {k: f"{100 * d / (r + d):.2f}%" if r + d else "n/a" for k, (r, d) in totals.items()}


def table(header: list[str], rows: list[list[str]]) -> list[str]:
    lines = ["| " + " | ".join(header) + " |", "|" + "|".join("---" for _ in header) + "|"]
    lines += ["| " + " | ".join(row) + " |" for row in rows]
    return lines


def summary(args: argparse.Namespace) -> None:
    video_rows = read_rows(VIDEO_CSV)
    audio_rows = read_rows(AUDIO_CSV)
    dropped = dropped_frames(read_rows(STATS_CSV))

    lines = [
        "# Measurement Summary",
        "",
        "Generated by `uv run tools/analyze.py summary` from the CSV files in this folder; do not edit.",
        "Delays in ms, relative to the reference (the passthrough display for video).",
        "",
        "## Video",
    ]
    samples: dict[tuple, list[float]] = {}
    for r in video_rows:
        samples.setdefault((r["platform"], same_mode(r["mode"]), r["path"], r["variant"]), []).append(float(r["delay_ms"]))
    platforms = sorted({k[0] for k in samples} | {k[0] for k in dropped}) or ["windows"]
    for platform in platforms:
        modes = sorted({k[1] for k in samples if k[0] == platform} | {k[1] for k in dropped if k[0] == platform})
        for mode in modes or ["no mode measured"]:
            lines += ["", f"### {platform}, {mode}", ""]
            keys = {k[2:] for k in samples if k[:2] == (platform, mode)}
            keys |= {k[2:] for k in dropped if k[:2] == (platform, mode)}
            keys |= {(p, "—") for p in VIDEO_PATHS if not any(k[0] == p for k in keys)}
            order = sorted(keys, key=lambda k: (VIDEO_PATHS.index(k[0]) if k[0] in VIDEO_PATHS else 99, k))
            rows = []
            for path, variant in order:
                s = stats_line(samples.get((platform, mode, path, variant), []))
                drop = dropped.get((platform, mode, path, variant), "n/a")
                rows.append([path, variant, s["n"], s["median"], s["p95"], s["min"], s["max"], drop])
            lines += table(["Path", "Variant", "n", "Median", "p95", "Min", "Max", "Dropped"], rows)

    lines += ["", "## Audio", ""]
    by_minute: dict[tuple, list[float]] = {}
    for r in audio_rows:
        by_minute.setdefault((r["platform"], r["output"], int(r["minute"])), []).append(float(r["delay_ms"]))
    reference = None
    if args.av_reference:
        path, variant, mode = (part.strip() for part in args.av_reference.split(","))
        reference = (path, variant, same_mode(mode))
    rows = []
    for platform, output in sorted({k[:2] for k in by_minute}):
        minutes = sorted(m for p, o, m in by_minute if (p, o) == (platform, output))
        medians = {m: float(np.median(by_minute[(platform, output, m)])) for m in minutes}
        drift = f"{medians[minutes[-1]] - medians[minutes[0]]:.1f}" if len(minutes) > 1 else "n/a"
        video_median = None
        if reference:
            values = samples.get((platform, reference[2], reference[0], reference[1]))
            video_median = float(np.median(values)) if values else None
        for m in minutes:
            s = stats_line(by_minute[(platform, output, m)])
            offset = f"{medians[m] - video_median:.1f}" if video_median is not None else "n/a"
            rows.append([platform, output, str(m), s["n"], s["median"], s["p95"], offset, drift])
    if rows:
        lines += table(["Platform", "Output", "Minute", "n", "Median", "p95", "A/V offset", "Drift"], rows)
        lines += [
            "",
            "A/V offset: audio median − video median of "
            + (f"`{', '.join(reference)}`" if reference else "no video reference (pass `--av-reference`)")
            + "; positive means the sound is behind the image. Drift: last minute's median − first's.",
        ]
    else:
        lines.append("No audio samples yet.")

    SUMMARY_MD.write_text("\n".join(lines) + "\n", encoding="utf-8", newline="\n")
    print(f"written {SUMMARY_MD}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    commands = parser.add_subparsers(dest="command", required=True)

    p = commands.add_parser("video", help="detect image changes and append video samples")
    p.add_argument("clip")
    p.add_argument("--path", required=True)
    p.add_argument("--variant", required=True)
    p.add_argument("--mode", required=True)
    p.add_argument("--platform", required=True, choices=["windows", "linux"])
    p.add_argument("--fps", type=float, help="recording rate, when the clip stores another (e.g. 240)")
    p.add_argument("--regions", help='"x,y,w,h x,y,w,h": TV and preview boxes, instead of marking them')
    p.add_argument("--remark", action="store_true", help="mark the regions again")
    p.add_argument("--min-gap", type=float, default=300, help="ms after a change during which others are the same change")
    p.add_argument("--lag", type=int, default=1, help="compare each frame with the one this many frames before (noisy clips: 3)")
    p.set_defaults(run=video)

    p = commands.add_parser("audio", help="detect sound onset pairs and append audio samples")
    p.add_argument("clip")
    p.add_argument("--platform", required=True, choices=["windows", "linux"])
    p.add_argument("--output", required=True)
    p.add_argument("--minute", required=True, type=int)
    p.add_argument("--max-delay", type=float, default=300, help="most ms between the two sounds of a pair")
    p.add_argument("--rise", type=float, default=12, help="dB a sound must rise over the 30 ms before it")
    p.add_argument("--min-gap", type=float, default=20, help="ms after an onset during which others are the same sound")
    p.add_argument("--stereo", action="store_true", help="reference on the left channel, preview on the right")
    p.set_defaults(run=audio)

    p = commands.add_parser("summary", help="write summary tables from all samples")
    p.add_argument("--av-reference", help='"path,variant,mode" of the video run the audio offset is measured against')
    p.set_defaults(run=summary)

    args = parser.parse_args()
    args.run(args)


if __name__ == "__main__":
    main()
