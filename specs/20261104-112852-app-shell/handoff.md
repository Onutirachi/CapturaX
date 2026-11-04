# Handoff: App Shell

**Updated**: 2026-11-04 | **Spec**: [spec.md](spec.md) | **Branch**: `feat/20261104-112852-app-shell`

## State

Spec, plan and tasks approved and committed. Implementation approved by the maintainer
(2026-11-04). Phase 1 committed (`build: add cargo workspace`), Phase 2 committed
(`feat: add window, render loop and settings store`). T060 and T061 (CI guard) written and
waiting for the maintainer's validation before the commit `ci: add platform boundary test and
workflow`.

T060: `app/tests/platform_boundary.rs` passes (4 tests; the parsers have their own tests:
renamed and target-specific dependencies, `cfg(not(windows))`, `cfg_attr`, `cfg!`, comments).
T061: `.github/workflows/ci.yml` written but **never run**: it runs for the first time when the
branch is pushed (T062), so T061 stays unchecked until it passes there. Locally on Windows:
`cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test
--workspace` (48 tests) and `cargo deny check licenses bans` pass. The Linux build of `app` is
still unchecked (CI or CachyOS).

## Done in the last sessions

- `/speckit-specify`: spec.md and checklists/requirements.md (all items pass).
- Clarifications: minimum window 640 × 360 with a compact layout below Float's 800 × 520
  (FR-009); Windows 11 snap layouts flyout best effort, dropped if fragile (FR-011); settings as a
  panel inside the main window, no separate windows (FR-025).
- FR-006 now names the brand assets: `brand/CapturaX-light.svg` (white text) in the dark theme,
  `brand/CapturaX-dark.svg` (black text) in the light theme, `brand/X.svg` for the window,
  taskbar and tray icon. `brand/` is committed with the spec.
- Out of Scope now points global hotkeys to F50 and the tray / taskbar status to F51.
- Product decisions outside this spec went to the stacked branch `docs/outputs-and-hotkeys`
  (based on this branch): constitution 1.1.1 "one source per output"; recording and streaming
  as two independent outputs in separate Record and Stream panels inside the main window, as
  the settings panel (F15, F16, F25-F27); hotkeys Ctrl+F8 screenshot, Ctrl+F9 stream, Ctrl+F10
  record, in the window and global, configurable, conflicts not handled (F19, F20, F50); tray
  icon and taskbar badge showing recording and streaming status (F51). Not pushed.

## Decisions

- T060/T061: `cargo deny check bans` was already failing after Phase 2: Slint's default features
  bring `ksni` (via `i-slint-core`, feature `system-tray`) and `muda` (via
  `i-slint-backend-winit`). Those two Slint crates are now allowed wrappers in `deny.toml`.
  Turning Slint's default features off fails: without a Slint backend, `.slint` SVG images do
  not compile. GStreamer 1.28 on Windows ships one Inno Setup `.exe` (no MSI), installed in CI
  with `/VERYSILENT /NORESTART /ALLUSERS /TYPE=devel`; Ubuntu also needs `libfontconfig1-dev`.
  All recorded in research R13 and T061.
- Phase 2: the presentation order is derived from the wgpu backends the build has (DX12 → the
  Windows order, else the Linux order) instead of `cfg(windows)`, so `app/src` stays free of
  platform `cfg` (T060). `settings::LoadOutcome::Damaged { kept_as }` is an `Option<PathBuf>`
  (the rename can fail). A first start (no file) or a damaged file writes the current values at
  once (`Store::save_soon`, recorded in data-model.md "Saving"). `KNOWN_LANGUAGES` in
  `app/src/main.rs` is temporary until `i18n::LANGUAGES` (T042). Things not wired yet by design:
  window placement (T032), notices pushed (US3), window actions beyond the basics (T030), the
  translated renderer message (T058).
- Phase 1: Slint 1.18.1 is still the newest 1.x (research R1). `zbus = "5"` because ksni 0.3
  uses zbus 5 (one copy in the build; zbus 6 exists). `windows-sys` 0.61 with the features the
  platform tasks need, declared once in the workspace. `deny.toml` has
  `unused-allowed-license = "allow"` because later specs bring the remaining licenses; the
  `tray-icon` ban was checked by adding it to `app` (fails as expected).
- While writing the tasks: `cargo deny` cannot ban `windows-sys` and `zbus` by name (winit, wgpu
  and Slint pull them in), so a test `app/tests/platform_boundary.rs` checks the direct
  dependencies of `app` and `settings` instead (research R2, R13 and plan updated).
- From `/speckit-analyze`: winit's Windows and Linux window extensions (undecorated shadow, app
  id) go through `platform::window_attributes`, so `platform` depends on winit (contracts/platform.md);
  Esc closes a menu, else the settings panel, else fullscreen (FR-007); language and theme switch
  times are logged for SC-005; the interrupted-save test kills only after the first save; wgpu is
  built without GL on Windows so a forced missing backend always fails (renderer failure check).
  Left open, low: no check catches a user-facing literal in Rust (C4); refresh-rate change between
  monitors is not in the quickstart (C5).

- All recorded in spec.md, plan.md, research.md, docs/product.md and the constitution; none live
  only here. Main plan choices: three-crate workspace (`app`, `crates/settings`,
  `crates/platform`); UI drawn on the main thread, video thread owns wgpu (spike R15); TOML
  settings `capturax.toml` with lenient load and atomic replace; shadows generated at the display
  scale; test video through GStreamer `videotestsrc`; tray `tray-icon` (Windows) / `ksni` (Linux).

## Open questions

- None in the spec.
- The record / stream only mode may change the product's main scenario and the roadmap's order;
  product.md asks to decide it before spec #3 starts.

## Next step

Maintainer validates T060/T061; commit `ci: add platform boundary test and workflow`; then
Phase 3 (US1), starting with the parallel tasks T021, T022, T024, T025 (tasks.md, Parallel
Example). The first CI run happens when the maintainer asks to push (T062).
