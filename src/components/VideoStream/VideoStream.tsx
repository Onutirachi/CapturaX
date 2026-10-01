import { useContext, useRef } from "preact/hooks";
import { VideoStreamContext } from "../../context/VideoStreamContext";

export default function VideoStream() {
    const ctx = useContext(VideoStreamContext);
    if (!ctx) return;

    const videoRef = useRef<HTMLVideoElement>(null);

    const { videoStream } = ctx;

    return (
        <div className="w-full h-full flex items-center justify-center">
            <video className={`h-full w-full object-contain transition`} autoPlay playsInline srcObject={videoStream} ref={videoRef} />
        </div>
    );
}
