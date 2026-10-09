// Fontes da gravação/transmissão: a mesma do preview, uma tela ou uma janela.
import type { CaptureSources, OutputSourceId, ScreenSource } from "../native/api";
import type { SourceInfo } from "../media/presets";
import { t } from "../../i18n";
import { deviceName } from "./videoModes";

/** "Tela 2 - 27G2G4": o número que o Windows dá à tela + o nome do monitor. */
export const screenLabel = (screen: ScreenSource) => (screen.name ? t("toolbar.screenNamed", { number: screen.number, name: screen.name }) : t("toolbar.screen", { number: screen.number }));

/** Nome da fonte para a interface. `previewTrack`: o que o preview mostra (para "a mesma do preview"). */
export function sourceLabel(source: OutputSourceId, sources: CaptureSources, previewTrack: MediaStreamTrack | null): string {
    if (source === null) return previewTrack ? deviceName(previewTrack.label) : t("source.preview");
    const screen = sources.screens.find((s) => s.id === source);
    if (screen) return screenLabel(screen);
    return sources.windows.find((w) => w.id === source)?.title ?? t(source.startsWith("window:") ? "source.window" : "source.screenUnknown");
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

/** Fonte salva que não existe mais (janela fechada desde a última vez) vira "a mesma do preview". */
export function validSource(source: OutputSourceId, sources: CaptureSources): OutputSourceId {
    if (source === null) return null;
    return sources.screens.some((s) => s.id === source) || sources.windows.some((w) => w.id === source) ? source : null;
}
