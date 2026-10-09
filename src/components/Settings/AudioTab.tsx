import { useContext } from "preact/hooks";
import { NativeContext } from "../../context/NativeContext";
import type { AudioConfig } from "../../core/native/api";
import { Row, Section, Select, Slider, Toggle, type Option } from "./controls";
import AppAudio from "./AppAudio";
import { useT } from "../../i18n";

const NONE = "none";
const AUTO = "auto";
const SAME = "same";
const DEFAULT = "default";

const percent = (v: number) => `${Math.round(v * 100)}%`;
const ms = (v: number) => `${v > 0 ? "+" : ""}${v} ms`;

export default function AudioTab() {
    const t = useT();
    const nctx = useContext(NativeContext)!;
    const audio = nctx.config!.audio;
    const set = (patch: Partial<AudioConfig>) => nctx.updateConfig((c) => ({ ...c, audio: { ...c.audio, ...patch } }));

    const inputs: Option<string>[] = nctx.devices.inputs.map((d) => ({ value: d.id, label: d.name }));
    const outputs: Option<string>[] = nctx.devices.outputs.map((d) => ({ value: d.id, label: d.name }));

    return (
        <div className="flex flex-col gap-6">
            <Section title={t("audio.preview")} description={t("audio.previewDescription")}>
                <Row label={t("audio.lowLatency")} hint={t("audio.lowLatencyHint")}>
                    <Toggle checked={audio.nativePreview} onChange={(nativePreview) => set({ nativePreview })} />
                </Row>
                {audio.nativePreview && (
                    <>
                        <Row label={t("audio.input")} hint={t("audio.inputHint")}>
                            <Select
                                value={audio.previewInputAuto ? AUTO : (audio.previewInput ?? NONE)}
                                options={[{ value: AUTO, label: t("audio.inputAuto") }, { value: NONE, label: t("toolbar.noAudio") }, ...inputs]}
                                onChange={(v) => set(v === AUTO ? { previewInputAuto: true } : { previewInputAuto: false, previewInput: v === NONE ? null : v })}
                            />
                        </Row>
                        <Row label={t("audio.output")} hint={t("audio.outputHint")}>
                            <Select
                                value={audio.previewOutput ?? DEFAULT}
                                options={[{ value: DEFAULT, label: t("toolbar.windowsDefault") }, ...outputs]}
                                onChange={(v) => set({ previewOutput: v === DEFAULT ? null : v })}
                            />
                        </Row>
                        <Row label={t("audio.delay")} hint={t("audio.delayHint")}>
                            <Slider value={audio.previewDelayMs} min={0} max={300} step={5} onChange={(previewDelayMs) => set({ previewDelayMs })} format={(v) => `${v} ms`} />
                        </Row>
                    </>
                )}
            </Section>

            <Section title={t("audio.recordingAndStreaming")} description={t("audio.recordingAndStreamingDescription")}>
                <Row label={t("audio.deviceAudio")} hint={t("audio.deviceAudioHint")}>
                    <Toggle checked={audio.recordInputEnabled} onChange={(recordInputEnabled) => set({ recordInputEnabled })} />
                </Row>
                {audio.recordInputEnabled && (
                    <Row label={t("audio.device")}>
                        <Select
                            value={audio.recordInput ?? SAME}
                            options={[{ value: SAME, label: t("audio.sameAsPreview") }, ...inputs]}
                            onChange={(v) => set({ recordInput: v === SAME ? null : v })}
                        />
                        <Slider value={audio.recordInputVolume} min={0} max={2} step={0.05} onChange={(recordInputVolume) => set({ recordInputVolume })} format={percent} />
                    </Row>
                )}
                <Row label={t("audio.microphone")} hint={t("audio.microphoneHint")}>
                    <Select value={audio.recordMic ?? NONE} options={[{ value: NONE, label: t("common.none") }, ...inputs]} onChange={(v) => set({ recordMic: v === NONE ? null : v })} />
                    {audio.recordMic && <Slider value={audio.recordMicVolume} min={0} max={2} step={0.05} onChange={(recordMicVolume) => set({ recordMicVolume })} format={percent} />}
                </Row>
                {audio.recordMic && (
                    <Row
                        label={t("audio.separateMic")}
                        hint={t("audio.separateMicHint")}>
                        <Toggle checked={audio.recordMicSeparate} onChange={(recordMicSeparate) => set({ recordMicSeparate })} />
                    </Row>
                )}
                <Row label={t("audio.appAudio")} hint={t("audio.appAudioHint")}>
                    {audio.recordSystem && (
                        <Slider value={audio.recordSystemVolume} min={0} max={2} step={0.05} onChange={(recordSystemVolume) => set({ recordSystemVolume })} format={percent} />
                    )}
                    <Toggle checked={audio.recordSystem} onChange={(recordSystem) => set({ recordSystem })} />
                </Row>
                <Row label={t("audio.sync")} hint={t("audio.syncHint")}>
                    <Slider value={audio.avOffsetMs} min={-300} max={300} step={5} onChange={(avOffsetMs) => set({ avOffsetMs })} format={ms} />
                </Row>
            </Section>

            <AppAudio />
        </div>
    );
}
