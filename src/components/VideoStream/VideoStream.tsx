import { useContext } from "preact/hooks";
import { MediaStreamContext } from "../../context/MediaStreamContext";

export default function VideoStream() {
    const ctx = useContext(MediaStreamContext);
    if (!ctx) throw new Error("mediaStream not found on context");

    const { mediaStream } = ctx;

    return (
        <div className="w-full h-full flex items-center justify-center">
            <video className="w-full h-full object-contain" autoPlay playsInline srcObject={mediaStream}></video>
        </div>
    );
}
