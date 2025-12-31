import { ComponentChildren, createContext } from "preact";
import { useState } from "preact/hooks";

export type VideoStreamType = MediaStream | null;
export type SetVideoStreamType = (v: VideoStreamType | null) => void;

export type VideoTrackType = MediaStreamTrack | null;
export type SetVideoTrackType = (v: VideoTrackType) => void;

export type VideoStreamContextType = {
    videoStream: VideoStreamType;
    setVideoStream: SetVideoStreamType;
    videoTrack: VideoTrackType;
    setVideoTrack: SetVideoTrackType;
};

export const VideoStreamContext = createContext<VideoStreamContextType | null>(null);

export function VideoStreamProvider({ children }: { children: ComponentChildren }) {
    const [videoStream, setVideoStream] = useState<VideoStreamType>(null);
    const [videoTrack, setVideoTrack] = useState<VideoTrackType>(null);

    return <VideoStreamContext.Provider value={{ videoStream, setVideoStream, videoTrack, setVideoTrack }}>{children}</VideoStreamContext.Provider>;
}
