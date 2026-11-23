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
The end is `saved and copied`, `saved, not copied (<clipboard error>)`, `not saved (<reason>),
copied` or `not saved (<reason>), not copied (<clipboard error>)`; `written` is missing when the
PNG could not be made (reason `ImageError`). A request that takes no frame logs
`screenshot: not taken (…)` or `screenshot N: no frame` instead (FR-008).
