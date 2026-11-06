# Contract: Single Instance

One running instance per settings location (FR-013).

## Channel

- Name: `capturax-<FNV-1a 64-bit hash of the settings location path, 16 hex digits>`; with
  settings in memory, of the executable path. FNV-1a is written in a few lines and stable across
  Rust versions (unlike `DefaultHasher`); it needs no cryptographic strength here. The path is
  resolved before hashing: its folder made canonical (so a settings file that does not exist yet
  gives the same name as once it does) and, on Windows, lowercased (paths there ignore case, and
  a start through a differently written path must find the same instance).
- Windows: named pipe `\\.\pipe\<name>`, created by the first instance with
  `FILE_FLAG_FIRST_PIPE_INSTANCE`; a second creation fails, which means another instance runs.
  One pipe instance, remote clients rejected, default security (only the owner, SYSTEM and
  administrators can write); the second start opens it with identification-only security.
- Linux: abstract Unix stream socket `\0<name>`; `bind` fails with `AddrInUse` when another
  instance runs. Released by the kernel when the process ends.

## Messages

UTF-8 lines, one request and one answer, 1 s timeout on the second start's side.

| Request | Answer | Running instance does | Second start does |
|---|---|---|---|
| `show` | `raised` | Shows the window (also from the tray) and focuses it | Exits 0 |
| `show` | `shown-not-raised` | Shows the window; the platform did not let it take focus (Wayland) | Shows system message `already-open` (a warning, not an error: "CapturaX is already open."), exits 0 |
| (no answer) | — | — | Logs it and exits 0 (never starts a second instance) |

The second start hands over before it writes anything (no settings file is created or saved).
If the channel cannot be created for another reason than a running instance, the app logs it and
starts without the single instance.

The listener accepts only these lines; anything else closes the connection. It never acts on any
other input (constitution V: no remote control, and this channel is local only).

## Lifetime

The name is held from the claim, before the listener starts. Stopping the listener (quit
sequence) wakes it with a connection of its own, waits up to 1 s for its thread and releases the
name; a thread stuck on a client that never writes (Windows pipe reads have no timeout) is left
to the process exit. On Linux a connected client has 1 s to send its line.
