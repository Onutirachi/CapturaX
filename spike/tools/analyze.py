# /// script
# requires-python = ">=3.11"
# dependencies = ["opencv-python", "numpy", "imageio-ffmpeg"]
# ///
"""Analyzes latency clips for the stack spike (research R2, R3, R12).

Usage (contracts/cli.md):
    uv run tools/analyze.py video <clip> --path <p> --variant <v> --mode <m> --platform <os>
    uv run tools/analyze.py audio <clip> --platform <os> --output <device> --minute <n>
    uv run tools/analyze.py summary
"""

import argparse
import sys


def video(args: argparse.Namespace) -> None:
    sys.exit("video: not implemented yet (T016)")


def audio(args: argparse.Namespace) -> None:
    sys.exit("audio: not implemented yet (T024)")


def summary(args: argparse.Namespace) -> None:
    sys.exit("summary: not implemented yet (T017)")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    commands = parser.add_subparsers(dest="command", required=True)

    p = commands.add_parser("video", help="detect image changes and append video samples")
    p.add_argument("clip")
    p.add_argument("--path", required=True)
    p.add_argument("--variant", required=True)
    p.add_argument("--mode", required=True)
    p.add_argument("--platform", required=True, choices=["windows", "linux"])
    p.set_defaults(run=video)

    p = commands.add_parser("audio", help="detect sound onset pairs and append audio samples")
    p.add_argument("clip")
    p.add_argument("--platform", required=True, choices=["windows", "linux"])
    p.add_argument("--output", required=True)
    p.add_argument("--minute", required=True, type=int)
    p.set_defaults(run=audio)

    p = commands.add_parser("summary", help="write summary tables from all samples")
    p.set_defaults(run=summary)

    args = parser.parse_args()
    args.run(args)


if __name__ == "__main__":
    main()
