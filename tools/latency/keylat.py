# /// script
# requires-python = ">=3.11"
# dependencies = ["numpy", "scipy", "sounddevice", "soundcard", "pynput"]
# ///
"""Windows: how late an app plays a microphone, by its own keyboard clicks (no extra hardware).

Usage, from the repository root (see tools/latency/README.md):
    uv run tools/latency/keylat.py --mic C920 --loopback "Voicemeeter Input" --seconds 40 --save clips/k.npz

Records, on one perf_counter clock:
- the space bar's presses (a global keyboard hook, any window focused);
- the microphone directly (sounddevice, WASAPI shared): what any app receives from it;
- a loopback of the output the app plays on (soundcard): what the app sends.

Press the space bar with short taps (a loud mechanical key), or speak near the microphone, a few
seconds apart. Each stream's sample k is placed at a + k/fs, with a the earliest
(block arrival - block end / fs): the time the sample reached the client, at best. Printed:
- per press, its click's onset in each stream (key → microphone shows the microphone's own delay);
- the output's lag behind the microphone by cross-correlation in 4 s windows: the delay added by
  the app (or by whatever plays the microphone, such as Windows' "Listen to this device") plus the
  output endpoint up to its loopback tap.

A loopback taps the endpoint where Windows mixes it, not where the sound is heard; on a virtual
endpoint (such as Voicemeeter's) compare with a reference on the same endpoint.
"""
import argparse
import threading
import time

import numpy as np
import soundcard as sc
import sounddevice as sd
from pynput import keyboard
from scipy.signal import correlate

FS = 48000
BLOCK = 480


def main():
    p = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    p.add_argument("--mic", required=True, help="part of the microphone's name")
    p.add_argument("--loopback", required=True, help="part of the output endpoint's name")
    p.add_argument("--seconds", type=float, default=40)
    p.add_argument("--save", help="keep the recordings and key times in this .npz")
    a = p.parse_args()

    wasapi = next(i for i, h in enumerate(sd.query_hostapis()) if "WASAPI" in h["name"])
    mic = next(
        i
        for i, d in enumerate(sd.query_devices())
        if d["hostapi"] == wasapi and a.mic.lower() in d["name"].lower() and d["max_input_channels"] > 0
    )
    loop = next(
        d for d in sc.all_microphones(include_loopback=True) if d.isloopback and a.loopback.lower() in d.name.lower()
    )
    print(f"microphone: {sd.query_devices(mic)['name']}; loopback: {loop.name}", flush=True)

    data = {"mic": [], "out": []}
    arrivals = {"mic": [], "out": []}
    keys = []
    ready = threading.Barrier(3)
    stop = [float("inf")]

    def mic_thread():
        n = [0]

        def cb(indata, frames, _time, _status):
            t = time.perf_counter()
            n[0] += frames
            data["mic"].append(indata[:, 0].copy())
            arrivals["mic"].append((t, n[0]))

        with sd.InputStream(device=mic, samplerate=FS, channels=1, blocksize=BLOCK, latency="low", callback=cb):
            ready.wait()
            while time.perf_counter() < stop[0]:
                time.sleep(0.05)

    def loop_thread():
        with loop.recorder(samplerate=FS, channels=1, blocksize=BLOCK) as r:
            ready.wait()
            n = 0
            while time.perf_counter() < stop[0]:
                b = r.record(numframes=BLOCK)
                t = time.perf_counter()
                n += len(b)
                data["out"].append(b[:, 0].copy())
                arrivals["out"].append((t, n))

    def on_press(k):
        if k == keyboard.Key.space:
            keys.append(time.perf_counter())

    threads = [threading.Thread(target=f, daemon=True) for f in (mic_thread, loop_thread)]
    for th in threads:
        th.start()
    ready.wait()
    stop[0] = time.perf_counter() + a.seconds
    print(f"recording {a.seconds:.0f} s: tap the space bar or speak", flush=True)
    with keyboard.Listener(on_press=on_press) as listener:
        while time.perf_counter() < stop[0]:
            time.sleep(0.05)
        listener.stop()
    for th in threads:
        th.join(timeout=2)

    x, start = {}, {}
    for name in data:
        x[name] = np.concatenate(data[name])
        arr = np.array(arrivals[name])
        start[name] = np.min(arr[:, 0] - arr[:, 1] / FS)

    # Presses: auto-repeat (a held key) is dropped; onset = first 1 ms envelope over the noise.
    presses = [k for i, k in enumerate(keys) if i == 0 or k - keys[i - 1] > 0.6]
    for name in data:
        env = np.convolve(np.abs(np.diff(x[name], prepend=0)), np.ones(48) / 48, mode="same")
        out = []
        for kt in presses:
            k0 = int((kt - start[name] - 0.05) * FS)
            k1 = int((kt - start[name] + 0.6) * FS)
            if k0 < 0 or k1 > len(env):
                out.append("-")
                continue
            base = env[k0 : k0 + int(0.04 * FS)]
            w = env[k0 + int(0.04 * FS) : k1]
            hit = np.argmax(w > base.mean() + 8 * base.std())
            out.append("-" if not w[hit] > base.mean() + 8 * base.std() else f"{((k0 + int(0.04 * FS) + hit) / FS + start[name] - kt) * 1000:.0f}")
        print(f"key -> {name} onset (ms): {' '.join(out)}")

    t0 = max(start.values()) + 1
    length = 4 * FS
    for s in range(0, int(a.seconds) - 6, 5):
        i = int((t0 + s - start["mic"]) * FS)
        j = int((t0 + s - start["out"]) * FS)
        m = np.diff(x["mic"][i : i + length])
        o = np.diff(x["out"][j : j + length + FS // 2])
        if len(m) < length - 1 or len(o) < length:
            break
        if not (m**2).sum() or not (o**2).sum():
            print(f"window {s:2d} s: {'microphone' if not (m**2).sum() else 'output'} silent")
            continue
        r = correlate(o, m, mode="valid")
        k = int(np.argmax(np.abs(r)))
        c = abs(r[k]) / np.sqrt((m**2).sum() * (o[k : k + len(m)] ** 2).sum())
        print(f"window {s:2d} s: output behind the microphone {k / FS * 1000:.1f} ms (correlation {c:.2f})")

    if a.save:
        np.savez(a.save, keys=np.array(keys), mic=x["mic"], out=x["out"], start=np.array([start["mic"], start["out"]]))


if __name__ == "__main__":
    main()
