# Handoff: Preview Extras

**Updated**: 2026-11-23 | **Spec**: [spec.md](spec.md) | **Branch**: `feat/20261123-213544-preview-extras`

## State

The spec is clarified and the plan is written (plan, research R1–R10, data-model, contracts,
quickstart). Nothing from this session is committed yet: it waits for the maintainer's
validation.

## Done in the last session

- 2026-11-23: `/speckit-clarify` asked 3 questions, recorded in the spec's Clarifications:
  - the ambient light button is always clickable;
  - the volume button stays usable with no sound (the roadmap #17 note to dim it was removed);
  - there is no tray entry for hiding the preview.

  Two edge cases were added: the controls stay shown during a drag, and the window shown from
  the tray while hidden stays hidden. "Just listening" was removed from User Story 2 (FR-012).
- 2026-11-23: `/speckit-plan` produced:
  - the glow on the GPU after `present`, from the uploaded planes: 32 × 18, eased with
    τ 150 ms, gated by luminance, drawn only in the bars (R1–R3);
  - hiding the preview as a second "closed" reason in `preview.rs`, beside the tray (R5);
  - the bubble layout and drag in Slint, with arrays from Rust (R6, R7);
  - grab cursors forwarded to winit (R8);
  - `[overlay.bubbles]` lists (R10).
- Spec change from the plan: SC-001 also covers a run with pinned bubbles updating while the
  controls are hidden (R9). Their once-a-second UI redraw would upload the whole UI texture, so
  the UI hand-off becomes partial (only the changed rectangle), built in this spec (maintainer,
  2026-11-23).

## Decisions

- All in spec.md, plan.md and research.md.

## Open questions

- None. The glow's tuning values (R2) are set with the maintainer in the app.

## Next step

The maintainer validates the clarify and plan changes, then commit them as `docs(preview-extras):
clarify and plan` (spec.md, plan.md, research.md, data-model.md, quickstart.md, contracts/,
handoff.md, docs/roadmap.md). After that, run `/speckit-tasks` and then `/speckit-analyze`.
