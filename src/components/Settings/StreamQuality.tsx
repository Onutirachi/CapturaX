// Qualidade da transmissão: resolução e FPS (mesmas listas do app, com "nativo") e bitrate
// automático por padrão.
import { useContext } from "preact/hooks";
import { NativeContext } from "../../context/NativeContext";
import { VideoStreamContext } from "../../context/VideoStreamContext";
import type { StreamConfig } from "../../core/native/api";
import { streamBitrateKbps, streamSize, type SourceInfo } from "../../core/media/presets";
import { NumberInput, Segmented } from "./controls";
import { FpsPicker, ResolutionPicker } from "./QualityPickers";
import { DropDown, DropOption } from "../../core/components/DropDown";
import Tag from "../../core/components/Tag";
import Resolution from "../../core/utils/Resolution";
import { choicesOf, formatFps, fpsColor, modeKey, parseModeKey } from "../../core/utils/videoModes";

const NATIVE = "native";

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

export function ResolutionSelect({ fullWidth }: { fullWidth?: boolean }) {
    const { stream, set } = useStreamConfig();
    return (
        <ResolutionPicker
            fullWidth={fullWidth}
            value={stream.height ? { width: stream.width, height: stream.height } : null}
            source={useSource()}
            onChange={(size) => set(size ?? { width: 0, height: 0 })}
        />
    );
}

export function FpsSelect({ fullWidth }: { fullWidth?: boolean }) {
    const { stream, set } = useStreamConfig();
    return <FpsPicker fullWidth={fullWidth} value={stream.fps || null} source={useSource()} onChange={(fps) => set({ fps: fps ?? 0 })} />;
}

/**
 * Resolução + FPS da transmissão. Com os modos reais do dispositivo: uma lista só, como a do
 * preview, com o que não passa do que está sendo capturado (a transmissão nunca aumenta a imagem).
 * Sem eles (tela): resolução e FPS separados.
 */
export function StreamQualityPicker({ fullWidth = false }: { fullWidth?: boolean }) {
    const { stream, set } = useStreamConfig();
    const source = useSource();
    const modes = useContext(VideoStreamContext)?.deviceModes ?? null;

    if (!modes) {
        return fullWidth ? (
            <div className="flex flex-col gap-5 pt-2 w-full">
                <ResolutionSelect fullWidth />
                <FpsSelect fullWidth />
            </div>
        ) : (
            <div className="flex flex-wrap items-center gap-x-4 gap-y-5 pt-2">
                <ResolutionSelect />
                <FpsSelect />
            </div>
        );
    }

    const choices = choicesOf(modes).filter((m) => m.width <= source.width && m.height <= source.height && m.fps <= source.fps + 0.5);
    const saved = stream.height ? { width: stream.width, height: stream.height, fps: stream.fps || Math.round(source.fps) } : null;
    // Qualidade salva que não é um modo deste dispositivo (ex.: veio de outra fonte): aparece na
    // lista assim mesmo, para a tela mostrar o que a transmissão vai usar de verdade.
    if (saved && !choices.some((c) => modeKey(c) === modeKey(saved))) choices.unshift(saved);
    const value = saved ? modeKey(saved) : NATIVE;
    const nativeTag = Resolution.getTag(source);
    return (
        <div className={`pt-2 ${fullWidth ? "w-full" : ""}`}>
            <DropDown
                title="Resolution & FPS"
                fullWidth={fullWidth}
                value={value}
                onChoose={(v) => {
                    const mode = v === NATIVE ? null : parseModeKey(v);
                    set(mode ? { width: mode.width, height: mode.height, fps: mode.fps } : { width: 0, height: 0, fps: 0 });
                }}>
                <DropOption value={NATIVE} className="flex gap-3">
                    {`Native (${source.width} x ${source.height})`}
                    <Tag text={nativeTag.text} color={nativeTag.color} />
                    <Tag text={`${formatFps(Math.round(source.fps))} FPS`} color={fpsColor(source.fps)} />
                </DropOption>
                {choices.map((mode) => {
                    const tag = Resolution.getTag(mode);
                    return (
                        <DropOption key={modeKey(mode)} value={modeKey(mode)} className="flex gap-3">
                            {`${mode.width} x ${mode.height}`}
                            <Tag text={tag.text} color={tag.color} />
                            <Tag text={`${formatFps(mode.fps)} FPS`} color={fpsColor(mode.fps)} />
                        </DropOption>
                    );
                })}
            </DropDown>
        </div>
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
