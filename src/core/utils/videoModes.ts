// Modos reais do dispositivo (lidos pelo Media Foundation no Rust) e a escolha automática.

export type VideoMode = { width: number; height: number; fps: number; format: string };
export type VideoDeviceModes = { name: string; symbolicLink: string; containerId: string | null; modes: VideoMode[] };

/** ContainerId que o Windows usa para "sem aparelho" (dispositivos virtuais, placa-mãe). */
const NO_CONTAINER = "00000000-0000-0000-ffff-ffffffffffff";

/** Entrada de áudio do mesmo aparelho do vídeo (ex.: "HDMI (ezcap Live GAMER ULTRA)"). */
export function pairedAudio<T extends { containerId: string | null }>(video: VideoDeviceModes | null, inputs: T[]): T | null {
    const container = video?.containerId;
    if (!container || container === NO_CONTAINER) return null;
    return inputs.find((input) => input.containerId === container) ?? null;
}
export type Size = { width: number; height: number };
export type ModeChoice = Size & { fps: number };

/** Acima disso o automático já considera fluido e passa a preferir resolução (aceita 59,94). */
const SMOOTH_FPS = 59;
/** Formatos que o Chromium consegue abrir numa câmera (H.264 da webcam, por exemplo, não). */
const SUPPORTED_FORMATS = new Set(["NV12", "YUY2", "MJPG", "I420", "IYUV", "YV12", "UYVY"]);
/** FPS baixos só aparecem na lista quando são o máximo daquela resolução. */
const MIN_LISTED_FPS = 24;

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

/** Só os modos que o Chromium consegue abrir. */
export function usableModes(modes: VideoMode[]) {
    return modes.filter((m) => SUPPORTED_FORMATS.has(m.format.toUpperCase()));
}

/**
 * Automático: o mais fluido primeiro (até ~60 fps), depois a maior resolução, depois o maior FPS.
 * Placa com 1440p60 e 4K30 → 1440p60; webcam com 1080p30 e uma foto 2304x1536 a 2 fps → 1080p30.
 * Com resolução ou FPS escolhidos, respeita a escolha e completa o resto.
 */
export function chooseMode(modes: VideoMode[], size: Size | null, fps: number | null): ModeChoice | null {
    const sizes = resolutionsOf(modes);
    if (sizes.length === 0) return null;
    const maxFps = (s: Size) => fpsFor(modes, s)[0] ?? 0;
    const best = [...sizes].sort(
        (a, b) => Math.min(maxFps(b), SMOOTH_FPS) - Math.min(maxFps(a), SMOOTH_FPS) || b.width * b.height - a.width * a.height || maxFps(b) - maxFps(a)
    )[0];
    const target =
        size && sizes.some((s) => sizeKey(s) === sizeKey(size))
            ? size
            : (fps ? sizes.find((s) => fpsFor(modes, s).some((f) => Math.abs(f - fps) / fps < 0.03)) : undefined) ?? best;
    const available = fpsFor(modes, target);
    const chosenFps = (fps && available.find((f) => Math.abs(f - fps) / fps < 0.03)) || available[0];
    return { ...target, fps: chosenFps };
}

/** Acha os modos do dispositivo do WebView: pelo VID:PID do rótulo, ou pelo nome. */
export function findDevice(devices: VideoDeviceModes[], label: string): VideoDeviceModes | null {
    const ids = label.match(/\(([0-9a-f]{4}):([0-9a-f]{4})\)\s*$/i);
    const found =
        (ids && devices.find((d) => d.symbolicLink.toLowerCase().includes(`vid_${ids[1].toLowerCase()}&pid_${ids[2].toLowerCase()}`))) ||
        devices.find((d) => d.name.trim().toLowerCase() === deviceName(label).toLowerCase());
    return found ? { ...found, modes: usableModes(found.modes) } : null;
}

/** Nome do dispositivo sem o "(046d:082d)" (VID:PID da USB) que o navegador acrescenta. */
export function deviceName(label: string) {
    return label.replace(/\s*\([0-9a-f]{4}:[0-9a-f]{4}\)\s*$/i, "").trim();
}

/** "59.94" fica com casas; "60" sem. */
export const formatFps = (fps: number) => (Number.isInteger(fps) ? String(fps) : fps.toFixed(2));

/** Modos distintos (resolução × FPS), da maior resolução para a menor e do maior FPS para o menor. */
export function choicesOf(modes: VideoMode[]): ModeChoice[] {
    return resolutionsOf(modes).flatMap((size) => {
        const all = fpsFor(modes, size);
        const listed = all.filter((fps, i) => i === 0 || fps >= MIN_LISTED_FPS);
        return listed.map((fps) => ({ ...size, fps }));
    });
}

export const modeKey = (mode: ModeChoice) => `${mode.width}x${mode.height}@${mode.fps}`;

export function parseModeKey(key: string): ModeChoice | null {
    const [size, fps] = key.split("@");
    const parsed = parseSizeKey(size);
    return parsed && Number(fps) ? { ...parsed, fps: Number(fps) } : null;
}

// Cor da bandeirinha de FPS, na mesma paleta das bandeirinhas de resolução.
const FPS_COLORS: [number, string][] = [
    [240, "#e0c427"], // 240+
    [165, "#d77895"], // 165–200
    [144, "#c40000"], // 144
    [100, "#845a87"], // 100–120
    [59, "#237323"], // 60 (59,94)
    [48, "#63acb7"], // 48–50
    [29, "#4974ab"], // 30 (29,97)
    [23, "#e5851f"], // 24–25 (23,976)
];

export function fpsColor(fps: number) {
    return FPS_COLORS.find(([min]) => fps >= min)?.[1] ?? "#606060";
}
