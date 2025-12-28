import { ComponentChildren, createContext } from "preact";
import { useState } from "preact/hooks";

export type VideoStreamType = MediaStream | null;
export type SetVideoStreamType = (v: MediaStream) => void;

export type VideoStreamContextType = {
    videoStream: VideoStreamType;
    setVideoStream: SetVideoStreamType;
};

export const VideoStreamContext = createContext<VideoStreamContextType | null>(null);

export function VideoStreamProvider({ children }: { children: ComponentChildren }) {
    const [videoStream, setVideoStream] = useState<MediaStream | null>(null);

    return <VideoStreamContext.Provider value={{ videoStream, setVideoStream }}>{children}</VideoStreamContext.Provider>;
}
