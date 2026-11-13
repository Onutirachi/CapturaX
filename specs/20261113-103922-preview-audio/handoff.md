# Handoff: Preview Audio

**Updated**: 2026-11-13 | **Spec**: [spec.md](spec.md) | **Branch**: `feat/20261113-103922-preview-audio`

## State

Spec written, its three open questions answered and validated by the maintainer, committed
(`docs: specify the preview audio`). No [NEEDS CLARIFICATION] left. Not pushed.

## Done in the last session

- PR #9 (device preview) squash-merged into `main` (2f4beea) at the maintainer's request; this
  branch starts from it.
- spec.md and checklists/requirements.md written; docs/roadmap.md #4 set to in progress with the
  spec link.
- Mockup page for the UI decisions (private Artifact):
  https://claude.ai/artifact/DWT9wMk8mY64QXU9woPy2n (Audio dropdown, volume popover, settings
  Audio category, every state, the real endpoint names of the maintainer's Windows PC). Version 2
  fixed the Audio menu running off the window.

## Decisions

All recorded in spec.md (maintainer, 2026-11-13):

- Auto pairs a webcam's microphone like any other input (FR-007): there is no reliable way to
  tell a capture card from a webcam; the user picks No audio once and it is remembered.
- The output is chosen in two places, the Audio menu (Output group) and the volume popover
  (FR-009).
- Volume button (FR-011): click toggles mute; hover (~300 ms) or right click opens the popover
  (mute button, slider, output); the wheel changes the volume; on touch a tap opens the popover
  (no long press); Tab focus opens it, Enter/Space mutes.
- Linux is measured inside the PC through PipeWire plus the output's reported latency
  (FR-020, SC-002): no splitter cable nor second input on the Windows PC for an end-to-end
  recording.
- Texts in FR-022; the maintainer will check them again in the interface. State texts follow
  the video devices' ("Waiting to reconnect", not explanatory sentences).

## Open questions

- None in the spec. For the plan: the pairing link per platform (Windows container ID, Linux
  sysfs/PipeWire device properties), the Linux engine (PipeWire directly, the spike's hang and
  starvation), and the 30-minute drift runs.

## Next step

`/speckit-plan`.
