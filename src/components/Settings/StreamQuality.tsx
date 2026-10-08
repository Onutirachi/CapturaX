// Qualidade da transmissão: resolução e FPS (mesmas listas do app, com "nativo") e bitrate
// automático por padrão.
import { useContext } from "preact/hooks";
import { NativeContext } from "../../context/NativeContext";
import { VideoStreamContext } from "../../context/VideoStreamContext";
import type { StreamConfig } from "../../core/native/api";
import { streamBitrateKbps, streamSize, type SourceInfo } from "../../core/media/presets";
import { NumberInput, Segmented } from "./controls";
import { FpsPicker, ResolutionPicker } from "./QualityPickers";

function useStreamConfig() {
    const nctx = useContext(NativeContext)!;
    const stream = nctx.config!.stream;
    const set = (patch: Partial<StreamConfig>) => nctx.updateConfig((c) => ({ ...c, stream: { ...c.stream, ...patch } }));
    return { stream, set };
}

/** Fonte atual (sem vídeo ainda, assume 1080p60 só para as estimativas). */
export function useSource(): SourceInfo {
    const settings = useContext(VideoStreamContext)?.videoTrack?.getSettings();
    return { width: settings?.width ?? 1920, height: settings?.height ?? 1080, fps: settings?.frameRate ?? 60 };
}

/** Upload que cada espectador consome (vídeo + áudio). */
export function usePerViewerMbps() {
    const { stream } = useStreamConfig();
    return (streamBitrateKbps(stream, useSource()) + stream.audioBitrateKbps) / 1000;
}

export function ResolutionSelect() {
    const { stream, set } = useStreamConfig();
    return (
        <ResolutionPicker
            value={stream.height ? { width: stream.width, height: stream.height } : null}
            source={useSource()}
            onChange={(size) => set(size ?? { width: 0, height: 0 })}
        />
    );
}

export function FpsSelect() {
    const { stream, set } = useStreamConfig();
    return <FpsPicker value={stream.fps || null} source={useSource()} onChange={(fps) => set({ fps: fps ?? 0 })} />;
}

export function BitrateControl() {
    const { stream, set } = useStreamConfig();
    const source = useSource();
    const auto = streamBitrateKbps({ ...stream, bitrateAuto: true }, source);
    const { width, height, fps } = streamSize(stream, source);
    return (
        <div className="flex items-center gap-3" title={`Sent as ${width}x${height} @ ${fps} fps`}>
            <Segmented
                value={stream.bitrateAuto ? "auto" : "manual"}
                onChange={(v) => set(v === "auto" ? { bitrateAuto: true } : { bitrateAuto: false, bitrateKbps: stream.bitrateAuto ? auto : stream.bitrateKbps })}
                options={[
                    { value: "auto", label: `Auto (${(auto / 1000).toFixed(1)} Mbps)` },
                    { value: "manual", label: "Manual" },
                ]}
            />
            {!stream.bitrateAuto && (
                <NumberInput value={stream.bitrateKbps / 1000} min={0.5} max={100} step={0.5} suffix="Mbps" onChange={(mbps) => set({ bitrateKbps: Math.round(mbps * 1000) })} />
            )}
        </div>
    );
}

/** Upload por espectador e atraso esperado. */
export function UploadNote() {
    const perViewer = usePerViewerMbps();
    return (
        <>
            Each viewer uses about <b>{perViewer.toFixed(1)} Mbps</b> of your upload and sees the stream about half a second to a second behind you.
        </>
    );
}
