"""Write the placeholder screenshot sound to app/sounds/screenshot.wav.

Screenshot spec (specs/20261121-134830-screenshot), research R10: 120 ms, 48 kHz mono 16-bit,
1.8 kHz for 50 ms then 1.2 kHz for 70 ms, each with a 2 ms attack and an exponential decay,
peak -3 dBFS. The maintainer's final sound replaces this file (tasks.md T040); this one is never
committed.

Usage: python tools/make_screenshot_sound.py
"""

import math
import struct
import wave
from pathlib import Path

RATE = 48_000
PEAK = 10 ** (-3 / 20)  # -3 dBFS
ATTACK = 0.002
TONES = [(1800.0, 0.050), (1200.0, 0.070)]  # (frequency in Hz, length in seconds)


def tone(frequency: float, length: float) -> list[float]:
    count = round(length * RATE)
    # The decay reaches about -40 dB at the tone's end.
    decay = math.log(100) / length
    samples = []
    for i in range(count):
        t = i / RATE
        envelope = min(t / ATTACK, 1.0) * math.exp(-decay * t)
        samples.append(PEAK * envelope * math.sin(2 * math.pi * frequency * t))
    return samples


def main() -> None:
    samples = [s for frequency, length in TONES for s in tone(frequency, length)]
    out = Path(__file__).resolve().parent.parent / "app" / "sounds" / "screenshot.wav"
    out.parent.mkdir(parents=True, exist_ok=True)
    with wave.open(str(out), "wb") as file:
        file.setnchannels(1)
        file.setsampwidth(2)
        file.setframerate(RATE)
        file.writeframes(b"".join(struct.pack("<h", round(s * 32767)) for s in samples))
    print(f"{out}: {len(samples)} samples, {len(samples) / RATE * 1000:.0f} ms")


if __name__ == "__main__":
    main()
