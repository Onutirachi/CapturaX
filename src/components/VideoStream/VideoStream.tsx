import { useContext, useEffect, useRef } from "preact/hooks";
import { VideoStreamContext } from "../../context/VideoStreamContext";
import { FrameStatsContext, EMPTY_FRAME_STATS } from "../../context/FrameStatsContext";
import AmbientLight from "./AmbientLight";

const STATS_UPDATE_MS = 500;

function median(values: number[]) {
    const sorted = [...values].sort((a, b) => a - b);
    return sorted[Math.floor(sorted.length / 2)];
}

export default function VideoStream() {
    const ctx = useContext(VideoStreamContext);
    const setStats = useContext(FrameStatsContext)?.setStats;
    const videoRef = useRef<HTMLVideoElement>(null);

    const volume = ctx?.volume ?? 1;
    const muted = ctx?.muted ?? false;
    const videoStream = ctx?.videoStream ?? null;

    useEffect(() => {
        if (!videoRef.current) return;
        videoRef.current.volume = volume;
        videoRef.current.muted = muted;
    }, [volume, muted]);

    // Estatísticas por frame via requestVideoFrameCallback:
    // - Latência = expectedDisplayTime - captureTime (do driver até a tela). Não inclui o atraso
    //   interno da câmera/placa antes do timestamp do driver.
    // - FPS = frames exibidos (presentedFrames) no intervalo / tempo decorrido.
    useEffect(() => {
        const video = videoRef.current;
        if (!video || !videoStream || !setStats) return;

        let latencySamples: number[] = [];
        let presentedFrames: number | null = null;
        let windowStartFrames: number | null = null;
        let windowStart = performance.now();
        let callbackId = 0;

        const onFrame: VideoFrameRequestCallback = (_, metadata) => {
            if (metadata.captureTime !== undefined) {
                latencySamples.push(metadata.expectedDisplayTime - metadata.captureTime);
            }
            presentedFrames = metadata.presentedFrames;
            windowStartFrames ??= presentedFrames;
            callbackId = video.requestVideoFrameCallback(onFrame);
        };
        callbackId = video.requestVideoFrameCallback(onFrame);

        const interval = window.setInterval(() => {
            const now = performance.now();
            const frames = presentedFrames !== null && windowStartFrames !== null ? presentedFrames - windowStartFrames : 0;
            setStats({
                latency: latencySamples.length > 0 ? median(latencySamples) : null,
                fps: frames > 0 ? (frames * 1000) / (now - windowStart) : null,
            });
            latencySamples = [];
            windowStartFrames = presentedFrames;
            windowStart = now;
        }, STATS_UPDATE_MS);

        return () => {
            video.cancelVideoFrameCallback(callbackId);
            window.clearInterval(interval);
            setStats(EMPTY_FRAME_STATS);
        };
    }, [videoStream, setStats]);

    if (!ctx) return;

    return (
        <div className="w-full h-full flex items-center justify-center relative">
            {ctx.ambientLight && <AmbientLight videoRef={videoRef} />}
            <video className={`h-full w-full object-contain transition relative`} autoPlay playsInline srcObject={videoStream} ref={videoRef} />
        </div>
    );
}
