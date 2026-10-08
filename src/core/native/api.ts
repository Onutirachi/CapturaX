// Ponte tipada para os comandos Rust (src-tauri/src/commands.rs).
import { invoke } from "@tauri-apps/api/core";

export type DeviceInfo = { id: string; name: string; isDefault: boolean };
export type AudioDevices = { inputs: DeviceInfo[]; outputs: DeviceInfo[] };

export type AudioConfig = {
    nativePreview: boolean;
    previewInput: string | null;
    previewOutput: string | null;
    previewDelayMs: number;
    recordInput: string | null;
    recordInputEnabled: boolean;
    recordInputVolume: number;
    recordMic: string | null;
    recordMicVolume: number;
    recordSystem: boolean;
    recordSystemVolume: number;
    excludedApps: string[];
    knownApps: string[];
    avOffsetMs: number;
};

export type RecordingPreset = "original" | "high" | "medium" | "light" | "custom";
export type VideoCodecName = "h264" | "hevc";

export type RecordingConfig = {
    folder: string | null;
    preset: RecordingPreset;
    codec: VideoCodecName;
    width: number | null;
    height: number | null;
    fps: number | null;
    bitrateKbps: number;
    audioBitrateKbps: number;
};

export type StreamConfig = {
    acceptViewers: boolean;
    port: number;
    upnp: boolean;
    width: number;
    height: number;
    fps: number;
    /** Bitrate calculado pela resolução/FPS; `false` usa `bitrateKbps`. */
    bitrateAuto: boolean;
    bitrateKbps: number;
    audioBitrateKbps: number;
    blockedIps: string[];
};

export type Config = { audio: AudioConfig; recording: RecordingConfig; stream: StreamConfig };

export type AppListEntry = { exe: string; name: string; running: boolean; playing: boolean; excluded: boolean };

export type SessionStatus = { id: number; kind: "recording" | "stream"; elapsedSecs: number; bytes: number };
export type MediaStatus = { sessions: SessionStatus[]; recording: { id: number; path: string } | null; streaming: number | null };

export type UpnpStatus =
    | { state: "disabled" }
    | { state: "searching" }
    | { state: "mapped"; externalIp: string; cgnat: boolean }
    | { state: "failed"; reason: string };

export type ViewerInfo = { ip: string; watchingSecs: number };

export type ServerStatus = {
    running: boolean;
    port: number;
    localUrl: string | null;
    publicUrl: string | null;
    upnp: UpnpStatus;
    firewallOk: boolean;
    live: boolean;
    viewers: ViewerInfo[];
    error: string | null;
};

export const native = {
    clockNow: () => invoke<number>("clock_now"),
    getConfig: () => invoke<Config>("get_config"),
    setConfig: (config: Config) => invoke<void>("set_config", { config }),

    audioDevices: () => invoke<AudioDevices>("audio_devices"),
    setPreviewVolume: (volume: number, muted: boolean) => invoke<void>("audio_set_preview_volume", { volume, muted }),
    audioApps: () => invoke<AppListEntry[]>("audio_apps"),

    recordingStart: () => invoke<{ id: number; path: string }>("recording_start"),
    recordingStop: () => invoke<string | null>("recording_stop"),
    streamStart: () => invoke<number>("stream_start"),
    streamStop: () => invoke<void>("stream_stop"),
    streamPrepare: () => invoke<void>("stream_prepare"),
    streamUnprepare: () => invoke<void>("stream_unprepare"),
    mediaStatus: () => invoke<MediaStatus>("media_status"),

    videoConfig: (session: number, codec: string, description: Uint8Array, width: number, height: number) =>
        invoke<void>("media_video_config", { session, codec, description: Array.from(description), width, height }),
    /** Pacote de vídeo codificado; vai como corpo binário (sem JSON) e os metadados nos cabeçalhos. */
    videoChunk: (session: number, data: Uint8Array, ptsUs: number, key: boolean) =>
        invoke<void>("media_video_chunk", data, {
            headers: { "x-session": String(session), "x-pts": String(Math.round(ptsUs)), "x-key": key ? "1" : "0" },
        }),

    streamStatus: () => invoke<ServerStatus>("stream_status"),
    firewallFix: () => invoke<void>("firewall_fix"),

    saveScreenshot: (png: Uint8Array) => invoke<string>("save_screenshot", png),
};

/** Mensagem de erro legível de uma rejeição do `invoke`. */
export function errorText(error: unknown) {
    if (typeof error === "string") return error;
    if (error instanceof Error) return error.message;
    return String(error);
}
