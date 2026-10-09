// Fonte e áudio da gravação/transmissão (as duas usam os mesmos), na janela "Gravar / Transmitir".
import { useContext } from "preact/hooks";
import { NativeContext } from "../../context/NativeContext";
import { VideoStreamContext } from "../../context/VideoStreamContext";
import { DropDown, DropHeader, DropOption } from "../../core/components/DropDown";
import Tag from "../../core/components/Tag";
import type { AudioConfig } from "../../core/native/api";
import type { SourceInfo } from "../../core/media/presets";
import { trackSettings } from "../../core/native/screenCapture";
import { captureSourceInfo, screenLabel, sourceLabel, validSource } from "../../core/utils/outputSources";
import Resolution from "../../core/utils/Resolution";
import { fpsColor, type VideoMode } from "../../core/utils/videoModes";
import { useT } from "../../i18n";
import { Slider, Toggle } from "./controls";

const PREVIEW = "preview";
const AUTO = "auto";
const NONE = "none";

const percent = (v: number) => `${Math.round(v * 100)}%`;

/** Fonte escolhida (a salva, se ainda existir). */
export function useOutputSource() {
    const nctx = useContext(NativeContext)!;
    const saved = nctx.config!.outputSource;
    // Enquanto a lista não chegou, mantém a salva (senão uma tela viraria "preview" por um instante).
    const loaded = nctx.captureSources.screens.length > 0;
    return loaded ? validSource(saved, nctx.captureSources) : saved;
}

/** Tamanho/FPS da fonte (para a qualidade) e os modos reais, quando a fonte é a placa do preview. */
export function useOutputSourceInfo(): { info: SourceInfo; modes: VideoMode[] | null } {
    const nctx = useContext(NativeContext)!;
    const vsctx = useContext(VideoStreamContext);
    const source = useOutputSource();
    if (source !== null) return { info: captureSourceInfo(source, nctx.captureSources), modes: null };
    const settings = vsctx?.videoTrack ? trackSettings(vsctx.videoTrack) : undefined;
    return { info: { width: settings?.width ?? 1920, height: settings?.height ?? 1080, fps: settings?.frameRate ?? 60 }, modes: vsctx?.deviceModes ?? null };
}

export function OutputSourcePicker({ disabled }: { disabled: boolean }) {
    const t = useT();
    const nctx = useContext(NativeContext)!;
    const previewTrack = useContext(VideoStreamContext)?.videoTrack ?? null;
    const source = useOutputSource();
    const { screens, windows } = nctx.captureSources;

    return (
        <div className="pt-2 w-full">
            <DropDown
                title={t("source.title")}
                fullWidth
                disabled={disabled}
                value={source ?? PREVIEW}
                onOpen={nctx.refreshCaptureSources}
                onChoose={(value) => nctx.updateConfig((c) => ({ ...c, outputSource: value === PREVIEW ? null : value }))}>
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

/**
 * O que vai junto no vídeo: o áudio do dispositivo (ex.: HDMI da placa), o microfone e o som dos
 * apps do PC. Os dispositivos valem na próxima vez que começar; os volumes, na hora.
 */
export function OutputAudio({ locked, close }: { locked: boolean; close: () => void }) {
    const t = useT();
    const nctx = useContext(NativeContext)!;
    const pairedId = useContext(VideoStreamContext)?.pairedAudioId ?? null;
    const audio = nctx.config!.audio;
    const inputs = nctx.devices.inputs;
    const set = (patch: Partial<AudioConfig>) => nctx.updateConfig((c) => ({ ...c, audio: { ...c.audio, ...patch } }));
    const paired = inputs.find((d) => d.id === pairedId)?.name ?? t("toolbar.noAudioLower");

    const device = !audio.recordInputEnabled ? NONE : (audio.recordInput ?? AUTO);
    const excluded = audio.excludedApps.map((exe) => exe.replace(/\.exe$/i, ""));

    return (
        <div className="flex flex-col gap-4">
            <div className="flex items-end gap-3">
                <div className="flex-1 min-w-0 pt-2">
                    <DropDown
                        title={t("output.device")}
                        fullWidth
                        disabled={locked}
                        value={device}
                        onChoose={(value) => set(value === NONE ? { recordInputEnabled: false } : { recordInputEnabled: true, recordInput: value === AUTO ? null : value })}>
                        <DropOption value={AUTO}>{t("toolbar.autoNamed", { name: paired })}</DropOption>
                        <DropOption value={NONE}>{t("toolbar.noAudio")}</DropOption>
                        {inputs.map(({ id, name }) => (
                            <DropOption key={id} value={id}>
                                {name}
                            </DropOption>
                        ))}
                    </DropDown>
                </div>
                {device !== NONE && <Slider value={audio.recordInputVolume} min={0} max={2} step={0.05} onChange={(recordInputVolume) => set({ recordInputVolume })} format={percent} />}
            </div>

            <div className="flex items-end gap-3">
                <div className="flex-1 min-w-0 pt-2">
                    <DropDown title={t("audio.microphone")} fullWidth disabled={locked} value={audio.recordMic ?? NONE} onChoose={(value) => set({ recordMic: value === NONE ? null : value })}>
                        <DropOption value={NONE}>{t("common.none")}</DropOption>
                        {inputs.map(({ id, name }) => (
                            <DropOption key={id} value={id}>
                                {name}
                            </DropOption>
                        ))}
                    </DropDown>
                </div>
                {audio.recordMic && <Slider value={audio.recordMicVolume} min={0} max={2} step={0.05} onChange={(recordMicVolume) => set({ recordMicVolume })} format={percent} />}
            </div>

            <div className="flex items-center gap-3">
                <Toggle checked={audio.recordSystem} disabled={locked} onChange={(recordSystem) => set({ recordSystem })} />
                <div className="flex-1 min-w-0">
                    <div>{t("output.pcSound")}</div>
                    <div className="text-sm text-(--text-muted)">
                        {audio.recordSystem ? (excluded.length ? t("output.pcSoundExcept", { apps: excluded.join(", ") }) : t("output.pcSoundAll")) : t("output.pcSoundOff")}{" "}
                        <button className="text-accent-light hover:underline cursor-pointer" onClick={() => (close(), nctx.openSettings("audio"))}>
                            {t("output.chooseApps")}
                        </button>
                    </div>
                </div>
                {audio.recordSystem && <Slider value={audio.recordSystemVolume} min={0} max={2} step={0.05} onChange={(recordSystemVolume) => set({ recordSystemVolume })} format={percent} />}
            </div>
            {locked && <p className="text-sm text-(--text-muted)">{t("output.lockedHint")}</p>}
        </div>
    );
}
