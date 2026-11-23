# Contract: Settings File (additions)

Adds to the [preview audio's settings file](../../20261113-103922-preview-audio/contracts/settings-file.md);
its rules (lenient read, unknown keys kept, atomic save) apply unchanged. No version bump: two new
tables.

```toml
[screenshot]
folder = "default"                 # or an absolute path (FR-020, FR-022)

[hotkeys]
screenshot = "Ctrl+Shift+F8"             # a key (data-model.md "Hotkey") or "" for none (FR-018)
```

Rules:

1. A missing `folder`, an empty or relative path, or a non-text value reads as `"default"`.
2. A missing hotkey takes its default; `""` is kept as no key; an invalid or not `allowed()` text
   takes the default (logged).
3. Two actions with the same key: the first in the table keeps it, the others read as no key
   (logged) (data-model.md "Hotkey binding").
4. Specs #9 and #10 add `record` and `live` to `[hotkeys]`.
