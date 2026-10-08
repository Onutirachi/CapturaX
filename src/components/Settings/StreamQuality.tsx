// Qualidade da transmissão: resolução e FPS (mesmas listas do app, com "nativo") e bitrate
// automático por padrão.
import { useContext } from "preact/hooks";
import { NativeContext } from "../../context/NativeContext";
import { VideoStreamContext } from "../../context/VideoStreamContext";
import type { StreamConfig } from "../../core/native/api";
import { streamBitrateKbps, streamSize, type SourceInfo } from "../../core/media/presets";
import { FRAMERATE_LIST, RESOLUTION_LIST } from "../../core/utils/constants";
import { NumberInput, Segmented, Select } from "./controls";

const NATIVE = "native";

function useStreamConfig() {
    const nctx = useContext(NativeContext)!;
    const stream = nctx.config!.stream;
    const set = (patch: Partial<StreamConfig>) => nctx.updateConfig((c) => ({ ...c, stream: { ...c.stream, ...patch } }));
    return { stream, set };
}

/** Fonte atual (sem vídeo ainda, assume 1080p60 só para as estimativas). */
function useSource(): SourceInfo {
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
    const source = useSource();
    const value = stream.height ? `${stream.width}x${stream.height}` : NATIVE;
    return (
        <Select
            value={value}
            width="w-56"
            onChange={(v) => {
                if (v === NATIVE) return set({ width: 0, height: 0 });
                const [width, height] = v.split("x").map(Number);
                set({ width, height });
            }}
            options={[
                { value: NATIVE, label: `Native (${source.width} x ${source.height})` },
                // Lista do app, só o que não passa da fonte (a transmissão nunca aumenta a imagem).
                ...RESOLUTION_LIST.filter((r) => r.width <= source.width && r.height <= source.height).map((r) => ({
                    value: `${r.width}x${r.height}`,
                    label: `${r.width} x ${r.height} (${r.tag})`,
                })),
                // Valor salvo que não está na lista (ex.: fonte trocou): continua visível.
                ...(value !== NATIVE && !RESOLUTION_LIST.some((r) => `${r.width}x${r.height}` === value) ? [{ value, label: value.replace("x", " x ") }] : []),
            ]}
        />
    );
}

export function FpsSelect() {
    const { stream, set } = useStreamConfig();
    const source = useSource();
    const sourceFps = Math.round(source.fps);
    return (
        <Select
            value={stream.fps ? String(stream.fps) : NATIVE}
            width="w-40"
            onChange={(v) => set({ fps: v === NATIVE ? 0 : Number(v) })}
            options={[
                { value: NATIVE, label: `Native (${sourceFps} FPS)` },
                ...FRAMERATE_LIST.filter((f) => f <= sourceFps || f === stream.fps).map((f) => ({ value: String(f), label: `${f} FPS` })),
            ]}
        />
    );
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
