# Handoff: Stack Spike

**Updated**: 2026-11-04 | **Spec**: [spec.md](spec.md) | **Branch**: `feat/20261026-154808-stack-spike`

## State

Done. The results, decisions and open questions are in [findings.md](findings.md); the
constitution is at 1.1.0 with the Slint decision, and docs/roadmap.md marks the spike done. The
spike code is kept in the tag `spike/stack-20261026` (pushed) and removed from the branch; the
latency tools were adopted into [tools/latency/](../../tools/latency/README.md). The branch goes
to `main` by squash merge. `brand/` is not committed.

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
- T043: tag pushed; `flash.html`, `analyze.py` and `make_sync_video.py` moved to tools/latency/
  (maintainer's request, so the tools and their links survive), `analyze.py` given
  `--measurements <dir>` and `summary --paths`; re-analyzing one video and one audio clip gave the
  committed samples exactly, and the summary only changed its header line. `spike/` deleted.

## Decisions

- All recorded in findings.md "Decisions" and in the constitution; none live only here.

## Open questions

- See findings.md "Open questions for later specs" (grouped by roadmap spec).
- Local branches `backup/stack-spike-before-rebase` and `backup/before-linux-fixups` and the
  ignored build output in `spike/target/` (with the launchers) are still on the maintainer's disk;
  delete them if the maintainer agrees. The spike can be rebuilt from the tag.

## Next step

Start the App shell spec (docs/roadmap.md #2) in a new conversation, from `main`.
