# Handoff: Device Preview

**Updated**: 2026-11-06 | **Spec**: [spec.md](spec.md) | **Branch**: `feat/20261106-170507-device-preview`

## State

Spec and plan approved by the maintainer and committed (`docs: add device preview spec`, `docs:
add device preview plan`; plan.md, research.md R1–R14, data-model.md, contracts/,
quickstart.md). tasks.md (59 tasks, T001–T059) validated and committed (`docs: add device preview tasks`). Nothing pushed. No code.

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

- **No HDCP hint** (maintainer, 2026-11-06): FR-020 removed. Black video with a signal also comes
  from a console that is off, RCA adapters and VHS tapes, so it is not a reliable sign of HDCP; no
  black-image detection. HDCP, color range, modes and the bubbles go into a getting started guide
  (product.md F54, roadmap #15, last spec). Spec, research R5, data-model, contracts, quickstart,
  tasks, product F11 and roadmap #3 updated.

- **AMD and Intel GPUs** (maintainer, 2026-11-06): FR-012 is covered and measured on NVIDIA
  only; AMD and Intel are a known limit (no hardware, known controls need root), to be measured
  when the maintainer has one (spec FR-012 and Assumptions, research R7).

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

Optionally run `/speckit-analyze`; then ask the maintainer for explicit approval before
`/speckit-implement` (starts at T001). Linux GPU power (R7) is validated early, at T025, before it
is wired in.
