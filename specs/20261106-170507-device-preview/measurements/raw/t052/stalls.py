"""T052: match the app's present log with PresentMon's CSV by QPC and list stalls over 50 ms.

python stalls.py <run>-app.tsv pm-<time>.csv [threshold_ms]
"""
import csv
import sys

app_path, pm_path = sys.argv[1], sys.argv[2]
limit = float(sys.argv[3]) if len(sys.argv) > 3 else 50.0

presents, events, stages = [], [], []
for line in open(app_path, encoding="utf8"):
    f = line.rstrip("\n").split("\t")
    if f[0] == "present":
        presents.append(int(f[1]))
    elif f[0] == "event":
        events.append((int(f[1]), f[2]))
    elif f[0] == "stage":
        stages.append((int(f[1]), f[2], float(f[3])))

rows = list(csv.DictReader(open(pm_path, encoding="utf-8-sig")))
rows = [r for r in rows if r["Application"].lower() == "capturax.exe"]
# QPC frequency from PresentMon's own columns (seconds vs counter).
a, b = rows[0], rows[-1]
freq = (int(b["QPCTime"]) - int(a["QPCTime"])) / (float(b["TimeInSeconds"]) - float(a["TimeInSeconds"]))
freq = round(freq / 1e5) * 1e5
t0 = events[0][0] if events else presents[0]


def t(q):
    return (q - t0) / freq


def near(q):
    """Last event at or before q (label, ms since), or the first one after if within 100 ms."""
    before = [(e, l) for e, l in events if e <= q]
    after = [(e, l) for e, l in events if e > q]
    if after and (after[0][0] - q) / freq * 1000 < 100:
        e, l = after[0]
        return f"{l} in {(e - q) / freq * 1000:.0f} ms"
    if before:
        e, l = before[-1]
        return f"{l} {(q - e) / freq * 1000:.0f} ms before"
    return "-"


# Event labels with ordinals: focus=false #n etc.
print(f"qpc freq {freq:.0f}; {len(presents)} app presents, {len(rows)} PresentMon rows, "
      f"{len(events)} events; span {t(presents[-1]):.1f} s")

print(f"\n## App presents: gaps over {limit:.0f} ms (present return to present return)")
for p, q in zip(presents, presents[1:]):
    gap = (q - p) / freq * 1000
    if gap > limit:
        print(f"  t={t(p):7.2f}s gap={gap:6.1f} ms  after {near(p)}")

print(f"\n## PresentMon: msBetweenPresents or msBetweenDisplayChange over {limit:.0f} ms")
modes = {}
prev = None
for i, r in enumerate(rows):
    q = int(r["QPCTime"])
    modes[r["PresentMode"]] = modes.get(r["PresentMode"], 0) + 1
    bp = float(r["msBetweenPresents"])
    bd = float(r["msBetweenDisplayChange"])
    if bp > limit or bd > limit:
        before = rows[i - 1]["PresentMode"] if i else "-"
        after = rows[i + 1]["PresentMode"] if i + 1 < len(rows) else "-"
        print(f"  t={t(q):7.2f}s betweenPresents={bp:6.1f} betweenDisplay={bd:6.1f} "
              f"inPresent={float(r['msInPresentAPI']):5.1f} untilDisplayed={float(r['msUntilDisplayed']):6.1f} "
              f"mode {before} -> {r['PresentMode']} -> {after} tearing={r['AllowsTearing']} "
              f"dropped={r['Dropped']}  [{near(q)}]")

print("\n## PresentMode changes")
last = None
for r in rows:
    m = r["PresentMode"]
    if m != last:
        q = int(r["QPCTime"])
        print(f"  t={t(q):7.2f}s {m}  [{near(q)}]")
        last = m
print("\n## PresentMode counts", modes)

print("\n## Events")
for e, l in events:
    print(f"  t={t(e):7.2f}s {l}")
if stages:
    print("\n## Language stages")
    for e, l, ms in stages:
        print(f"  t={t(e):7.2f}s {l} {ms} ms")
