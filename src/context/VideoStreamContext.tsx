import { ComponentChildren, createContext } from "preact";
import { useEffect, useState } from "preact/hooks";

export type VideoStreamType = MediaStream | null;
export type SetVideoStreamType = (v: VideoStreamType | null) => void;

export type VideoTrackType = MediaStreamTrack | null;
export type SetVideoTrackType = (v: VideoTrackType) => void;

export type VideoStreamContextType = {
    videoStream: VideoStreamType;
    setVideoStream: SetVideoStreamType;
    videoTrack: VideoTrackType;
    setVideoTrack: SetVideoTrackType;
    volume: number;
    setVolume: (v: number) => void;
    muted: boolean;
    setMuted: (v: boolean) => void;
};

export const VideoStreamContext = createContext<VideoStreamContextType | null>(null);

function loadStored<T>(key: string, fallback: T): T {
    const stored = localStorage.getItem(key);
    return stored === null ? fallback : (JSON.parse(stored) as T);
}

export function VideoStreamProvider({ children }: { children: ComponentChildren }) {
    const [videoStream, setVideoStream] = useState<VideoStreamType>(null);
    const [videoTrack, setVideoTrack] = useState<VideoTrackType>(null);
    const [volume, setVolume] = useState<number>(() => loadStored("volume", 1));
    const [muted, setMuted] = useState<boolean>(() => loadStored("muted", false));

    useEffect(() => localStorage.setItem("volume", JSON.stringify(volume)), [volume]);
    useEffect(() => localStorage.setItem("muted", JSON.stringify(muted)), [muted]);

    return (
        <VideoStreamContext.Provider value={{ videoStream, setVideoStream, videoTrack, setVideoTrack, volume, setVolume, muted, setMuted }}>
            {children}
        </VideoStreamContext.Provider>
    );
}
