# Handoff: Device Preview

**Updated**: 2026-11-06 | **Spec**: [spec.md](spec.md) | **Branch**: `feat/20261106-170507-device-preview`

## State

Spec and plan approved by the maintainer and committed (`docs: add device preview spec`, `docs:
add device preview plan`; plan.md, research.md R1–R14, data-model.md, contracts/,
quickstart.md). Nothing pushed. No code.

## Done in the last session

- `/speckit-specify`: spec with 5 stories; the maintainer chose: latency bubble with Measured /
  Measured + estimated (default) / Total, frame rate bubble with Presented / Presented / nominal
  (default) / Nominal, both chosen from a settings button on the bubble's hover; color range as
  its own toolbar dropdown.
- `/speckit-plan`: research on the maintainer's machines (Windows with a C920, CachyOS with the
  ezcap over SSH) and in the GStreamer 1.28.8 and wgpu 30 sources.

## Decisions

- All recorded in spec.md and research.md. Main plan choices: new crate `crates/capture`; YUV
  planes uploaded as they come and converted in the shader (range switches live); Auto source not
  shown on UVC cards (none reports its input, R3) but kept for devices that do; the default is
  Auto max FPS (maintainer, 2026-11-06; 1080p120 on the ezcap, whose cost with a 60 Hz source is
  measured, L3); a card without a serial moved to another port is a new device (maintainer); Linux GPU kept awake through
  NVML PowerMizer, per process, no root (R7, to validate first); Windows stalls investigated with
  PresentMon and a present log before any fix (R8); a G-SYNC driver profile would be a maintainer
  decision.

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

Plan approved and committed (`docs: add device preview plan`). Run `/speckit-tasks`.
