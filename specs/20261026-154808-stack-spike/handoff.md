# Handoff: Stack Spike

**Updated**: 2026-10-26 | **Spec**: [spec.md](spec.md) | **Branch**: `feat/20261026-154808-stack-spike`

## State

Spec committed. Plan and design artifacts written, waiting for the maintainer's review.
No spike code yet.

## Done in the last session

- Spec with 4 user stories (latency, audio, visual directions, capabilities); assumptions
  confirmed by the maintainer (hardware, measurement method, OBS available).
- Plan, research (R1–R12), data model, contracts and quickstart.

## Decisions

- Latency measured relative to the TV passthrough: PS5 plays an A/V sync test video, filmed at
  240 fps; a script detects the flashes (research R2).
- Slint's official GStreamer example copies frames through the CPU on Windows and uses EGL on
  Linux, so the Slint path is measured in two variants; the Windows GPU variant is timeboxed (R5).
- Spike code in `spike/`, tagged and deleted before the PR so only docs reach `main` (R11).

## Open questions

- None blocking. GStreamer is not installed on Windows yet (first task).

## Next step

Maintainer reviews the plan; then `/speckit-tasks`.
