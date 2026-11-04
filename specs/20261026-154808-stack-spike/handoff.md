# Handoff: Stack Spike

**Updated**: 2026-11-04 | **Spec**: [spec.md](spec.md) | **Branch**: `feat/20261026-154808-stack-spike`

## State

The spike is complete; only T043 and T044 are left, and both need the maintainer's approval. The
results, decisions and open questions are in [findings.md](findings.md); the constitution is at
1.1.0 with the Slint decision, and docs/roadmap.md marks the spike done. Everything is committed
locally; nothing is pushed. `brand/` is not committed.

## Done in the last session

- T022 and T026: the formal video and audio results and the SC-001 to SC-005 evaluation in
  findings.md. SC-001 to SC-004 pass (SC-002 only with the own render loop); SC-005 passes on
  offset and by ear, drift is open for the WASAPI engine.
- T040: findings.md complete: method, decisions, open questions grouped by roadmap spec; OBS and
  Edge versions in measurements/environment.md (read after the clips).
- T041: constitution 1.0.0 → 1.1.0 (MINOR, approved by the maintainer): Slint confirmed on
  condition of the own render loop; principle III lets the preview leave GStreamer at its last
  step (own GPU renderer, WASAPI preview audio) and keeps one clock only for recordings and
  streams. The maintainer's reading: only the capture-card preview must have the lowest latency;
  for recording and streaming low latency is desirable, not required. Roadmap entries 1, 2, 3,
  4 (now with F47) and 6 revised; docs/product.md lip-sync note corrected.

## Decisions

- All recorded in findings.md "Decisions" and in the constitution; none live only here.

## Open questions

- See findings.md "Open questions for later specs" (grouped by roadmap spec).
- After `spike/` is deleted, links from the spec documents into `spike/` (for example
  findings.md "Method" to `spike/tools/flash.html`) will point to removed files; decide in T043
  whether to point them at the tag instead.
- Local branches `backup/stack-spike-before-rebase` and `backup/before-linux-fixups` were kept
  until the rewritten history was approved; delete them after the merge if the maintainer agrees.

## Next step

1. T043, with the maintainer's approval: tag the branch head as `spike/stack-20261026`, push the
   tag, then delete `spike/` in a commit (research R11).
2. T044, with the maintainer's approval: push the branch, open the pull request, squash merge.
3. Then the next spec is App shell (docs/roadmap.md #2), in a new conversation.
