# Contract: Single Instance

One running instance per settings location (FR-013).

## Channel

- Name: `capturax-<FNV-1a 64-bit hash of the settings location path, 16 hex digits>`; with
  settings in memory, of the executable path. FNV-1a is written in a few lines and stable across
  Rust versions (unlike `DefaultHasher`); it needs no cryptographic strength here.
- Windows: named pipe `\\.\pipe\<name>`, created by the first instance with
  `FILE_FLAG_FIRST_PIPE_INSTANCE`; a second creation fails, which means another instance runs.
- Linux: abstract Unix stream socket `\0<name>`; `bind` fails with `AddrInUse` when another
  instance runs. Released by the kernel when the process ends.

## Messages

UTF-8 lines, one request and one answer, 1 s timeout on the second start's side.

| Request | Answer | Running instance does | Second start does |
|---|---|---|---|
| `show` | `raised` | Shows the window (also from the tray) and focuses it | Exits 0 |
| `show` | `shown-not-raised` | Shows the window; the platform did not let it take focus (Wayland) | Shows system message `already-open`, exits 0 |
| (no answer) | — | — | Logs it and exits 0 (never starts a second instance) |

The listener accepts only these lines; anything else closes the connection. It never acts on any
other input (constitution V: no remote control, and this channel is local only).
