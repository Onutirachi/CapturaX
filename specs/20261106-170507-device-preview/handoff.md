# Handoff: Device Preview

**Updated**: 2026-11-06 | **Spec**: [spec.md](spec.md) | **Branch**: `feat/20261106-170507-device-preview`

## State

Spec, plan and tasks approved and committed. Implementation started (2026-11-06):

- **Phase 1 (T001–T004)** committed: `build: add capture crate` (38c843d).
- **Phase 2 (T005–T016)** validated by the maintainer (test video and `--list-devices` on
  Windows) and committed: `feat: add capture crate and yuv render path`.

Nothing pushed.

## Done in the last session

- Phase 1: crate skeleton, boundary test over `crates/capture` (+ `libloading` forbidden outside
  platform), CI installs `gstreamer1.0-plugins-base`; the libloading ban in deny.toml was left
  out (wgpu-hal, ash, glutin, dlib already depend on it). Caps recorded:
  `crates/capture/tests/data/c920-windows.txt`, `ezcap-linux.txt` (`ezcap-windows.txt` when the
  ezcap is at the Windows PC).
- Phase 2: `frame.rs` (planes, pool, `captured_at`), `mode.rs` (modes, automatic modes, choice
  text, badges), `device.rs` (identity, numbering, start order), `analysis.rs` (range detector),
  `monitor.rs`, `session.rs` + `test_source.rs` (appsink copying planes into pooled frames),
  settings `[preview]`, `[overlay]`, `[devices."<id>"]`, YUV composite (NV12, I420, YUY2, UYVY,
  BGRx, RGBA; matrix and range as uniform rows), the test video through the capture crate in
  NV12 (`app/src/test_video.rs` deleted, `app` no longer depends on the gst crates),
  `--list-devices`.
- Checks: `cargo fmt`, `clippy -D warnings`, `cargo test` pass on Windows and on CachyOS.
  `--list-devices` lists the C920 (Auto max resolution → 1080p30 NV12; YUY2 where it beats the
  Frame Server's NV12) and the ezcap on Linux (Auto max FPS → 1080p120 NV12). Test video
  1080p60: Windows `ready_ms` 0.6–0.7 median, max ≤ 1.1 after warm-up; Linux 0.7 at first, then
  2.6 after a few idle seconds (GPU clocks dropping: what T024–T026 address).
- Implementation notes (not spec changes): `modes_from_caps(caps, declared_full_nv12)` gets
  `device.api == "mediafoundation" && nv12_declared_full(caps)` from the monitor; `Playing` is
  sent when the appsink receives its caps (the device accepted the format), so the no-signal
  timer (T042) starts there; `Device` has `base_name` (for renumbering) and `gst:
  Option<gst::Device>` (`None` only in tests); a decoded MJPG mode goes through `videoconvert`
  (passthrough when the decoder gives an uploaded format).
- The Linux clone (`~/Projetos/CapturaX`) is on a detached commit made from the working tree
  (`refs/tmp/wip`); its old app-shell copies were stashed ("pre-device-preview copies").

## Decisions

- All recorded in spec.md and research.md. Main plan choices: new crate `crates/capture`; YUV
  planes uploaded as they come and converted in the shader (range switches live); Auto source not
  shown on UVC cards (none reports its input, R3) but kept for devices that do; the default is
  Auto max FPS (maintainer, 2026-11-06; 1080p120 on the ezcap, whose cost with a 60 Hz source is
  measured, L3); a card without a serial moved to another port is a new device (maintainer); Linux GPU kept awake through
  NVML PowerMizer, per process, no root (R7, to validate first); Windows stalls investigated with
  PresentMon and a present log before any fix (R8); a G-SYNC driver profile would be a maintainer
  decision.

- **No HDCP hint** (maintainer, 2026-11-06): FR-020 removed. Black video with a signal also comes
  from a console that is off, RCA adapters and VHS tapes, so it is not a reliable sign of HDCP; no
  black-image detection. HDCP, color range, modes and the bubbles go into a getting started guide
  (product.md F54, roadmap #15, last spec). Spec, research R5, data-model, contracts, quickstart,
  tasks, product F11 and roadmap #3 updated.

- **AMD and Intel GPUs** (maintainer, 2026-11-06): FR-012 is covered and measured on NVIDIA
  only; AMD and Intel are a known limit (no hardware, known controls need root), to be measured
  when the maintainer has one (spec FR-012 and Assumptions, research R7).

- **No switch back to the remembered device** (maintainer, 2026-11-06): when the app started on
  another device because the remembered one was missing, plugging the remembered one in only adds
  it to the menu; the user picks it. Reconnect of the open device (F4, FR-014) is unchanged.
  Spec edge case, data-model and T032 updated, with the other `/speckit-analyze` fixes (FR-018
  wording, US4 reference, T028/T033 references, SC-004 list timing in T034 and quickstart).

- **Device notices** (maintainer, 2026-11-06): reconnect (FR-014) kept; a notice says when a
  device is connected, reconnected or disconnected, so the user is not left waiting on a device
  that will not come back (for example the ezcap back on another USB port, a new identity). In
  the app when the window has focus, a system notification otherwise, **Open** on a newly
  connected device. FR-027, US4 scenario 6, SC-005, product F4, research R15 (Windows toast for a
  portable app via a per-user `AppUserModelId` key, validated first at T061; Linux freedesktop
  notifications through `zbus`), tasks T060–T063 inside Phase 6. New direct dependency: `windows`
  0.62 (already in the tree) in `crates/platform`. In-app notice: one balloon drawn by the app
  (no toast library), only the latest message, × button, closes on any click.

## Open questions

- R7 is untested on the CachyOS PC: if the NVML request does not hold the clocks, the fallback
  (small keep-alive GPU work) needs the maintainer's decision.
- Windows error mapping for "in use" vs "failed" is a heuristic (R6), tested in acceptance
  (quickstart US4: card open in another app); if it misreads, Windows shows the generic Failed
  state instead (maintainer, 2026-11-06), no Media Foundation code of our own.
- The ezcap's Windows identity (USB serial or port-based) is unknown until it is plugged into the
  Windows PC (`Get-PnpDevice`); without a serial its choices only follow it on the same port
  (accepted by the maintainer).

## Next step

1. `/speckit-implement` from T017 (Phase 3, US1). Linux GPU power (R7) is validated at T025 and
   system notifications (R15) at T061, each before it is wired in.
