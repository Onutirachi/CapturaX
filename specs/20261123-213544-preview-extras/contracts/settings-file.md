# Contract: Settings File (additions)

Adds to the [screenshot's settings file](../../20261121-134830-screenshot/contracts/settings-file.md).
Its rules (lenient read, unknown keys kept, atomic save) apply unchanged. There is no version
bump: two keys and one table are new.

```toml
[preview]
ambient = false        # the ambient light (FR-001); applies to every device
hidden = false         # the preview is hidden (FR-011); a start while true opens no device

[overlay.bubbles]
# Each corner's bubbles in reading order: left to right in a row, top to bottom in a column.
top-left = []
top-right = []
bottom-left = []
bottom-right = ["source", "mode", "fps", "latency"]
pinned = []            # kinds shown also while the controls are hidden
```

Rules:

1. A missing or non-boolean `ambient` or `hidden` reads as `false`.
2. Kinds: `source`, `mode`, `fps`, `latency`. Specs #9 and #10 add `recording` and `live`.
3. In the corner lists:
   - an unknown kind is dropped (logged);
   - a kind listed twice keeps its first place;
   - a kind in no list is appended to `bottom-right`, in the order of rule 2.
4. `pinned` keeps only known kinds. Order does not matter, and duplicates collapse.
5. A missing `[overlay.bubbles]` table reads as the defaults above, which is today's layout.
   **Reset** (FR-016) writes the defaults.
6. The existing `[overlay]` keys (`latency`, `fps`, `mode`) are unchanged.
