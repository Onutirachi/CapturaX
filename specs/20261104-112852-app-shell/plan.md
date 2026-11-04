# Implementation Plan: App Shell

**Branch**: `feat/20261104-112852-app-shell` | **Date**: 2026-11-04 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/20261104-112852-app-shell/spec.md`

## Summary

The first application code: the CapturaX window every later feature lives in. It ports the stack
spike's validated render architecture (tag `spike/stack-20261026`, `layered-preview`) into a
maintained structure: a winit window and the Slint UI on the main thread, a video thread that owns
wgpu and presents each frame on arrival, compositing the last finished UI image (research R3).
Around it: the Float frameless window and chrome, the theme and both languages switching live,
the portable settings file with fallbacks, single instance, tray and autostart, the empty and
error state components, a test video with presentation statistics for the latency budget, and CI
on Windows and Linux.

## Technical Context

**Language/Version**: Rust 1.99, edition 2024 (research R1)

**Primary Dependencies**: Slint 1.18 (software renderer, bundled translations), winit 0.30,
wgpu 30, gstreamer-rs 0.25 (test video only); full list and licenses in research R14

**Storage**: one TOML file `capturax.toml` plus `capturax.log`, in the application folder, else
the per-user configuration folder, else memory (research R6, contracts/settings-file.md)

**Testing**: `cargo test`: unit tests (settings, placement, i18n and theme resolution, shadow
generation, stats), integration tests (settings interruption SC-007, i18n catalog FR-039, contrast
FR-018); manual acceptance and latency measurement in quickstart.md

**Target Platform**: Windows 11 (Direct3D 12) and Linux (Vulkan; KDE Plasma Wayland checked, X11
supported)

**Project Type**: desktop app (Cargo workspace)

**Performance Goals**: frame ready → presented median ≤ 1.5 ms, max ≤ 3 ms; 59–61 presents per
second at 60 fps in ≥ 99% of seconds; window visible within 2 s; language and theme switch
< 100 ms (SC-001, SC-002, SC-004, SC-005)

**Constraints**: the video never waits for the UI or the window's thread; no network traffic;
no OS call outside `crates/platform`; every user-facing text in the catalogs; CI under 20 min

**Scale/Scope**: one window, one settings panel, ~10 Slint components, two languages, two themes

No NEEDS CLARIFICATION remains: research.md resolves every open point, including the spike's four
open questions for this spec (R5 snap flyout, fast moves accepted as a known limit, R8 shadows at scale, and the small
window bugs: hover buttons and menus in a top layer, menus flipping side when they do not fit,
UI structure below).

## Constitution Check

*Constitution 1.1.0. Checked before research and again after design: passes, no deviations.*

| Principle | How the plan complies |
|---|---|
| I. Preview latency | Render loop from the spike's fastest path (DX12/Vulkan + Mailbox); latency budget SC-001 measured by the app's own stats (contracts/stats.md, quickstart.md); UI never on the video thread (R3) |
| II. Simple by default | Defaults need no configuration (Automatic language and theme); settings panel has only the five spec'd settings; explicit "no source" state and notices with next action (data-model.md) |
| III. One native pipeline | One process; the test video goes through GStreamer and an appsink into the app's own GPU presenter, as the constitution allows for the preview |
| IV. Platform layer | `crates/platform` is the only crate with OS APIs and OS crates; `cargo deny` bans them elsewhere (R2, R13); degraded behavior defined for Wayland raise, no tray, no theme portal, Linux window icon |
| V. Private and safe | No network code; the log is local; the single-instance channel is local and accepts one fixed request (contracts/single-instance.md) |
| VI. Portable | Settings and log next to the executable (AppImage folder on Linux), per-user fallback with a notice, memory-only last resort (R6) |
| VII. Spec-driven, tested | Spec approved; unit and integration tests for all pure logic; manual acceptance listed; every dependency justified (R14); spike code adopted only where this plan names it |
| Technology constraints | Rust, GStreamer, Slint with the app owning window and swap chain (constitution's condition); i18n English + pt-BR from day one; brand assets from `brand/`, accent `#FF7304` |
| Workflow | English everywhere; Conventional Commits without AI trailers; CI on both platforms gates merges (R13) |

## Design

### Startup sequence

1. Parse the command line (contracts/cli.md).
2. Resolve the settings location and load settings (R6); start the logger there.
3. Claim the single instance (R10); if another runs, hand over and exit.
4. Set the Slint platform, create the UI component, select the language (R7) and theme (R8).
5. Create the winit window hidden at the restored placement (data-model.md); create the surface
   and start the video thread (R4). On failure: system message `renderer-unavailable`, exit 2.
6. Apply the start mode (fullscreen, windowed, or hidden in tray with `--autostart`), show the
   window; show any pending notice; start the test video if asked.

### Quit sequence (FR-014)

