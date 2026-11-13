# Handoff: Preview Audio

**Updated**: 2026-11-13 | **Spec**: [spec.md](spec.md) | **Branch**: `feat/20261113-103922-preview-audio`

## State

Spec, plan and tasks committed. Phase 1 (T001–T004) done and committed (`build: add audio
crate`): empty `crates/audio` wired into the app and the boundary test, `pipewire` 0.10 and the
WASAPI features in the workspace, PipeWire build packages in CI. Not pushed.

Phase 2 validations (2026-11-13, committed in `test: add the audio probe examples`):

- T005 written: `crates/platform/examples/pipewire_probe.rs`. **Not compiled**: this PC has no
  Linux toolchain (WSL Ubuntu has no Rust or PipeWire headers), so the first build is on
  CachyOS. Written against the `pipewire` 0.10.1 sources; it needs the crate feature `v0_3_49`
  (`target.object`, a playback buffer's `requested` size), now set in the workspace Cargo.toml.
  Signals: the main thread's own PipeWire loop holds the `SIGINT`/`SIGTERM` sources (spa blocks
  the signal in the adding thread, and `add_signal_local` asserts the thread is named `main`)
  and is set up before the `pipewire` thread starts, so that thread inherits the blocked mask.
- T007 done: `crates/platform/examples/audio_endpoints.rs`, run here (T008's Windows half,
  recorded in measurements/pairing.md): the ezcap and the C920 each show one container id on
  video and audio. Finding: virtual endpoints all report the null container id
  `{00000000-0000-0000-ffff-ffffffffffff}`; research R5 and data-model.md "Pairing" now say it
  is no link.

## Done in the last sessions

- PR #9 (device preview) squash-merged into `main` (2f4beea); this branch starts from it.
- spec.md and checklists/requirements.md; docs/roadmap.md #4 in progress with the spec link.
- Mockup page for the UI decisions (private Artifact):
  https://claude.ai/artifact/DWT9wMk8mY64QXU9woPy2n (version 2).
- Plan (2026-11-13), main decisions:
  - New pure crate `crates/audio` (bridge: ring, spike's rate follower with 2 ms margin, cubic
    interpolation, channel map, volume ramp; pairing; choices). Device streams in
    `crates/platform` (research R1, R13).
  - Windows: the spike's WASAPI engine, now in any device format (shared via `IAudioClient3`
    with the mix format, exclusive probing formats) (R2).
  - Linux: PipeWire directly with the `pipewire` 0.10 crate, one `pipewire` thread, both streams
    in one `node.group` (as module-loopback), `node.latency` 128/48000, format held explicitly;
    no PulseAudio layer unless the probe fails (R3).
  - Pairing by the system's link: Windows container id, Linux USB bus path without the interface
    suffix; name fallback; never across different links (R5).
  - Volume in the app's bridge, cubic law, 10 ms ramp; OS stream volume untouched (R8).
  - Nothing waits for an audio thread; quit waits at most 1 s for the audio close (R9).
  - Linux measurement: null sink `capturax-measure` with the card on the left and the app on the
    right, plus the output's reported latency; `analyze.py audio --drift` for the 30-minute runs
    (R10).
  - Constitution amended to 1.2.0 (maintainer, 2026-11-13): principle III's preview audio
    exception now names PipeWire on Linux next to WASAPI, so the plan has no deviation.

## Decisions

Spec decisions (maintainer, 2026-11-13) are in spec.md. The maintainer accepted the plan's
engines (WASAPI on Windows, PipeWire directly on Linux) and the constitution amendment
(2026-11-13). `--no-audio` (contracts/cli.md) is kept unless the maintainer removes it.

## Open questions

- Validated during implementation, before the code that depends on them: the Linux engine probe
  and the pairing keys (`--list-devices`) on both systems (quickstart).

## Next step

1. While the ezcap is on this Windows PC: nothing else of T006/T008 needs it here.
2. On CachyOS (ezcap and C920 moved there): `cargo build` (needs `pipewire` headers and
   `clang`), fix the probe if it does not compile (fold into the commit), then T006 (quickstart
   "Linux engine probe") and T008's Linux half (commands in measurements/pairing.md).
3. Meanwhile, on any PC: the pure crate (T009, T010, T013, T014), then T011, T012.
