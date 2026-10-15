// Fonte e áudio da gravação/transmissão (as duas usam os mesmos), na janela "Gravar / Transmitir".
import { useContext, useState } from "preact/hooks";
import { NativeContext } from "../../context/NativeContext";
import { VideoStreamContext } from "../../context/VideoStreamContext";
import { DropDown, DropHeader, DropOption } from "../../core/components/DropDown";
import Tag from "../../core/components/Tag";
import type { AudioConfig } from "../../core/native/api";
import type { SourceInfo } from "../../core/media/presets";
import { trackSettings } from "../../core/native/screenCapture";
import { trackDeviceId } from "../../core/native/deviceShare";
import { captureSourceInfo, deviceSource, screenLabel, sourceDeviceId } from "../../core/utils/outputSources";
import Resolution from "../../core/utils/Resolution";
import { chooseMode, deviceName, findDevice, fpsColor, type VideoMode } from "../../core/utils/videoModes";
import Icon from "../Overlay/Icon";
import { useT } from "../../i18n";
import { Button, Slider, Toggle } from "./controls";
import AppAudio from "./AppAudio";

const AUTO = "auto";
const NONE = "none";

const percent = (v: number) => `${Math.round(v * 100)}%`;

/** Tamanho/FPS da fonte (para a qualidade) e os modos reais, quando a fonte é um dispositivo. */
export function useOutputSourceInfo(): { info: SourceInfo; modes: VideoMode[] | null } {
    const nctx = useContext(NativeContext)!;
    const vsctx = useContext(VideoStreamContext);
    const source = nctx.outputSource;
    const deviceId = sourceDeviceId(source);
    if (source && deviceId === null) return { info: captureSourceInfo(source, nctx.captureSources), modes: null };
    // O dispositivo do preview: o que está aberto agora.
    if (deviceId === null || deviceId === trackDeviceId(vsctx?.videoTrack ?? null)) {
        const settings = vsctx?.videoTrack ? trackSettings(vsctx.videoTrack) : undefined;
        return { info: { width: settings?.width ?? 1920, height: settings?.height ?? 1080, fps: settings?.frameRate ?? 60 }, modes: vsctx?.deviceModes ?? null };
    }
    // Outro dispositivo: o modo automático dele (é o que a saída vai abrir).
    const label = vsctx?.videoDevices.find((d) => d.deviceId === deviceId)?.label ?? "";
    const modes = findDevice(vsctx?.allDeviceModes ?? [], label)?.modes ?? null;
    const auto = modes ? chooseMode(modes, null, null) : null;
    return { info: auto ?? { width: 1920, height: 1080, fps: 60 }, modes };
}

