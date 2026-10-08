import { useContext } from "preact/hooks";
import Tag from "../../core/components/Tag";
import Resolution from "../../core/utils/Resolution";
import { VideoStreamContext } from "../../context/VideoStreamContext";
import { FrameStatsContext } from "../../context/FrameStatsContext";
import { NativeContext } from "../../context/NativeContext";

import Icon from "./Icon";
import ButtonToggle from "../../core/components/ButtonToggle";
import InfoBubble from "./InfoBubble";
import Alerts from "./Alerts";

type ControlsProps = { fixed: boolean; setFixed: (value: boolean) => void };

function Controls({ fixed, setFixed }: ControlsProps) {
    return (
        <>
            <ButtonToggle on={() => setFixed(true)} off={() => setFixed(false)} storeState="statusBarFixed">
                <InfoBubble className="cursor-pointer">
                    {fixed ? <Icon src="./assets/StatusBar/pin_enable.svg" /> : <Icon src="./assets/StatusBar/pin_disable.svg" />}
                </InfoBubble>
            </ButtonToggle>
        </>
    );
}

function formatDuration(seconds: number) {
    const s = Math.floor(seconds);
    const hh = Math.floor(s / 3600);
    const mm = String(Math.floor((s % 3600) / 60)).padStart(2, "0");
    const ss = String(s % 60).padStart(2, "0");
    return hh > 0 ? `${hh}:${mm}:${ss}` : `${mm}:${ss}`;
}

function formatBytes(bytes: number) {
    if (bytes >= 1e9) return `${(bytes / 1e9).toFixed(2)} GB`;
    return `${Math.round(bytes / 1e6)} MB`;
}

/** Indicadores de gravação e transmissão em andamento. */
function MediaBubbles() {
    const nctx = useContext(NativeContext);
    if (!nctx) return null;
    const recording = nctx.status?.sessions.find((s) => s.kind === "recording");
    const streaming = nctx.status?.sessions.find((s) => s.kind === "stream");
    const viewers = nctx.server?.viewers.length ?? 0;

    return (
        <>
            {nctx.recording !== "idle" && (
                <InfoBubble>
                    <span className="w-3 h-3 rounded-full bg-bad animate-pulse" />
                    <span title={nctx.status?.recording?.path}>
                        {nctx.recording === "stopping" ? "Saving…" : `REC ${formatDuration(recording?.elapsedSecs ?? 0)} · ${formatBytes(recording?.bytes ?? 0)}`}
                    </span>
                </InfoBubble>
            )}
            {nctx.streaming !== "idle" && (
                <InfoBubble>
                    <span className="px-1.5 rounded-sm bg-accent text-sm font-semibold">LIVE</span>
                    <span title="Viewers">{`${formatDuration(streaming?.elapsedSecs ?? 0)} · ${viewers} watching`}</span>
                </InfoBubble>
            )}
        </>
    );
}

export default function StatusBarContent({ fixed, setFixed }: { fixed: boolean; setFixed: (value: boolean) => void }) {
    const vsctx = useContext(VideoStreamContext);
    const { latency, latencyAvailable, fps: measuredFps } = useContext(FrameStatsContext)?.stats ?? { latency: null, latencyAvailable: false, fps: null };
    if (!vsctx) return null;
    const { videoTrack } = vsctx;

    if (!videoTrack) {
        return (
            <>
                <Alerts />
                <MediaBubbles />
                <Controls fixed={fixed} setFixed={setFixed} />
            </>
        );
    }

    const settings = videoTrack.getSettings();
    const width = settings.width || 0;
    const height = settings.height || 0;
    const resolutionTag = Resolution.getApproximateTag({ width, height });
    const aspectRatioTag = Resolution.getAspectRatio({ width, height });
    const isCapturing = videoTrack.readyState === "live" ? true : false;
    const configuredFps = settings.frameRate || 0;

    return (
        <>
            <Alerts />
            <MediaBubbles />
            <InfoBubble>
                {isCapturing ? (
                    <>
                        <Icon src="./assets/StatusBar/play.svg" />
                        <span>Capturing</span>
                    </>
                ) : (
                    <>
                        <Icon src="./assets/StatusBar/pause.svg" />
                        <span>Not capturing</span>
                    </>
                )}
            </InfoBubble>
            <InfoBubble>
                <span>{`${settings.width} x ${settings.height}`}</span>
                {resolutionTag && <Tag text={resolutionTag.text} color={resolutionTag.color}></Tag>}
                {aspectRatioTag && <Tag text={aspectRatioTag}></Tag>}
            </InfoBubble>
            <InfoBubble className="min-w-24 tabular-nums">
                <span title={`Configured: ${Math.round(configuredFps)} FPS`}>{`${Math.round(measuredFps ?? configuredFps)} FPS`}</span>
            </InfoBubble>
            {latencyAvailable && (
                <InfoBubble className="min-w-36 tabular-nums">
                    <span title="Time from frame capture (driver timestamp) to display">{latency !== null ? `Latency ${Math.round(latency)} ms` : "Latency —"}</span>
                </InfoBubble>
            )}
            <Controls fixed={fixed} setFixed={setFixed} />
        </>
    );
}
