// Fonte e áudio da gravação/transmissão, usados nas janelas de gravar e de entrar ao vivo.
import { useContext } from "preact/hooks";
import { NativeContext, type OutputKind } from "../../context/NativeContext";
import { VideoStreamContext } from "../../context/VideoStreamContext";
import { DropDown, DropHeader, DropOption } from "../../core/components/DropDown";
import Tag from "../../core/components/Tag";
import type { SourceInfo } from "../../core/media/presets";
import { trackSettings } from "../../core/native/screenCapture";
import { captureSourceInfo, screenLabel, sourceLabel, validSource } from "../../core/utils/outputSources";
import Resolution from "../../core/utils/Resolution";
import { fpsColor, type VideoMode } from "../../core/utils/videoModes";
import { useT } from "../../i18n";

const PREVIEW = "preview";

const configKey = (kind: OutputKind) => (kind === "record" ? "recording" : "stream");

/** Fonte escolhida para a saída (a salva, se ainda existir). */
export function useOutputSource(kind: OutputKind) {
    const nctx = useContext(NativeContext)!;
    const saved = nctx.config![configKey(kind)].source;
    // Enquanto a lista não chegou, mantém a salva (senão uma tela viraria "preview" por um instante).
    const loaded = nctx.captureSources.screens.length > 0;
    return loaded ? validSource(saved, nctx.captureSources) : saved;
}

/** Tamanho/FPS da fonte (para a qualidade) e os modos reais, quando a fonte é a placa do preview. */
export function useOutputSourceInfo(kind: OutputKind): { info: SourceInfo; modes: VideoMode[] | null } {
    const nctx = useContext(NativeContext)!;
    const vsctx = useContext(VideoStreamContext);
    const source = useOutputSource(kind);
    if (source !== null) return { info: captureSourceInfo(source, nctx.captureSources), modes: null };
    const settings = vsctx?.videoTrack ? trackSettings(vsctx.videoTrack) : undefined;
    return { info: { width: settings?.width ?? 1920, height: settings?.height ?? 1080, fps: settings?.frameRate ?? 60 }, modes: vsctx?.deviceModes ?? null };
}

export function OutputSourcePicker({ kind }: { kind: OutputKind }) {
    const t = useT();
    const nctx = useContext(NativeContext)!;
    const previewTrack = useContext(VideoStreamContext)?.videoTrack ?? null;
    const source = useOutputSource(kind);
    const { screens, windows } = nctx.captureSources;
    const set = (value: string | null) => nctx.updateConfig((c) => ({ ...c, [configKey(kind)]: { ...c[configKey(kind)], source: value } }));

    return (
        <div className="pt-2 w-full">
            <DropDown title={t("source.title")} fullWidth value={source ?? PREVIEW} onOpen={nctx.refreshCaptureSources} onChoose={(value) => set(value === PREVIEW ? null : value)}>
                <DropOption value={PREVIEW}>{t("source.previewNamed", { name: sourceLabel(null, nctx.captureSources, previewTrack) })}</DropOption>
                {screens.length > 0 && <DropHeader>{t("toolbar.screens")}</DropHeader>}
                {screens.map((screen) => {
                    const tag = Resolution.getTag(screen);
                    return (
                        <DropOption key={screen.id} value={screen.id}>
                            {screenLabel(screen)}
                            <Tag text={tag.text} color={tag.color} />
                            <Tag text={`${screen.refreshRate} FPS`} color={fpsColor(screen.refreshRate)} />
                        </DropOption>
                    );
                })}
                {windows.length > 0 && <DropHeader>{t("toolbar.windows")}</DropHeader>}
                {windows.map((window) => (
                    <DropOption key={window.id} value={window.id}>
                        {window.title}
                        {window.app && <Tag text={window.app} />}
                    </DropOption>
                ))}
            </DropDown>
        </div>
    );
}

/** "Áudio: HDMI (ezcap) + Microfone (…) + apps do PC (sem Discord)", com atalho para mudar. */
export function AudioSummary({ close }: { close: () => void }) {
    const t = useT();
    const nctx = useContext(NativeContext)!;
    const pairedId = useContext(VideoStreamContext)?.pairedAudioId ?? null;
    const audio = nctx.config!.audio;
    const name = (id: string | null) => nctx.devices.inputs.find((d) => d.id === id)?.name ?? null;

    const parts: string[] = [];
    if (audio.recordInputEnabled) {
        const device = name(audio.recordInput ?? pairedId);
        if (device) parts.push(device);
    }
    const mic = name(audio.recordMic);
    if (mic) parts.push(t("audioSummary.mic", { name: mic }));
    if (audio.recordSystem) {
        const excluded = audio.excludedApps.map((exe) => exe.replace(/\.exe$/i, ""));
        parts.push(excluded.length ? t("audioSummary.appsExcept", { apps: excluded.join(", ") }) : t("audioSummary.apps"));
    }

    return (
        <div className="flex items-start gap-3 text-sm">
            <p className="flex-1 text-(--text-muted)">{parts.length ? parts.join(" + ") : t("audioSummary.none")}</p>
            <button className="shrink-0 text-accent-light hover:underline cursor-pointer" onClick={() => (close(), nctx.openSettings("audio"))}>
                {t("audioSummary.change")}
            </button>
        </div>
    );
}
