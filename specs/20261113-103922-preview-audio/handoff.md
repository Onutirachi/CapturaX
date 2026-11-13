# Handoff: Preview Audio

**Updated**: 2026-11-13 | **Spec**: [spec.md](spec.md) | **Branch**: `feat/20261113-103922-preview-audio`

## State

Spec committed (`docs: specify the preview audio`). Plan written (`/speckit-plan`): plan.md,
research.md (R1–R15), data-model.md, contracts/ (audio, platform, settings-file, stats, cli),
quickstart.md. **Not committed yet**: waiting for the maintainer's review. Not pushed.

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

After the maintainer validates, commit the constitution amendment and the plan
(`docs: allow PipeWire for the preview audio` and `docs: plan the preview audio`), then
`/speckit-tasks`.
