// Áudio que muda pouco. O que você ouve é escolhido na barra de cima; o que vai na
// gravação/transmissão (dispositivo, microfone, som do PC), na janela "Gravar / Transmitir".
import { useContext } from "preact/hooks";
import { NativeContext } from "../../context/NativeContext";
import type { AudioConfig } from "../../core/native/api";
import { Row, Section, Select, Slider, Toggle, type Option } from "./controls";
import AppAudio from "./AppAudio";
import { useT } from "../../i18n";

const DEFAULT = "default";

const ms = (v: number) => `${v > 0 ? "+" : ""}${v} ms`;

export default function AudioTab() {
    const t = useT();
    const nctx = useContext(NativeContext)!;
    const audio = nctx.config!.audio;
    const set = (patch: Partial<AudioConfig>) => nctx.updateConfig((c) => ({ ...c, audio: { ...c.audio, ...patch } }));

    const outputs: Option<string>[] = nctx.devices.outputs.map((d) => ({ value: d.id, label: d.name }));

    return (
        <div className="flex flex-col gap-6">
            <Section title={t("audio.preview")} description={t("audio.previewDescription")}>
                <Row label={t("audio.lowLatency")} hint={t("audio.lowLatencyHint")}>
                    <Toggle checked={audio.nativePreview} onChange={(nativePreview) => set({ nativePreview })} />
                </Row>
                {audio.nativePreview && (
                    <>
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
                <Row label={t("audio.sync")} hint={t("audio.syncHint")}>
                    <Slider value={audio.avOffsetMs} min={-300} max={300} step={5} onChange={(avOffsetMs) => set({ avOffsetMs })} format={ms} />
                </Row>
                <Row label={t("audio.separateMic")} hint={t("audio.separateMicHint")}>
                    <Toggle checked={audio.recordMicSeparate} onChange={(recordMicSeparate) => set({ recordMicSeparate })} />
                </Row>
            </Section>

            <AppAudio />
        </div>
    );
}