export function OutputSourcePicker({ disabled }: { disabled: boolean }) {
    const t = useT();
    const nctx = useContext(NativeContext)!;
    const devices = useContext(VideoStreamContext)?.videoDevices ?? [];
    const { screens, windows } = nctx.captureSources;

    return (
        <div className="pt-2 w-full">
            <DropDown
                title={t("source.title")}
                fullWidth
                disabled={disabled}
                value={nctx.outputSource ?? ""}
                onOpen={nctx.refreshCaptureSources}
                onChoose={(value) => nctx.updateConfig((c) => ({ ...c, outputSource: value }))}>
                {devices.length > 0 && <DropHeader>{t("source.devices")}</DropHeader>}
                {devices.map((device) => (
                    <DropOption key={device.deviceId} value={deviceSource(device.deviceId)}>
                        {deviceName(device.label)}
                    </DropOption>
                ))}
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

function Volume({ value, onChange }: { value: number; onChange: (value: number) => void }) {
    const t = useT();
    return (
        <div className="flex items-center gap-3 pl-1">
            <span className="w-16 text-sm text-(--text-muted)">{t("output.volume")}</span>
            <Slider value={value} min={0} max={2} step={0.05} onChange={onChange} format={percent} />
        </div>
    );
}

/** Escolher os apps do som do PC sem sair da janela; salva na hora. */
function AppPicker({ close }: { close: () => void }) {
    const t = useT();
    return (
        <div className="fixed inset-0 z-30 flex items-center justify-center p-6 bg-black/50" onPointerDown={(e) => e.target === e.currentTarget && close()}>
            <div className="flex flex-col w-[min(560px,100%)] max-h-full rounded-2xl bg-darker [border:var(--border)] shadow-2xl overflow-hidden">
                <div className="flex-1 overflow-y-auto p-6">
                    <AppAudio />
                </div>
                <footer className="flex justify-end p-4 [border-top:var(--border)]">
                    <Button variant="accent" onClick={close}>
                        {t("output.done")}
                    </Button>
                </footer>
            </div>
        </div>
    );
}

/**
 * O que vai junto no vídeo, um embaixo do outro: o áudio do dispositivo (ex.: HDMI da placa), o
 * microfone e o som dos apps do PC. Os dispositivos valem na próxima vez que começar; os volumes,
 * na hora.
 */
export function OutputAudio({ locked }: { locked: boolean }) {
    const t = useT();
    const nctx = useContext(NativeContext)!;
    const [picking, setPicking] = useState(false);
    const audio = nctx.config!.audio;
    const inputs = nctx.devices.inputs;
    const set = (patch: Partial<AudioConfig>) => nctx.updateConfig((c) => ({ ...c, audio: { ...c.audio, ...patch } }));
    const auto = inputs.find((d) => d.id === nctx.outputAutoAudioId)?.name ?? t("toolbar.noAudioLower");

    const device = !audio.recordInputEnabled ? NONE : (audio.recordInput ?? AUTO);
    const excluded = audio.excludedApps.map((exe) => exe.replace(/\.exe$/i, ""));

    return (
        <div className="flex flex-col gap-4">
            <div className="flex flex-col gap-2">
                <div className="pt-2">
                    <DropDown
                        title={t("output.device")}
                        fullWidth
                        disabled={locked}
                        value={device}
                        onChoose={(value) => set(value === NONE ? { recordInputEnabled: false } : { recordInputEnabled: true, recordInput: value === AUTO ? null : value })}>
                        <DropOption value={AUTO}>{t("toolbar.autoNamed", { name: auto })}</DropOption>
                        <DropOption value={NONE}>{t("toolbar.noAudio")}</DropOption>
                        {inputs.map(({ id, name }) => (
                            <DropOption key={id} value={id}>
                                {name}
                            </DropOption>
                        ))}
                    </DropDown>
                </div>
                {device !== NONE && <Volume value={audio.recordInputVolume} onChange={(recordInputVolume) => set({ recordInputVolume })} />}
            </div>

            <div className="flex flex-col gap-2">
                <div className="pt-2">
                    <DropDown title={t("audio.microphone")} fullWidth disabled={locked} value={audio.recordMic ?? NONE} onChoose={(value) => set({ recordMic: value === NONE ? null : value })}>
                        <DropOption value={NONE}>{t("common.none")}</DropOption>
                        {inputs.map(({ id, name }) => (
                            <DropOption key={id} value={id}>
                                {name}
                            </DropOption>
                        ))}
                    </DropDown>
                </div>
                {audio.recordMic && <Volume value={audio.recordMicVolume} onChange={(recordMicVolume) => set({ recordMicVolume })} />}
            </div>

            <div className="flex flex-col gap-2">
                <div className="flex items-center gap-3">
                    <Toggle checked={audio.recordSystem} disabled={locked} onChange={(recordSystem) => set({ recordSystem })} />
                    <div className="flex-1 min-w-0">
                        <div>{t("output.pcSound")}</div>
                        <div className="text-sm text-(--text-muted)">
                            {audio.recordSystem ? (excluded.length ? t("output.pcSoundExcept", { apps: excluded.join(", ") }) : t("output.pcSoundAll")) : t("output.pcSoundOff")}
                        </div>
                    </div>
                    <button
                        title={t("output.chooseApps")}
                        onClick={() => setPicking(true)}
                        className="h-9 px-3 flex items-center gap-2 rounded-lg bg-normal hover:brightness-125 cursor-pointer text-sm shrink-0">
                        <Icon src="./assets/Settings/apps.svg" />
                        {t("output.chooseApps")}
                    </button>
                </div>
                {audio.recordSystem && <Volume value={audio.recordSystemVolume} onChange={(recordSystemVolume) => set({ recordSystemVolume })} />}
            </div>
            {locked && <p className="text-sm text-(--text-muted)">{t("output.lockedHint")}</p>}
            {picking && <AppPicker close={() => setPicking(false)} />}
        </div>
    );
}
