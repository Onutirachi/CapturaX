# T052 run notes (working copy; final goes to specs/.../measurements/stalls.md)

Setup: AOC 27G2G4 (G-SYNC, 144 Hz, DISPLAY2, middle), window 1071x565 at 115,37; ezcap 1080p60
NV12 fed by the CachyOS HDMI out at 1920x1080@60.00 (desktop, inhibited from sleeping);
build 8b0a033 + T054 working tree; drive.ps1 (real clicks, F11 by keybd_event), pm.ps1.

## base (G-SYNC on) — 18:31, pm-183120.csv, base-analysis.txt
- Focus changes (10 out, 10 in): no app present gap; PresentMon one display gap of 50-57 ms
  (~17 ms over normal: video frame displayed ~34-40 ms after present) ~21 ms after each event.
  Mode stays Hardware Composed: Independent Flip. Not the 50-675 ms of T028 (that was the
  Samsung C24FG7x, FreeSync, on the AMD iGPU? to check).
- Fullscreen (11 in, 11 out): every switch goes Independent Flip -> Composed: Flip (~60-130 ms)
  -> Independent Flip; one dropped present, display gaps 50-208 ms (first exit 208, others
  60-116); the app's own presents stop 50-186 ms at the switch (surface resize on the video
  thread).
- 30 s idle fullscreen: no stall.
- App present gaps split (acquire line): two kinds. (a) the video thread is late to acquire
  (50-186 ms between the last present and the next acquire: resize/reconfigure work) at
  fullscreen exits mostly; (b) the acquire itself blocks ~37-66 ms (swap chain waitable object)
  right after a switch: only with G-SYNC on.

## gsync-off (NVIDIA profile: Monitor Technology = Fixed refresh for t052\capturax.exe) — 18:41, pm-184114.csv
- Focus changes: nothing over 50 ms (the 50-57 ms display gaps of base are gone).
- Fullscreen: entries clean; exits keep Independent Flip -> Composed: Flip -> Independent Flip,
  with 1 dropped present and display gaps 53-82 ms (vs 60-208 with G-SYNC); only 3 app gaps
  (50-54 ms, kind a); no blocking acquire (kind b gone).
- 30 s idle fullscreen: no stall.

## visual (WGPU_DX12_PRESENTATION_SYSTEM=visual, G-SYNC back to global per the maintainer) — 18:46
- Focus: nothing over 50 ms. Fullscreen: no dropped present, no app gap, display gaps 52-83 ms on
  exits only. Looks like gsync-off or better: control needed (base again) to be sure G-SYNC was
  really back on.

## base2 (control, same as base, 18:49) — pm-184929.csv
- Looks like gsync-off/visual, not like base: focus clean, exits 52-90 ms, 1 dropped present.
  So G-SYNC was most likely still off for the app after the maintainer set "Use global setting"
  (or base is not repeatable). visual is inconclusive until G-SYNC is shown active
  (NVIDIA G-SYNC indicator) and base reproduces.
- Maintainer (19:0x): G-SYNC was on in base2 (with Fixed refresh the AOC's text is visibly
  blurred, a monitor defect; base2 was not). So visual was most likely G-SYNC on too.

## base3 (NVIDIA profile removed, 18:57:54) — pm-185754.csv
- t=19-43 s (18:58:13-18:58:37): a burst of stalls on a 1 s grid, not aligned with focus events,
  max 681 ms; [loop] shows recv dropping (25 frames in one second) and ready_ms up to 666:
  frames arrived late from the capture side too, so something outside the presentation path
  (system-wide hiccup?). Ask the maintainer what ran then.
- Outside that window: focus clean; fullscreen exits 53-90 ms, 1 dropped present; idle clean.
- So base (first) is the odd one: base2 and base3 agree with each other.

## Suspend (2026-11-11 ~19:13)
- The maintainer's earlier "sleep" (18:14) was `nircmd monitor off`, not a suspend: Kernel-Power
  566 events at 18:14:37/18:14:55. The first base (18:31) ran after that monitor-off cycle.
- A real Suspend (first time on this PC): no flicker on wake, but the ezcap stopped working:
  MF "Failed to read sample" 190 ms after Playing, mapped to InUse by the R6 heuristic (wrong:
  nothing else held it); `pnputil /restart-device` on the USB composite did not help; OBS fails
  too. Device/driver after suspend, not the app; needs a reboot. For T043/T045: a failure right
  after resume reads as "in use".
- base-sleep: not possible (files in void/sleep).
Next: reboot with no DWM keys -> base-boot; then nircmd monitor off/on -> base-monoff (the
hypothesis for the odd first base); then minfps0, mpo-off (a reboot each), dwm-default.

## base-boot (fresh reboot, no DWM keys, 19:24) — pm-192451.csv
- Cleanest so far: only the start (first 1.3 s, before the run) has gaps; focus changes clean;
  fullscreen exits 3 of 11 over 50 ms (54-59 ms), no dropped present after start; idle clean.

## base-monoff (after `nircmd monitor off` and wake, no flicker, 19:29) — pm-192927.csv
- Focus clean; fullscreen exits 52-90 ms (7 of 11), no dropped present: same as base2/base3/boot.
  So the monitor-off cycle does not bring the first base's ~50 ms focus stalls back.
- One stall of 437 ms in the idle fullscreen (5.2 s after entering), [loop] recv=41 ready_ms 420.

## Random stalls (base3 burst, base-monoff 437 ms): a lead, in the app
- All of them are before the acquire (the video thread is late, not the swap chain), the frame
  waited in the inbox (ready_ms up to 666), and the card's frames stop arriving too (recv 25-41).
