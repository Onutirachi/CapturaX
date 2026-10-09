import { useContext } from "preact/hooks";
import { NativeContext } from "../../context/NativeContext";
import type { AudioConfig } from "../../core/native/api";
import { Row, Section, Select, Slider, Toggle, type Option } from "./controls";
import AppAudio from "./AppAudio";

const NONE = "none";
const SAME = "same";
const DEFAULT = "default";

const percent = (v: number) => `${Math.round(v * 100)}%`;
const ms = (v: number) => `${v > 0 ? "+" : ""}${v} ms`;

export default function AudioTab() {
    const nctx = useContext(NativeContext)!;
    const audio = nctx.config!.audio;
    const set = (patch: Partial<AudioConfig>) => nctx.updateConfig((c) => ({ ...c, audio: { ...c.audio, ...patch } }));

    const inputs: Option<string>[] = nctx.devices.inputs.map((d) => ({ value: d.id, label: d.name }));
    const outputs: Option<string>[] = nctx.devices.outputs.map((d) => ({ value: d.id, label: d.name }));

    return (
        <div className="flex flex-col gap-6">
            <Section title="Preview" description="What you hear while playing.">
                <Row label="Low-latency audio" hint="Plays the capture audio straight through Windows (about 10–20 ms). Turn off to use the browser engine.">
                    <Toggle checked={audio.nativePreview} onChange={(nativePreview) => set({ nativePreview })} />
                </Row>
                {audio.nativePreview && (
                    <>
                        <Row label="Input" hint="Usually the audio of the capture card.">
                            <Select
                                value={audio.previewInput ?? NONE}
                                options={[{ value: NONE, label: "No audio" }, ...inputs]}
                                onChange={(v) => set({ previewInput: v === NONE ? null : v })}
                            />
                        </Row>
                        <Row label="Output" hint="Where you listen (headset, speakers).">
                            <Select
                                value={audio.previewOutput ?? DEFAULT}
                                options={[{ value: DEFAULT, label: "Windows default" }, ...outputs]}
                                onChange={(v) => set({ previewOutput: v === DEFAULT ? null : v })}
                            />
                        </Row>
                        <Row label="Audio delay" hint="Delays the sound to line up with the picture (lip-sync). Keep at 0 for the lowest latency.">
                            <Slider value={audio.previewDelayMs} min={0} max={300} step={5} onChange={(previewDelayMs) => set({ previewDelayMs })} format={(v) => `${v} ms`} />
                        </Row>
                    </>
                )}
            </Section>

            <Section title="Recording and streaming" description="What goes into recordings and to people watching the link. Independent from the preview.">
                <Row label="Capture device audio" hint="Recorded straight from the device, at full quality.">
                    <Toggle checked={audio.recordInputEnabled} onChange={(recordInputEnabled) => set({ recordInputEnabled })} />
                </Row>
                {audio.recordInputEnabled && (
                    <Row label="Device">
                        <Select
                            value={audio.recordInput ?? SAME}
                            options={[{ value: SAME, label: "Same as preview" }, ...inputs]}
                            onChange={(v) => set({ recordInput: v === SAME ? null : v })}
                        />
                        <Slider value={audio.recordInputVolume} min={0} max={2} step={0.05} onChange={(recordInputVolume) => set({ recordInputVolume })} format={percent} />
                    </Row>
                )}
                <Row label="Microphone" hint="Optional, for commentary.">
                    <Select value={audio.recordMic ?? NONE} options={[{ value: NONE, label: "None" }, ...inputs]} onChange={(v) => set({ recordMic: v === NONE ? null : v })} />
                    {audio.recordMic && <Slider value={audio.recordMicVolume} min={0} max={2} step={0.05} onChange={(recordMicVolume) => set({ recordMicVolume })} format={percent} />}
                </Row>
                {audio.recordMic && (
                    <Row
                        label="Microphone on separate tracks"
                        hint="Recordings also get “Game and apps (no mic)” and “Microphone” as extra audio tracks, so a video editor can remove or adjust your voice. Players keep playing the full mix. The stream always has a single track.">
                        <Toggle checked={audio.recordMicSeparate} onChange={(recordMicSeparate) => set({ recordMicSeparate })} />
                    </Row>
                )}
                <Row label="App audio" hint="Sound from other apps (music, browser…), minus the ones you leave out below.">
                    {audio.recordSystem && (
                        <Slider value={audio.recordSystemVolume} min={0} max={2} step={0.05} onChange={(recordSystemVolume) => set({ recordSystemVolume })} format={percent} />
                    )}
                    <Toggle checked={audio.recordSystem} onChange={(recordSystem) => set({ recordSystem })} />
                </Row>
                <Row label="Audio/video sync" hint="Fine-tunes recordings and streams. Positive values delay the audio.">
                    <Slider value={audio.avOffsetMs} min={-300} max={300} step={5} onChange={(avOffsetMs) => set({ avOffsetMs })} format={ms} />
                </Row>
            </Section>

            <AppAudio />
        </div>
    );
}
