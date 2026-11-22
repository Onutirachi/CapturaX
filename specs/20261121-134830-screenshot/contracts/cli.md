# Contract: Command Line (additions)

Adds to the device preview's `contracts/cli.md`.

| Flag | Build | Meaning |
|---|---|---|
| `--screenshot-every <ms>` | debug only | takes a screenshot every `<ms>` ms while the preview shows an image, as the key would (SC-003 measurement, research R16); 0 or absent: off |

Each screenshot logs one line at info level:

```text
screenshot 3: 3840x2160 taken +14 ms, rgb 38 ms, png 312 ms, written 9 ms, copied 120 ms, saved and copied
```

`taken` is from the request to the frame's hand-off; the others are each step's own time.
