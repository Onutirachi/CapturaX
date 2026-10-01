import { useContext, useEffect, useRef } from "preact/hooks";
import { VideoStreamContext } from "../../context/VideoStreamContext";

const LATENCY_UPDATE_MS = 500;

function median(values: number[]) {
    const sorted = [...values].sort((a, b) => a - b);
    return sorted[Math.floor(sorted.length / 2)];
}

export default function VideoStream() {
    const ctx = useContext(VideoStreamContext);
    const videoRef = useRef<HTMLVideoElement>(null);

    const volume = ctx?.volume ?? 1;
    const muted = ctx?.muted ?? false;
    const videoStream = ctx?.videoStream ?? null;
    const setLatency = ctx?.setLatency;

    useEffect(() => {
        if (!videoRef.current) return;
        videoRef.current.volume = volume;
        videoRef.current.muted = muted;
    }, [volume, muted]);

    // Latência = expectedDisplayTime - captureTime de cada frame (do driver até a tela).
    // Não inclui o atraso interno da câmera/placa antes do timestamp do driver.
    useEffect(() => {
        const video = videoRef.current;
        if (!video || !videoStream || !setLatency) return;

        let samples: number[] = [];
        let callbackId = 0;

        const onFrame: VideoFrameRequestCallback = (_, metadata) => {
            if (metadata.captureTime !== undefined) {
                samples.push(metadata.expectedDisplayTime - metadata.captureTime);
            }
            callbackId = video.requestVideoFrameCallback(onFrame);
        };
        callbackId = video.requestVideoFrameCallback(onFrame);

        const interval = window.setInterval(() => {
            setLatency(samples.length > 0 ? median(samples) : null);
            samples = [];
        }, LATENCY_UPDATE_MS);

        return () => {
            video.cancelVideoFrameCallback(callbackId);
            window.clearInterval(interval);
            setLatency(null);
        };
    }, [videoStream, setLatency]);

    if (!ctx) return;

    return (
        <div className="w-full h-full flex items-center justify-center">
            <video className={`h-full w-full object-contain transition`} autoPlay playsInline srcObject={videoStream} ref={videoRef} />
        </div>
    );
}
