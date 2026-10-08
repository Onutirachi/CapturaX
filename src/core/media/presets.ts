// Qualidade de gravação/transmissão → configuração do VideoEncoder (WebCodecs).
import type { RecordingConfig, StreamConfig, VideoCodecName } from "../native/api";

export type EncoderSettings = {
    codec: string;
    width: number;
    height: number;
    fps: number;
    bitrate: number;
    /** Intervalo entre keyframes; define o tamanho dos fragmentos/segmentos. */
    keyframeSecs: number;
};

export type SourceInfo = { width: number; height: number; fps: number };

const even = (n: number) => Math.max(2, Math.round(n / 2) * 2);

/** Reduz para no máximo `maxHeight` mantendo a proporção da fonte. */
function fitHeight(source: SourceInfo, maxHeight: number) {
    if (source.height <= maxHeight) return { width: even(source.width), height: even(source.height) };
    return { width: even((source.width * maxHeight) / source.height), height: even(maxHeight) };
}

/** Bits por pixel por frame: quanto maior, melhor a imagem e maior o arquivo. */
function bitrateFor(width: number, height: number, fps: number, bitsPerPixel: number) {
    return Math.round(Math.min(150_000_000, Math.max(2_000_000, width * height * fps * bitsPerPixel)));
}

// Níveis H.264: (amostras de luma por segundo, macroblocos por frame).
const AVC_LEVELS: [number, number, number][] = [
    [0x28, 62_914_560, 8_192], // 4.0
    [0x2a, 133_693_440, 8_704], // 4.2
    [0x32, 150_994_944, 22_080], // 5.0
    [0x33, 251_658_240, 36_864], // 5.1
    [0x34, 530_841_600, 36_864], // 5.2
    [0x3c, 4_278_190_080, 139_264], // 6.0
    [0x3e, 16_711_680_000, 139_264], // 6.2
];

// Níveis HEVC (nível × 30) com o limite de amostras de luma por segundo.
const HEVC_LEVELS: [number, number][] = [
    [120, 66_846_720], // 4
    [123, 133_693_440], // 4.1
    [150, 267_386_880], // 5
    [153, 534_773_760], // 5.1
    [156, 1_069_547_520], // 5.2
    [180, 1_069_547_520], // 6
    [183, 2_139_095_040], // 6.1
    [186, 4_278_190_080], // 6.2
];

export function codecString(codec: VideoCodecName, width: number, height: number, fps: number) {
    const samples = width * height * fps;
    if (codec === "hevc") {
        const level = HEVC_LEVELS.find(([, max]) => samples <= max)?.[0] ?? 186;
        return `hvc1.1.6.L${level}.B0`;
    }
    const macroblocks = Math.ceil(width / 16) * Math.ceil(height / 16);
    const level = AVC_LEVELS.find(([, max, mbs]) => samples <= max && macroblocks <= mbs)?.[0] ?? 0x3e;
    return `avc1.6400${level.toString(16).padStart(2, "0")}`;
}

export function recordingSettings(config: RecordingConfig, source: SourceInfo): EncoderSettings {
    const sourceFps = Math.max(1, Math.round(source.fps || 60));
    let size: { width: number; height: number };
    let fps: number;
    let bitrate: number;

    switch (config.preset) {
        case "original":
            size = fitHeight(source, Infinity);
            fps = sourceFps;
            bitrate = bitrateFor(size.width, size.height, fps, 0.12);
            break;
        case "high":
            size = fitHeight(source, 1080);
            fps = sourceFps;
            bitrate = bitrateFor(size.width, size.height, fps, 0.1);
            break;
        case "medium":
            size = fitHeight(source, 720);
            fps = Math.min(sourceFps, 60);
            bitrate = bitrateFor(size.width, size.height, fps, 0.08);
            break;
        case "light":
            size = fitHeight(source, 720);
            fps = Math.min(sourceFps, 30);
            bitrate = bitrateFor(size.width, size.height, fps, 0.07);
            break;
        case "custom":
            size = config.height ? fitHeight(source, config.height) : fitHeight(source, Infinity);
            fps = Math.min(sourceFps, config.fps ?? sourceFps);
            bitrate = config.bitrateKbps * 1000;
            break;
    }
    return { codec: codecString(config.codec, size.width, size.height, fps), ...size, fps, bitrate, keyframeSecs: 2 };
}

export function streamSettings(config: StreamConfig, source: SourceInfo): EncoderSettings {
    const size = fitHeight(source, config.height || 720);
    const fps = Math.min(Math.max(1, Math.round(source.fps || 60)), config.fps || 30);
    // Transmissão sempre em H.264: é o que todo navegador toca.
    return { codec: codecString("h264", size.width, size.height, fps), ...size, fps, bitrate: config.bitrateKbps * 1000, keyframeSecs: 1 };
}

/** Configuração completa do VideoEncoder para estas opções. */
export function encoderConfig(settings: EncoderSettings): VideoEncoderConfig {
    const base: VideoEncoderConfig = {
        codec: settings.codec,
        width: settings.width,
        height: settings.height,
        bitrate: settings.bitrate,
        framerate: settings.fps,
        hardwareAcceleration: "prefer-hardware",
        latencyMode: "realtime",
        bitrateMode: "variable",
    };
    if (settings.codec.startsWith("avc")) return { ...base, avc: { format: "avc" } } as VideoEncoderConfig;
    return { ...base, hevc: { format: "hevc" } } as VideoEncoderConfig;
}

/** Usa o codec pedido se a máquina suportar; senão cai para H.264. */
export async function resolveSupported(settings: EncoderSettings): Promise<EncoderSettings> {
    const supported = await VideoEncoder.isConfigSupported(encoderConfig(settings)).catch(() => ({ supported: false }));
    if (supported.supported) return settings;
    if (!settings.codec.startsWith("avc")) {
        const fallback = { ...settings, codec: codecString("h264", settings.width, settings.height, settings.fps) };
        const ok = await VideoEncoder.isConfigSupported(encoderConfig(fallback)).catch(() => ({ supported: false }));
        if (ok.supported) return fallback;
    }
    throw new Error(`This PC cannot encode ${settings.width}x${settings.height} @ ${settings.fps} fps`);
}

export async function isHevcSupported() {
    if (typeof VideoEncoder === "undefined") return false;
    const settings: EncoderSettings = { codec: codecString("hevc", 1920, 1080, 60), width: 1920, height: 1080, fps: 60, bitrate: 10_000_000, keyframeSecs: 2 };
    const result = await VideoEncoder.isConfigSupported(encoderConfig(settings)).catch(() => ({ supported: false }));
    return !!result.supported;
}