- base3's 15 stalls sit on a 1.00 s grid (19.23, 20.25, 23.25 … 43.30 s): the stats tick.
- render/mod.rs:534-536: `print_stats` (println! to stdout redirected to a file + log::info! to
  capturax.log, app/src/log.rs: unbuffered `write_all` under the logger mutex) runs on the video
  thread *inside the inbox lock*; the capture thread pushes frames under the same lock, hence recv
  drops. D: is a Seagate ST2000DM008 HDD (C: is NVMe): a write can take hundreds of ms.
- Only with --stats (without it the video thread logs nothing while playing). T028's 50-675 ms
  stalls were measured with --stats from D: as well.
- To confirm: the same run from a copy on C: (NVMe) or without --stats; or time print_stats.
  Fix candidate for T053: print outside the inbox lock and hand lines to a writer thread (as the
  present log does).

## hdd-d / hdd-c (19:40, confirmation of the lead; no PresentMon, app data only)
- `stress.py`: 4 MiB writes with fsync + random 64 KiB reads on D: (~13 MB/s, HDD busy), 70 s;
  `idle.ps1`: app windowed, idle, `--stats --present-log`, 60 s, outputs next to the exe.
- hdd-d (exe, stdout, capturax.log on D:): 44 of 57 seconds have a stall, 40-718 ms, recv down to
  23: every one on the stats tick (ready_ms max = gap_ms - ~16).
- hdd-c (copy in C:\capx-t052, NVMe; same stress on D:): no gap over 40 ms in 60 s.
- **Confirmed**: the random stalls are the app's own `--stats` writes on the video thread inside
  the inbox lock, slow when D: is busy. Without `--stats` the playing loop writes nothing.
  Fix for T053: format inside the lock, print and log outside it on a writer thread (as the
  present log). Not a user-facing stall today (--stats is a dev option), but it pollutes every
  measurement taken from D:.
- From here, runs go from C:\capx-t052 (drive.ps1 -Exe), so the matrix is not polluted.

## base-c (exe from C:\capx-t052, G-SYNC on, no DWM keys, 19:44) — pm-194444.csv, base-c-analysis.txt
- The reference for the rest of the matrix.
- Focus changes (10 out, 10 in): nothing over 50 ms, Independent Flip throughout.
- Fullscreen entries: clean (display gaps of 50 ms at most at the Composed: Flip step).
- Fullscreen exits: 6 of 11 have an app gap of 62-84 ms (1 dropped present each: the present of
  the old size during Independent Flip -> Composed: Flip -> Independent Flip), display gaps
  51-118 ms; the others 40-53 ms. Same as base-boot / base-monoff.
- 30 s idle fullscreen: clean. No stall off the events in the whole run.

## minfps0 (OverlayMinFPS=0, reboot 19:54, exe from C:, 20:19) — pm-201927.csv, minfps0-analysis.txt
- Two earlier tries (20:13, 19:56) have no PresentMon CSV (pm.ps1 was not running elevated);
  the app files of the third overwrote them.
- Focus: no app gap; 4 of 20 changes have a display gap of 55-57 ms (frame shown ~39-45 ms after
  its present, in Independent Flip; like the first base's ~50 ms ones), not seen in base-c.
- Fullscreen entries: display gaps 50-88 ms at the Composed: Flip step, no app gap.
- Fullscreen exits: 9 of 11 have app gaps of 51-73 ms (base-c: 6 of 11, 62-84), 1 dropped
  present each; display gaps 52-87 ms. Same pattern (Independent Flip -> Composed: Flip ->
  Independent Flip, video thread late after the resize).
- 30 s idle fullscreen: clean.
- Conclusion: OverlayMinFPS=0 does not help (if anything slightly worse on focus). Not a fix.

## mpo-off (OverlayTestMode=5, reboot 20:43, exe from C:, 20:47) — pm-204716.csv, mpo-off-analysis.txt
- (probe at 20:46, pm-204641.csv: only to check pm.ps1 was capturing.)
- Windowed the app is Composed: Flip (no overlay plane): msUntilDisplayed median 11.1 ms vs 1.2
  in base-c (Hardware Composed: Independent Flip). Fullscreen is Hardware: Independent Flip, 0.4
  in both. So MPO off costs ~10 ms of display latency windowed: not acceptable as a workaround.
- Focus: clean (no app or display gap over 50 ms).
- Fullscreen entries: clean. Exits: 5 of 11 app gaps 60-68 ms (1 dropped present each), display
  gaps 58-80 ms: same pattern as base-c (Independent Flip -> Composed: Flip at the exit, then
  the video thread is late after the resize).
- 30 s idle fullscreen: clean.
- Conclusion: MPO off does not remove the exit stall and makes windowed latency worse.

## Matrix so far (fullscreen exit is the only stall left everywhere)
- Every config: entries and idle clean; exits 50-85 ms in 5-9 of 11, with the switch
  Independent Flip -> Composed: Flip -> Independent Flip and one dropped present. Focus clean
  except base (first) and minfps0 (display-only 50-57 ms). The exit stall looks like the app's
  part (the video thread late to acquire after the resize, "kind a"), the T053 subject.

## lang (defaults back: dwm-default.reg + reboot 20:52; exe from C:, 20:56) — pm-205659.csv, lang-analysis.txt
- First pm.ps1 window was not elevated (access denied); the elevated one started at 20:56:59,
  t=44 s of the app run, before the switches (t=54-90 s).
- Ten language switches by the maintainer in the panel: no app gap, no display gap over 50 ms.
  Stages: catalog 0.0, property 0.1-0.2, draw 2.6-2.8, upload 0.2-0.3, present 0.4-7.8 ms.
- Display gaps of 55-458 ms at t=43-47 s in Composed: Flip with the app presenting every 16 ms:
  the UAC prompt of the elevated PowerShell (secure desktop), not the app.