Stop the test video pipeline → stop and join the video thread → flush settings → drop the tray
and the single-instance listener → destroy the window.

### UI structure (`app/ui/`)

- `main.slint`: the window root. Layers from bottom to top: video area (reported to Rust as
  position, size and radius), state message, chrome (toolbar slot, left and right docks), notices,
  settings panel, then one top-level layer for menus, dropdowns, tooltips and the info bubbles'
  hover buttons. Drawn last, they are above everything, so no dock can cover a hover (FR-010; the
  bubbles themselves arrive in spec #6 and put their hover buttons in this layer).
- Menu placement (FR-010), as context menus in Windows or VS Code: a menu opens on its default
  side (below and aligned to the left of its control). If it would not fit inside the window on
  that side, it opens on the other side (to the left instead of the right, above instead of
  below). Nothing more.
- `theme.slint` (`Theme`), `texts.slint` (`Texts`: every text Rust needs, by identifier),
  `window-state.slint` (window actions and state between Rust and the UI).
- `components/`: `title-bar`, `dock`, `icon-button`, `menu` (flips side when it does not fit,
  above), `tooltip`, `state-message`, `notice`, `settings-panel`,
  `toggle`, `select`, `focus-ring`.
- Layout: Float at 800 × 520 and above; compact below it (docks stack into the toolbar's row, no
  overlaps, FR-009). Chrome hide timer 2 s (FR-008). Keyboard focus order and focus ring on every
  control (FR-020).
- Docks hold only fullscreen and settings (FR-015); the toolbar slot stays empty for spec #3.

## Project Structure

### Documentation (this feature)

```text
specs/20261104-112852-app-shell/
├── spec.md
├── plan.md              # this file
├── research.md          # R1–R14
├── data-model.md
├── quickstart.md
├── contracts/
│   ├── cli.md
│   ├── settings-file.md
│   ├── stats.md
│   ├── single-instance.md
│   └── platform.md
├── checklists/requirements.md
├── handoff.md
├── measurements/        # latency and acceptance results (created during implementation)
└── tasks.md             # /speckit-tasks
```

### Source Code (repository root)

```text
Cargo.toml                 # workspace: app, crates/settings, crates/platform
rust-toolchain.toml        # 1.99, rustfmt, clippy
rustfmt.toml               # max_width = 120 (as the spike)
deny.toml                  # allowed licenses; OS crates only under platform
.cargo/config.toml         # PKG_CONFIG_PATH for GStreamer on Windows
.github/workflows/ci.yml

app/                       # binary `capturax`
├── Cargo.toml
├── build.rs               # compile ui/main.slint with bundled translations; rasterize brand/X.svg
├── src/
│   ├── main.rs            # startup and quit sequences
│   ├── cli.rs
│   ├── app.rs             # winit ApplicationHandler: events, window actions, resize band, keys
│   ├── ui.rs              # Slint platform, event forwarding, UI image hand-off, Rust ↔ UI bridge
│   ├── render/
│   │   ├── mod.rs         # video thread, inbox (mailbox), scene
│   │   ├── gpu.rs         # presentation choice, surface, layers, composite
│   │   └── composite.wgsl # letterbox, rounded video corners, UI over video
│   ├── stats.rs
│   ├── test_video.rs      # GStreamer videotestsrc → appsink
│   ├── placement.rs       # window placement fit (pure)
│   ├── i18n.rs            # language list and resolution (pure)
│   ├── theme.rs           # theme resolution (pure)
│   ├── shadows.rs         # nine-slice shadow generation at scale (pure)
│   └── log.rs             # size-limited file logger
├── ui/                    # .slint files and assets (Inter, Tabler icons)
├── lang/pt-BR/LC_MESSAGES/capturax.po
└── tests/
    ├── i18n.rs            # catalog completeness, no untranslated literals (FR-039)
    └── contrast.rs        # 4.5:1 text pairs in both themes (FR-018)

crates/settings/           # format, lenient load, unknown values, atomic save, location
├── src/lib.rs
└── tests/interrupted_save.rs   # SC-007

crates/platform/           # OS-specific code only (contracts/platform.md)
└── src/{lib.rs, windows/, linux/}

brand/                     # logos and app icon (existing)
tools/latency/             # spike measurement tools (existing)
```

**Structure Decision**: a three-crate Cargo workspace (research R2). `app` holds everything that
is not OS-specific and not the settings file; `crates/settings` is pure and fully tested;
`crates/platform` is the platform layer of principle IV. Spike code from `layered-preview`
(`gpu.rs`, `shader.wgsl`, the Slint platform and event forwarding in `main.rs`, `window.rs`, the
Float `.slint` components, theme and texts) is adopted by this plan as the starting point and
rewritten into this structure; the ambient light, info bubbles, dropdown contents and audio stay
in the tag for their own specs.

## Complexity Tracking

No constitution violations to justify.
