// Modos reais do dispositivo (lidos pelo Media Foundation no Rust) e a escolha automática.

export type VideoMode = { width: number; height: number; fps: number; format: string };
export type VideoDeviceModes = { name: string; symbolicLink: string; modes: VideoMode[] };
export type Size = { width: number; height: number };
export type ModeChoice = Size & { fps: number };

/** Abaixo disso o automático não considera a resolução "fluida" (aceita 59,94). */
const SMOOTH_FPS = 59;

export const sizeKey = (size: Size) => `${size.width}x${size.height}`;

export function parseSizeKey(key: string): Size | null {
    const [width, height] = key.split("x").map(Number);
    return width && height ? { width, height } : null;
}

/** Resoluções distintas, da maior para a menor. */
export function resolutionsOf(modes: VideoMode[]): Size[] {
    const seen = new Map<string, Size>();
    for (const { width, height } of modes) seen.set(sizeKey({ width, height }), { width, height });
    return [...seen.values()].sort((a, b) => b.width * b.height - a.width * a.height || b.width - a.width);
}

/** FPS disponíveis numa resolução, do maior para o menor. */
export function fpsFor(modes: VideoMode[], size: Size): number[] {
    const values = modes.filter((m) => m.width === size.width && m.height === size.height).map((m) => m.fps);
    return [...new Set(values)].sort((a, b) => b - a);
}

/**
 * Automático: a maior resolução que mantém pelo menos ~60 fps (senão, a maior), no maior FPS que
 * ela oferece. Com resolução ou FPS escolhidos, respeita a escolha e completa o resto.
 */
export function chooseMode(modes: VideoMode[], size: Size | null, fps: number | null): ModeChoice | null {
    const sizes = resolutionsOf(modes);
    if (sizes.length === 0) return null;
    const maxFps = (s: Size) => fpsFor(modes, s)[0] ?? 0;
    const target =
        size && sizes.some((s) => sizeKey(s) === sizeKey(size))
            ? size
            : (fps ? sizes.find((s) => fpsFor(modes, s).some((f) => Math.abs(f - fps) / fps < 0.03)) : undefined) ??
              sizes.find((s) => maxFps(s) >= SMOOTH_FPS) ??
              sizes[0];
    const available = fpsFor(modes, target);
    const chosenFps = (fps && available.find((f) => Math.abs(f - fps) / fps < 0.03)) || available[0];
    return { ...target, fps: chosenFps };
}

/** Acha os modos do dispositivo do WebView: pelo VID:PID do rótulo, ou pelo nome. */
export function findDevice(devices: VideoDeviceModes[], label: string): VideoDeviceModes | null {
    const ids = label.match(/\(([0-9a-f]{4}):([0-9a-f]{4})\)\s*$/i);
    if (ids) {
        const [, vid, pid] = ids;
        const byId = devices.find((d) => d.symbolicLink.toLowerCase().includes(`vid_${vid.toLowerCase()}&pid_${pid.toLowerCase()}`));
        if (byId) return byId;
    }
    const name = label.replace(/\s*\([0-9a-f]{4}:[0-9a-f]{4}\)\s*$/i, "").trim().toLowerCase();
    return devices.find((d) => d.name.trim().toLowerCase() === name) ?? null;
}

/** "59.94" fica com casas; "60" sem. */
export const formatFps = (fps: number) => (Number.isInteger(fps) ? String(fps) : fps.toFixed(2));

/** Modos distintos (resolução × FPS), da maior resolução para a menor e do maior FPS para o menor. */
export function choicesOf(modes: VideoMode[]): ModeChoice[] {
    return resolutionsOf(modes).flatMap((size) => fpsFor(modes, size).map((fps) => ({ ...size, fps })));
}

export const modeKey = (mode: ModeChoice) => `${mode.width}x${mode.height}@${mode.fps}`;

export function parseModeKey(key: string): ModeChoice | null {
    const [size, fps] = key.split("@");
    const parsed = parseSizeKey(size);
    return parsed && Number(fps) ? { ...parsed, fps: Number(fps) } : null;
}

/** Cor da bandeirinha de FPS (quanto mais fluido, mais destaque). */
export function fpsColor(fps: number) {
    if (fps >= 100) return "#8e44ad";
    if (fps >= 59) return "#237323";
    if (fps >= 48) return "#4974ab";
    return "#606060";
}
