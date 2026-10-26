# CapturaX Constitution

CapturaX is a desktop app for Windows and Linux to watch and hear capture cards with the lowest
possible latency (enough to play a console through the preview), capture screens and windows,
take screenshots, record, and share a live stream with friends by link. It is a simple
alternative for these tasks, not a replacement for OBS.

## Core Principles

### I. Preview Latency Is the Product

- The capture-card preview (video and audio) MUST have the lowest latency the platform allows.
  The target is playing a console through the preview without a noticeable delay.
- Every spec that touches the preview path MUST state a latency budget and how it is measured
  (method and reference, for example a high-speed camera filming the passthrough display and
  the preview side by side).
- Recording, streaming, screenshots and the UI MUST NOT add latency or dropped frames to the
  preview. Specs that run them during a preview MUST measure this.
- Devices open in a native mode (resolution, frame rate and format reported by the driver)
  with no software scaling unless the user chooses a non-native mode.

Rationale: low-latency preview is the reason CapturaX exists; every other feature is optional.

### II. Simple by Default

- One source at a time. Scenes, compositing, overlays burned into the output and streaming to
  third-party platforms (RTMP, Twitch, YouTube) are out of scope; adding them requires an
  amendment.
- Defaults MUST work without configuration: automatic device mode, audio paired with the
  video device, automatic network setup. An option is added only for a demonstrated need;
  prefer removing an option over explaining it.
- User-facing text is short: labels and values, not paragraphs. Explanations go in tooltips
  or are left out. No decorative outlines.
- Empty and error states (no devices, permission denied, device in use, device disconnected,
  server unreachable) MUST be shown explicitly with the next action, never as an empty list.

Rationale: the previous version grew verbose and cluttered; simplicity is a feature.

### III. One Native Media Pipeline

- A single native process owns every device and the whole media path: capture, preview
  rendering, audio, encoding, muxing, recording and streaming. A device is opened once and
  shared with every consumer.
- Media is built on GStreamer. Custom media code (muxers, protocols, codecs) is written only
  when GStreamer cannot do the job, and the plan MUST justify it.
- The UI never handles raw media: it controls the pipeline and displays the rendered preview.
- Audio and video share one clock; synchronization is automatic, not a user setting.
- Hardware encoders are preferred; a software fallback MUST exist.

Rationale: the previous version split the pipeline between a WebView and native code, which
required bridges, clock calibration and a workaround for almost every feature.

### IV. Windows and Linux Through a Platform Layer

- Windows and Linux are first-class. Every feature spec covers both and is done only when
  accepted on both. When a platform cannot do something (for example, listing windows on
  Wayland), the spec defines the degraded behavior and the UI states it.
- Platform-specific code (device discovery, per-app audio, screen capture, firewall,
  installer, updates) lives behind platform interfaces. No code outside a platform module
  may call OS-specific APIs.
- macOS is not a target until an amendment adds it, but no design may preclude it.

Rationale: Linux gaming is growing, and late porting is far more expensive than early
boundaries.

### V. Private and Safe by Design

- No telemetry, analytics or crash reporting that sends data anywhere. The only network
  traffic is: streams the user starts, the server the user configures, and the update check,
  which the user can turn off.
- Network-facing code (direct-link server, viewer page, signaling server) is read-only toward
  the host: fixed routes, no file access derived from URLs, no remote control of the app,
  request and connection limits, restrictive security headers. Viewers need no account.
- The self-hosted server requires a password to stream (admin and usage passwords), so a
  public instance cannot be used by strangers. Watching needs only the link.
- The app runs as a normal user. Elevation is requested only to configure the firewall, and
  only on explicit user action.

Rationale: the app opens network ports on personal computers and the server is open source.

### VI. Portable and Self-Contained

- Settings, keys and state live next to the executable, so copying the folder is a complete
  backup. If that folder is not writable, the app warns and falls back to the per-user
  configuration directory of the OS.
- Runtime dependencies, GStreamer included, ship with the app. Nothing has to be installed
  separately.
- Installers only copy files and configure the firewall. The app offers the same firewall
  setup in its settings (for example, after restoring a backup on another system).

