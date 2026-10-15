import { useContext, useEffect, useState } from "preact/hooks";
import { open } from "@tauri-apps/plugin-dialog";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { videoDir, join } from "@tauri-apps/api/path";
import { NativeContext } from "../../context/NativeContext";
import type { RecordingConfig, RecordingPreset, VideoCodecName } from "../../core/native/api";
import { isHevcSupported, recordingSettings } from "../../core/media/presets";
import { FpsPicker, ResolutionPicker } from "./QualityPickers";
import { Button, NumberInput, Row, Section, Segmented, Select } from "./controls";
import { DropDown, DropOption } from "../../core/components/DropDown";
import { useOutputSourceInfo } from "./OutputSource";
import { useT, type MessageKey } from "../../i18n";

const PRESET_HINTS: Record<RecordingPreset, MessageKey> = {
    original: "recording.hint.original",
    high: "recording.hint.high",
    medium: "recording.hint.medium",
    light: "recording.hint.light",
    custom: "recording.hint.custom",
};

function useRecordingConfig() {
    const nctx = useContext(NativeContext)!;
    const recording = nctx.config!.recording;
    const set = (patch: Partial<RecordingConfig>) => nctx.updateConfig((c) => ({ ...c, recording: { ...c.recording, ...patch } }));
    return { nctx, recording, set };
}

/** Qualidade da gravação (na janela de gravar), calculada para a fonte escolhida. */
export function RecordingQuality() {
    const t = useT();
    const { recording, set } = useRecordingConfig();
    const source = useOutputSourceInfo().info;
    const result = recordingSettings(recording, source);
    const gbPerHour = ((result.bitrate + recording.audioBitrateKbps * 1000) * 3600) / 8 / 1e9;
    const presets: RecordingPreset[] = ["original", "high", "medium", "light", "custom"];

    return (
        <div className="flex flex-col gap-4">
            <div className="pt-2">
                <DropDown title={t("recording.preset")} fullWidth value={recording.preset} onChoose={(preset) => set({ preset: preset as RecordingPreset })}>
                    {presets.map((preset) => (
                        <DropOption key={preset} value={preset}>
                            {t(`recording.preset.${preset}` as MessageKey)}
                        </DropOption>
                    ))}
                </DropDown>
                <p className="text-sm text-(--text-muted) mt-1.5">{t(PRESET_HINTS[recording.preset])}</p>
            </div>
            {recording.preset === "custom" && (
                <>
                    <div className="pt-2">
                        <ResolutionPicker
                            fullWidth
                            value={recording.width && recording.height ? { width: recording.width, height: recording.height } : null}
                            source={source}
                            onChange={(size) => set({ width: size?.width ?? null, height: size?.height ?? null })}
                        />
                    </div>
                    <div className="pt-2">
                        <FpsPicker fullWidth value={recording.fps} source={source} onChange={(fps) => set({ fps })} />
                    </div>
                    <div className="flex items-center gap-3">
                        <span className="text-sm flex-1">{t("recording.videoBitrate")}</span>
                        <NumberInput value={recording.bitrateKbps / 1000} min={1} max={150} step={0.5} suffix="Mbps" onChange={(mbps) => set({ bitrateKbps: Math.round(mbps * 1000) })} />
                    </div>
                </>
            )}
            <p className="text-sm text-(--text-muted)">
                {t("recording.currentSourceHint", { size: `${result.width}x${result.height}`, fps: result.fps, mbps: (result.bitrate / 1e6).toFixed(1), gb: gbPerHour.toFixed(1) })}
            </p>
        </div>
    );
}

export default function RecordingTab() {
    const t = useT();
    const { nctx, recording, set } = useRecordingConfig();

    const [hevc, setHevc] = useState<boolean | null>(null);
    const [defaultFolder, setDefaultFolder] = useState("");
    useEffect(() => {
        isHevcSupported().then(setHevc);
        videoDir()
            .then((dir) => join(dir, "CapturaX"))
            .then(setDefaultFolder)
            .catch(() => {});
    }, []);

    const chooseFolder = async () => {
        const folder = await open({ directory: true, defaultPath: recording.folder ?? defaultFolder });
        if (typeof folder === "string") set({ folder });
    };
    const folder = recording.folder ?? defaultFolder;
    const lastFile = nctx.status?.recording?.path;

    return (
        <div className="flex flex-col gap-6">
            <Section title={t("recording.format")} description={t("recording.formatDescription")}>
                <Row
                    label={t("recording.codec")}
                    hint={t(hevc === false ? "recording.hevcUnavailable" : "recording.hevcHint")}>
                    <Segmented<VideoCodecName>
                        value={recording.codec}
                        onChange={(codec) => set({ codec })}
                        options={[
                            { value: "h264", label: "H.264" },
                            { value: "hevc", label: "HEVC", disabled: hevc === false },
                        ]}
                    />
                </Row>
                <Row label={t("recording.audioQuality")}>
                    <Select
                        value={String(recording.audioBitrateKbps)}
                        onChange={(v) => set({ audioBitrateKbps: Number(v) })}
                        width="w-40"
                        options={[128, 160, 192].map((k) => ({ value: String(k), label: `${k} kbps AAC` }))}
                    />
                </Row>
            </Section>

            <Section title={t("recording.files")} description={t("recording.filesDescription")}>
                <Row label={t("recording.folder")} hint={folder}>
                    <Button onClick={chooseFolder}>{t("recording.change")}</Button>
                    {recording.folder && <Button onClick={() => set({ folder: null })}>{t("recording.default")}</Button>}
                    <Button onClick={() => revealItemInDir(lastFile ?? folder).catch(() => {})}>{t("recording.open")}</Button>
                </Row>
            </Section>
        </div>
    );
}
