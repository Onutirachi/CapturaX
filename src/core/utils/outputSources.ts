// Fontes da gravação/transmissão: um dispositivo (`device:<id>`), uma tela (`screen:...`) ou uma
// janela (`window:...`).
import type { CaptureSources, OutputSourceId, ScreenSource } from "../native/api";
import type { SourceInfo } from "../media/presets";
import { sharedDeviceTrack } from "../native/deviceShare";
import { t } from "../../i18n";
import { chooseMode, deviceName, type VideoMode } from "./videoModes";

export const DEVICE_PREFIX = "device:";
export const deviceSource = (deviceId: string) => `${DEVICE_PREFIX}${deviceId}`;
export const sourceDeviceId = (source: OutputSourceId) => (source?.startsWith(DEVICE_PREFIX) ? source.slice(DEVICE_PREFIX.length) : null);

/** "Tela 2 - 27G2G4": o número que o Windows dá à tela + o nome do monitor. */
export const screenLabel = (screen: ScreenSource) => (screen.name ? t("toolbar.screenNamed", { number: screen.number, name: screen.name }) : t("toolbar.screen", { number: screen.number }));

/** Nome da fonte para a interface. */
export function sourceLabel(source: OutputSourceId, sources: CaptureSources, devices: MediaDeviceInfo[]): string {
    const deviceId = sourceDeviceId(source);
    if (deviceId !== null) return deviceName(devices.find((d) => d.deviceId === deviceId)?.label ?? "") || t("source.device");
    const screen = sources.screens.find((s) => s.id === source);
    if (screen) return screenLabel(screen);
    return sources.windows.find((w) => w.id === source)?.title ?? t(source?.startsWith("window:") ? "source.window" : "source.screenUnknown");
}

/** Taxa de atualização usada como FPS automático (janela: a maior entre as telas). */
export function sourceRefreshRate(source: string, sources: CaptureSources) {
    return sources.screens.find((s) => s.id === source)?.refreshRate ?? Math.max(60, ...sources.screens.map((s) => s.refreshRate));
}

/** Tamanho e FPS de uma tela/janela antes de abrir (para escolher a qualidade). */
export function captureSourceInfo(source: string, sources: CaptureSources): SourceInfo {
    const screen = sources.screens.find((s) => s.id === source);
    const window = sources.windows.find((w) => w.id === source);
    const size = screen ?? window ?? { width: 1920, height: 1080 };
    return { width: size.width & ~1, height: size.height & ~1, fps: sourceRefreshRate(source, sources) };
}

/** A fonte ainda existe? (janela fechada, dispositivo desplugado → não). */
export function sourceExists(source: string, sources: CaptureSources, devices: MediaDeviceInfo[]) {
    const deviceId = sourceDeviceId(source);
    if (deviceId !== null) return devices.some((d) => d.deviceId === deviceId);
    return sources.screens.some((s) => s.id === source) || sources.windows.some((w) => w.id === source);
}

// `resizeMode` ainda não está nos tipos do TypeScript.
type VideoConstraints = MediaTrackConstraints & { resizeMode?: "none" | "crop-and-scale" };

/**
 * Abre um dispositivo para a gravação/transmissão no modo automático. Se o preview (ou a outra
 * saída) já o tem aberto, usa uma cópia da trilha: abrir de novo falharia na maioria das placas.
 */
export async function openDeviceTrack(deviceId: string, modes: VideoMode[] | null): Promise<MediaStreamTrack> {
    const shared = sharedDeviceTrack(deviceId);
    if (shared) return shared.clone();
    const mode = modes ? chooseMode(modes, null, null) : null;
    const id = { deviceId: { exact: deviceId } };
    const attempts: VideoConstraints[] = mode
        ? [{ ...id, width: { exact: mode.width }, height: { exact: mode.height }, frameRate: { ideal: mode.fps }, resizeMode: "none" }]
        : [{ ...id, width: { ideal: 7680 }, height: { ideal: 4320 }, frameRate: { ideal: 60 }, resizeMode: "none" }];
    attempts.push(id);
    let lastError: unknown = null;
    for (const constraints of attempts) {
        try {
            return (await navigator.mediaDevices.getUserMedia({ video: constraints })).getVideoTracks()[0];
        } catch (error) {
            lastError = error;
            if (!(error instanceof DOMException && error.name === "OverconstrainedError")) break;
        }
    }
    throw lastError instanceof DOMException ? new Error(t("stream.videoFailed", { detail: lastError.name })) : lastError;
}