Rationale: easy backup and migration between machines and operating system installations.

### VII. Spec-Driven, Tested and Readable

- Every feature starts from a spec under `specs/` with user scenarios and acceptance
  criteria, followed by a plan and tasks. No implementation code is written before the
  maintainer approves the spec.
- Experiments (spikes) live on their own branches. Their findings are recorded in the spec or
  plan; their code is not merged unless a spec adopts it.
- Behavior is covered by automated tests where feasible: unit tests for pure logic, pipeline
  tests with GStreamer test sources, integration tests for the server. Checks that need real
  hardware are listed as manual acceptance steps in the spec.
- Build the smallest thing that satisfies the spec. Readable code over clever code. Every new
  dependency MUST be justified in the plan (purpose, license compatibility with GPL-3.0,
  maintenance status).

Rationale: an open source project must be understandable by contributors and by agents
starting from a fresh context.

## Technology and Product Constraints

- App: Rust. Media: GStreamer, bundled with the app. UI: Slint, pending confirmation by the
  first spike spec; replacing it afterwards requires an amendment.
- Viewer page: HTML and TypeScript without a heavy framework, served both by the app (direct
  link) and by the server.
- Server: Rust, distributed as a container image with a compose file, keeping all its data in
  one volume so it can be backed up and moved. It provides signaling, the viewer page and a
  TURN relay used only when a direct connection fails.
- Live streaming: media goes from the user's PC to each viewer, peer-to-peer (WebRTC through
  the server) or by direct link (the PC serves the stream). The server never relays media
  except through the TURN fallback.
- Codecs: live streams use H.264 for browser compatibility. Recordings use H.264 or HEVC
  (AV1 where supported) in fragmented MP4 while recording, so a crash never loses the file.
- Audio: up to 8 sources. Track 1 is always the full mix; recordings may add one separate
  track per source.
- Internationalization from day one: English and Portuguese (Brazil), detected from the
  system (app) or the browser (viewer page), English as fallback. No user-facing text may be
  hard-coded outside locale files.
- Updates: the app checks GitHub Releases, notifies about new versions and installs them
  automatically when the user agrees. The check can be turned off.
- Brand: the name CapturaX, the existing logo, and orange `#FF7304` as the accent color.

## Licensing

- Source code is licensed under GPL-3.0-only.
- Third-party components (GStreamer under LGPL, Slint and others) are used under licenses
  compatible with GPL-3.0, and their notices ship with the app.

## Development Workflow

- All code, specs, commits, documentation and code comments are written in English.
- Every commit MUST follow Conventional Commits. Commits MUST NOT include `Co-Authored-By`
  or any AI attribution trailer.
- Branches: `feat/<timestamp>-<slug>` for spec work (timestamp numbering avoids collisions
  between contributors); `fix/`, `docs/`, `chore/`, `refactor/` and `test/` for other work.
  Nobody commits directly to `main`.
- Nothing is committed before the maintainer validates the change, and nothing is pushed
  unless the maintainer asks. Agents never push on their own.
- Corrections made before merging are folded into the branch history (amend or fixup);
  reverts are only for changes already on `main`.
- Changes land on `main` through pull requests merged with squash, after CI passes (format,
  lint, tests and build on Windows and Linux).
- When a decision changes during implementation, the spec is updated first, then the code.
- Each spec folder is self-contained so work can continue in a fresh conversation or with a
  different agent: `spec.md`, `plan.md`, `tasks.md`, and a `handoff.md` updated at the end of
  every session with the current state, decisions taken and the next step.

## Governance

- This constitution supersedes any other practice or guideline in the project.
- Amendments are made by pull request that states the rationale and bumps the version: MAJOR
  for removing or redefining a principle, MINOR for adding a principle or materially expanding
  guidance, PATCH for clarifications and wording.
- Every plan MUST include a constitution check, and every pull request review MUST verify
  compliance. Deviations MUST be documented in the plan with a justification.

**Version**: 1.0.0 | **Ratified**: 2026-10-26 | **Last Amended**: 2026-10-26
