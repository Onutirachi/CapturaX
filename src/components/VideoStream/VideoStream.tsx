import { useContext, useEffect, useRef } from "preact/hooks";
import { MediaStreamContext } from "../../context/MediaStreamContext";

export default function VideoStream() {
    const ctx = useContext(MediaStreamContext);
    if (!ctx) throw new Error("mediaStream not found on context");

    const { mediaStream } = ctx;
    const videoRef = useRef<HTMLVideoElement>(null);

    useEffect(() => {
        const video = videoRef.current;
        if (!video) return;

        // Sempre limpa primeiro
        video.pause();
        video.srcObject = null;

        if (!mediaStream) return;

        video.srcObject = mediaStream;

        const playPromise = video.play();
        if (playPromise !== undefined) {
            playPromise.catch(() => {});
        }

        return () => {
            video.pause();
            video.srcObject = null;
        };
    }, [mediaStream]);

    return (
        <div className="w-full h-full flex items-center justify-center">
            <video ref={videoRef} className="w-full h-full object-contain" autoPlay playsInline muted />
        </div>
    );
}
