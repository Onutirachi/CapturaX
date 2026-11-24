# Handoff: Preview Extras

**Updated**: 2026-11-23 | **Spec**: [spec.md](spec.md) | **Branch**: `feat/20261123-213544-preview-extras`

## State

Spec drafted, both open questions answered; nothing committed yet. The visual decisions wait for
a mockup page before `/speckit-clarify` and `/speckit-plan`.

## Done in the last session

- 2026-11-23: branch and spec created (`/speckit-specify`); checklist passes.
- Q1 (FR-012): the preview is video and audio together; hiding stops both. Recorded in
  product.md F10; "To reconsider" item removed.
- Q2 (FR-011): the hidden state is saved; a start while hidden opens no device.
- docs (same branch, uncommitted): roadmap #6 in progress; #17 UI review moved after #9 and #10;
  F59 moved from product.md's Removed table to K (General); notes for later: one screenshot key
  vs. one per output (product.md To reconsider, roadmap #9), Discord-like source picker with
  Screens and Apps tabs and preview images (F5, roadmap #7), Settings as a compact floating
  window with `--settings` (F48, roadmap #14).

## Decisions

- All in the spec and in docs/ (see above).

## Open questions

- None. Mockup decisions D1–D7 answered 2026-11-23 and written into the spec (Assumptions lists
  them; FR-001, FR-002, FR-006, FR-008, FR-014–FR-016). product.md F10 updated (icon
  `device_tv_off`, no action).

## Next step

`/speckit-clarify` (quick pass; few open points expected), then `/speckit-plan`. Mockup page:
https://claude.ai/artifact/C7uffxbE8YBmFXSGkNNbPz. The plan must cover: the glow in the app's
renderer (reduced, blurred frame copy, eased over ~0.5 s, dimmed on dark frames) within SC-001;
releasing and reopening video and audio on hide/show; drag in Slint with grab cursors.
