import { useContext } from "preact/hooks";
import { VideoStreamContext } from "../../context/VideoStreamContext";

export default function VideoStream() {
    const ctx = useContext(VideoStreamContext);
    if (!ctx) throw new Error("videoStream not found on context");

    const { videoStream } = ctx;
    // const videoRef = useRef<HTMLVideoElement>(null);

    // useEffect(() => {
    //     const video = videoRef.current;
    //     if (!video || !videoStream) return;

    //     video.srcObject = videoStream;
    //     video.play().catch(() => {});

    //     return () => {
    //         video.pause();
    //         video.srcObject = null;
    //         video.load();
    //     };
    // }, [videoStream]);

    // if (videoStream === null) return null;

    return (
        <div className="w-full h-full flex items-center justify-center">
            <video
                // key={videoStream?.id || "no-stream"}
                // id={videoStream?.id || "no-stream"}
                // ref={videoRef}
                className="w-full h-full object-contain"
                autoPlay
                playsInline
                muted
                srcObject={videoStream}
            />
        </div>
    );
}
