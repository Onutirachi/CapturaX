# Handoff: Preview Extras

**Updated**: 2026-11-23 | **Spec**: [spec.md](spec.md) | **Branch**: `feat/20261123-213544-preview-extras`

## State

Spec, plan and tasks are done and analyzed: 32 tasks (US1 ambient light T001–T009, US2 hide
preview T010–T017, US3 bubbles T018–T029, polish T030–T032). Implementation has not started.
The tasks and the analysis fixes wait for the maintainer's validation before the commit.

## Done in the last session

- 2026-11-23: `/speckit-tasks` wrote tasks.md. It has no Setup or Foundational phase: each
  story carries its own settings key, so the three stories stay independent.
- 2026-11-23: `/speckit-analyze` found no critical issue. Its fixes are applied:
  - spec FR-005: the "With ambient light" shadow follows the ambient light being on with video,
    fading with the toggle; the luminance gate does not dim it;
  - spec FR-006: the preview button is disabled only with no device to show ("no devices",
    "no source"); a device's own states keep it enabled;
  - research R4a and data-model "Glow": the update runs only with the ambient light on (or
    fading) and a bar; after a pause longer than 100 ms the history is filled directly, so a
    stale field never shows;
  - data-model `slint_arrays()`: N is the number of kinds (4); Slint reads a missing index as
    no value;
  - quickstart check 15: bubbles stay under the settings panel;
  - tasks: T002, T004, T005 (no longer [P]), T006, T007 (`left-dock` in main.slint), T012,
    T021, T023 and the dependencies.

## Decisions

- All in spec.md, plan.md, research.md and data-model.md.

## Open questions

- None. The glow's tuning values (R2) are set with the maintainer in the app (T008).

## Next step

The maintainer validates, then commit as `docs(preview-extras): add the tasks` (tasks.md,
spec.md, plan.md, research.md, data-model.md, quickstart.md, handoff.md). After that,
`/speckit-implement` starting with US1: T001 and T002 (pure, committable after the checks).
