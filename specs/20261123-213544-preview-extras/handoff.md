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

- Visual, for the mockup page: the glow's look (blur, reach, strength, black frames), the dock
  icons (ambient light, preview), the pinned mark, the gestures to move a bubble (drag to a
  corner and within a row) and the menu alternative, the "Preview hidden" state, where
  **Reset bubbles** sits.

## Next step

Build the mockup page (Artifact, app palette, both themes, real device data: ezcap 1080p60 and
1080p120, C920) for the open visual questions; record the choices in the spec; then
`/speckit-clarify` and `/speckit-plan`. Nothing is committed until the maintainer validates.
