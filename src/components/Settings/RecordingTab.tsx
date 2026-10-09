import { trackSettings } from "../../core/native/screenCapture";
import { useContext, useEffect, useState } from "preact/hooks";
import { open } from "@tauri-apps/plugin-dialog";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { videoDir, join } from "@tauri-apps/api/path";
import { NativeContext } from "../../context/NativeContext";
import { VideoStreamContext } from "../../context/VideoStreamContext";
import type { RecordingConfig, RecordingPreset, VideoCodecName } from "../../core/native/api";
import { isHevcSupported, recordingSettings } from "../../core/media/presets";
import { FpsPicker, ResolutionPicker } from "./QualityPickers";
import { Button, NumberInput, Row, Section, Segmented, Select } from "./controls";

const PRESET_HINTS: Record<RecordingPreset, string> = {
    original: "Same resolution and FPS as the preview, high bitrate.",
    high: "Up to 1080p, same FPS as the preview.",
    medium: "Up to 720p, up to 60 FPS. Smaller files.",
    light: "Up to 720p at 30 FPS. Smallest files.",
    custom: "Pick resolution, FPS and bitrate yourself.",
};

export default function RecordingTab() {
    const nctx = useContext(NativeContext)!;
    const track = useContext(VideoStreamContext)?.videoTrack ?? null;
    const recording = nctx.config!.recording;
    const set = (patch: Partial<RecordingConfig>) => nctx.updateConfig((c) => ({ ...c, recording: { ...c.recording, ...patch } }));

    const [hevc, setHevc] = useState<boolean | null>(null);
    const [defaultFolder, setDefaultFolder] = useState("");
    useEffect(() => {
        isHevcSupported().then(setHevc);
        videoDir()
            .then((dir) => join(dir, "CapturaX"))
            .then(setDefaultFolder)
            .catch(() => {});
    }, []);

    const settings = track ? trackSettings(track) : undefined;
    const source = settings ? { width: settings.width ?? 1920, height: settings.height ?? 1080, fps: settings.frameRate ?? 60 } : null;
    const result = source ? recordingSettings(recording, source) : null;
    const pickerSource = source ?? { width: 1920, height: 1080, fps: 60 };
    const gbPerHour = result ? ((result.bitrate + recording.audioBitrateKbps * 1000) * 3600) / 8 / 1e9 : 0;

    const chooseFolder = async () => {
        const folder = await open({ directory: true, defaultPath: recording.folder ?? defaultFolder });
        if (typeof folder === "string") set({ folder });
    };
    const folder = recording.folder ?? defaultFolder;
    const lastFile = nctx.status?.recording?.path;

    return (
        <div className="flex flex-col gap-6">
            <Section title="Quality" description={PRESET_HINTS[recording.preset]}>
                <Row label="Preset">
                    <Segmented<RecordingPreset>
                        value={recording.preset}
                        onChange={(preset) => set({ preset })}
                        options={[
                            { value: "original", label: "Original" },
                            { value: "high", label: "High" },
                            { value: "medium", label: "Medium" },
                            { value: "light", label: "Light" },
                            { value: "custom", label: "Custom" },
                        ]}
                    />
                </Row>
                {recording.preset === "custom" && (
                    <>
                        <Row label="Resolution" hint="Never above the preview; keeps its aspect ratio.">
                            <ResolutionPicker
                                value={recording.width && recording.height ? { width: recording.width, height: recording.height } : null}
                                source={pickerSource}
                                onChange={(size) => set({ width: size?.width ?? null, height: size?.height ?? null })}
                            />
                        </Row>
                        <Row label="Frame rate">
                            <FpsPicker value={recording.fps} source={pickerSource} onChange={(fps) => set({ fps })} />
                        </Row>
                        <Row label="Video bitrate" hint="Higher = better picture and bigger files.">
                            <NumberInput value={recording.bitrateKbps / 1000} min={1} max={150} step={0.5} suffix="Mbps" onChange={(mbps) => set({ bitrateKbps: Math.round(mbps * 1000) })} />
                        </Row>
                    </>
                )}
                <Row
                    label="Codec"
                    hint={hevc === false ? "HEVC is not available on this PC (needs GPU support)." : "HEVC makes files about 40% smaller, but not every player opens it."}>
                    <Segmented<VideoCodecName>
                        value={recording.codec}
                        onChange={(codec) => set({ codec })}
                        options={[
                            { value: "h264", label: "H.264" },
                            { value: "hevc", label: "HEVC", disabled: hevc === false },
                        ]}
                    />
                </Row>
                <Row label="Audio quality">
                    <Select
                        value={String(recording.audioBitrateKbps)}
                        onChange={(v) => set({ audioBitrateKbps: Number(v) })}
                        width="w-40"
                        options={[128, 160, 192].map((k) => ({ value: String(k), label: `${k} kbps AAC` }))}
                    />
                </Row>
                {result && (
                    <Row
                        label="With the current source"
                        hint={`${result.width}x${result.height} @ ${result.fps} fps · ${(result.bitrate / 1e6).toFixed(1)} Mbps · about ${gbPerHour.toFixed(1)} GB per hour`}
                    />
                )}
            </Section>

            <Section title="Files" description="Recordings are safe while running: if the PC crashes, everything up to the last second is kept.">
                <Row label="Folder" hint={folder}>
                    <Button onClick={chooseFolder}>Change…</Button>
                    {recording.folder && <Button onClick={() => set({ folder: null })}>Default</Button>}
                    <Button onClick={() => revealItemInDir(lastFile ?? folder).catch(() => {})}>Open</Button>
                </Row>
            </Section>
        </div>
    );
}
