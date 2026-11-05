# Quickstart: App Shell

How to build the shell and check it against the spec. Options are in
[contracts/cli.md](contracts/cli.md), the stats format in [contracts/stats.md](contracts/stats.md).

## Prerequisites

- Rust from `rust-toolchain.toml` (rustup installs it on first build).
- GStreamer 1.28 development files, only for `--test-video`:
  - Windows: MSVC x86_64 installer with the development components; `.cargo/config.toml` points
    `PKG_CONFIG_PATH` at `C:\Program Files\gstreamer\1.0\msvc_x86_64\lib\pkgconfig` and the
    `bin` folder must be on `PATH` to run.
  - Linux (CachyOS): `gstreamer gst-plugins-base` (Ubuntu: `libgstreamer1.0-dev
    libgstreamer-plugins-base1.0-dev`), plus `libxkbcommon-dev libwayland-dev` on Ubuntu.
- For SC-001 to SC-003: the maintainer's machines (findings, environment).

## Automated checks (what CI runs)

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test            # settings (incl. 100 interrupted saves, SC-007), placement, i18n catalog (FR-039), contrast (FR-018)
cargo build --release
cargo deny check licenses bans
```

Expected: all pass on Windows and Linux. A `.slint` text without `@tr`, or a msgid missing from
`app/lang/pt-BR/LC_MESSAGES/capturax.po`, makes `cargo test` fail naming the text.

## Manual acceptance (SC-009)

Run from a writable folder with the release build: `target/release/capturax`.

| # | Check | Spec |
|---|---|---|
| 1 | Window appears within 2 s with the "no source" state, in the system language and theme | SC-004, US4.1, US2.1 |
| 2 | Move by the title bar, double click maximize/restore, maximized window restores under the pointer, resize all edges and corners down to 640 × 360 | US1.1, US1.2 |
| 3 | F11 and the dock's control toggle fullscreen; Esc closes an open menu first, then leaves fullscreen, never quits | US1.3, US1.4 |
| 4 | Chrome hides 2 s after the pointer leaves, returns at the top of the video | US1.6 |
| 5 | Below 800 × 520: compact layout, no overlaps; menus and tooltips stay inside the window; hovers not covered | US1.7 |
| 6 | Windows 11: snap by dragging and Win+arrows, half of a 1080p screen; flyout on maximize (best effort) | US1.11, FR-011 |
| 7 | Settings panel: language and theme switch at once (open menu and tooltip included); Automatic follows a system theme change | US2.2, US2.3, SC-005 |
| 8 | Change settings, restart: kept; `capturax.toml` is next to the executable | US3.1 |
| 9 | Copy the folder elsewhere, start: copied settings used | US3.2 |
| 10 | Read-only folder: notice naming the per-user folder; changes survive a restart | US3.3 |
| 11 | Replace `capturax.toml` with garbage: starts with defaults, notice, `.damaged-*` copy kept | US3.4 |
| 12 | Start in fullscreen on → starts fullscreen; `--windowed` overrides; `--fullscreen` overrides off | US1.8 |
| 13 | Keep in tray on: close hides to tray; click shows; Quit quits; a second start shows the hidden window | US1.9, FR-013 |
| 14 | Start with the system + tray on: sign out/in → only the tray icon; tray off → window. Remove the entry in Task Manager / autostart settings → panel shows off | US1.10, FR-014b |
| 15 | Second start while open: running window comes to the front (Wayland: message "already open") | FR-013 |
| 16 | 150% or 200% scale, and moving between monitors of different scale: shadows, icons, logo, text sharp | FR-019 |
| 17 | Disconnect the monitor the window was on, start: opens fully visible on the primary | FR-012 |
| 18 | Tab through every control except the title bar's window buttons: focus ring visible, all usable by keyboard | FR-020 |

Do 1–18 on Windows 11 and on CachyOS KDE Plasma Wayland, at 100% and at 150% or 200%.

## Latency budget (SC-001 to SC-003)

```sh
capturax --test-video 1080p60 --stats
```

On a 1080p screen and on a 1440p screen (NVIDIA DSR 1.78x on a 1080p monitor), windowed and
fullscreen, in each interface state (idle, chrome showing and hiding, a
menu opening and closing, language switch, theme switch), record about 30 s of `[loop]` lines:

- SC-001: `ready_ms` median ≤ 1.5 ms and max ≤ 3 ms in every line.
- SC-002: at least 99% of lines with `pres` between 59 and 61 (window still).
- SC-003: hold the window still mid-drag and resize slowly → `pres` stays 59–61, `gap_ms` near
  16.7. Fast moves: the window moves smoothly and the video never stalls (no `gap_ms` of hundreds
  of ms); a lower `pres` during the move is accepted.
- The `[present]` header shows `dx12 mailbox` on Windows and `vulkan mailbox` on Linux (FR-003).
- On Linux, note the GPU clock state (`nvidia-smi -q -d PERFORMANCE`): the spike saw the RTX 3050
  power state add up to 6 ms; keeping the GPU awake belongs to spec #3, but the readings must say
  which state they were taken in.

Results go in `specs/20261104-112852-app-shell/measurements/`.

## Repository rule (FR-038, once)

The maintainer adds a rule on `main` in GitHub (Settings → Rules) requiring the `checks` and
`build-test` jobs of `ci.yml` to pass before merging.
