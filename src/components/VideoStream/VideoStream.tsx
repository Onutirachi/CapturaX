import { useContext, useEffect, useRef } from "preact/hooks";
import { VideoStreamContext } from "../../context/VideoStreamContext";

export default function VideoStream() {
    const ctx = useContext(VideoStreamContext);
    const videoRef = useRef<HTMLVideoElement>(null);

    const volume = ctx?.volume ?? 1;
    const muted = ctx?.muted ?? false;

    useEffect(() => {
        if (!videoRef.current) return;
        videoRef.current.volume = volume;
        videoRef.current.muted = muted;
    }, [volume, muted]);

    if (!ctx) return;

    return (
        <div className="w-full h-full flex items-center justify-center">
            <video className={`h-full w-full object-contain transition`} autoPlay playsInline srcObject={ctx.videoStream} ref={videoRef} />
        </div>
    );
}
