"""T052: keep the D: HDD busy: 4 MiB writes with fsync and random 64 KiB reads, for N seconds.
python -I stress.py <folder on D:> <seconds>"""
import os, random, sys, time
folder, seconds = sys.argv[1], float(sys.argv[2])
os.makedirs(folder, exist_ok=True)
path = os.path.join(folder, "stress.bin")
chunk = os.urandom(4 << 20)
end = time.time() + seconds
n = 0
with open(path, "wb+", buffering=0) as f:
    while time.time() < end:
        f.seek((n % 256) * len(chunk))
        f.write(chunk)
        os.fsync(f.fileno())
        n += 1
        size = min(n, 256) * len(chunk)
        for _ in range(8):
            f.seek(random.randrange(0, size - 65536))
            f.read(65536)
print(f"{n} writes of 4 MiB")
os.remove(path)
