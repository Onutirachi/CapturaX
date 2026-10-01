# CapturaX — Agent Instructions

Tauri 2 desktop app for video/audio capture. **Uses Preact, not React.**

## Build & Dev Commands

```bash
npm run tauri dev      # Dev mode (Tauri + Vite together — use this, not just npm run dev)
npm run build          # tsc && vite build
npm run tauri          # Access Tauri CLI directly
```

Dev server runs on `localhost:1420` (fixed port — fails if unavailable).

## Stack

| Layer | Tech |
|-------|------|
| Frontend | **Preact** 10 (not React), TypeScript 5.6 strict mode |
| Styling | **Tailwind CSS v4** (`@import "tailwindcss"` + `@theme` blocks in CSS) |
| Build | Vite 6 + `@preact/preset-vite` |
| Desktop | **Tauri 2** (Rust backend) |
| JSX | `jsxImportSource: "preact"` — import from `preact/hooks`, not `react` |

## Critical: Preact vs React

- Import hooks from `preact/hooks`: `import { useState, useEffect, useRef } from "preact/hooks"`
- Import component types from `preact`: `import { type ComponentChildren } from "preact"`
- Never `import React from "react"` or use React-specific APIs

## Architecture

```
src/main.tsx        → FullscreenProvider > WindowBar + App
src/App.tsx         → VideoStreamProvider > AlertsProvider > VideoStream + Overlay
src/context/        → 3 contexts: VideoStreamContext, AlertsContext, FullscreenContext
src/components/     → UI components (Overlay/, VideoStream/, WindowBar/)
src/core/           → Reusable primitives (components/, utils/)
src-tauri/          → Rust backend (currently minimal, no custom commands)
```

### Context Providers
- `VideoStreamContext` — `videoStream` (MediaStream) + `videoTrack` (MediaStreamTrack)
- `AlertsContext` — `alert` `{ text, color, icon }` for transient notifications
- `FullscreenContext` — `fullscreen` boolean

### Key Class: `Stream` (`src/core/utils/Stream.ts`)
Central media manager. Instantiated once via `useRef` in `ToolbarContent`. Handles:
- Device enumeration (`Stream.getVideoDevices()`, `Stream.getAudioDevices()`)
- Track updates (`updateVideoTrack()`, `updateAudioTrack()`)
- Constraints (`setResolution()`, `setFrameRate()`, `setResolutionTag()`)
- Capability caching (`Map<string, MediaTrackCapabilities>`)
- Alerts via injected `setAlert` from context

### Core Reusable Components (`src/core/components/`)
- `DropDown` / `DropOption` — custom controlled dropdown with **localStorage auto-persistence**
- `ButtonToggle` — toggle with optional `storeState` (localStorage key)
- `Tag` — badge with `color` prop using CSS custom property `--color`

### Styling Conventions
- Tailwind CSS v4 utility classes directly on JSX elements
- Custom CSS tokens defined in [`src/Globals.css`](src/Globals.css) under `@theme { ... }`
- Key colors: `--color-good` (#3b82f6 blue), `--color-bad` (#ef4444 red)
- No CSS modules — global CSS + Tailwind only

### Tauri-Specific Patterns
- Frameless window: drag region via `data-tauri-drag-region` attribute on `WindowBar`
- Window controls: `getCurrentWindow().minimize()` / `.maximize()` / `.close()`
- No custom Rust commands currently — Tauri used only for windowing

## TypeScript Strictness
- Strict mode + `noUnusedLocals` + `noUnusedParameters` enabled
- All new code must be fully typed; avoid `any`

## Resolution & Framerate Data
- 48 standard resolutions in [`src/core/utils/constants.ts`](src/core/utils/constants.ts) (8K → QVGA), each with `tag`, `width`, `height`, `color`
- `Resolution` utility class: `getFromTag()`, `getApproximateTag()`, `getAspectRatio()`
- Framerates: `[320, 240, 180, 165, 144, 120, 60, 30, 25, 24]`
