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
    ambientLight: boolean;
    setAmbientLight: (v: boolean) => void;
    /** Preview oculto: a captura segue (gravação/transmissão), mas nada é desenhado na tela. */
    previewHidden: boolean;
    setPreviewHidden: (v: boolean) => void;
};

export const VideoStreamContext = createContext<VideoStreamContextType | null>(null);

function loadStored<T>(key: string, fallback: T): T {
    const stored = localStorage.getItem(key);
    if (stored === null) return fallback;
    try {
        return JSON.parse(stored) as T;
    } catch {
        return fallback; // valor corrompido não pode impedir o app de abrir
    }
}

export function VideoStreamProvider({ children }: { children: ComponentChildren }) {
    const [videoStream, setVideoStream] = useState<VideoStreamType>(null);
    const [videoTrack, setVideoTrack] = useState<VideoTrackType>(null);
    const [volume, setVolume] = useState<number>(() => loadStored("volume", 1));
    const [muted, setMuted] = useState<boolean>(() => loadStored("muted", false));
    const [ambientLight, setAmbientLight] = useState<boolean>(() => loadStored("ambientLight", false));
    const [previewHidden, setPreviewHidden] = useState<boolean>(() => loadStored("previewHidden", false));

    useEffect(() => localStorage.setItem("volume", JSON.stringify(volume)), [volume]);
    useEffect(() => localStorage.setItem("muted", JSON.stringify(muted)), [muted]);
    useEffect(() => localStorage.setItem("ambientLight", JSON.stringify(ambientLight)), [ambientLight]);
    useEffect(() => localStorage.setItem("previewHidden", JSON.stringify(previewHidden)), [previewHidden]);

    return (
        <VideoStreamContext.Provider value={{ videoStream, setVideoStream, videoTrack, setVideoTrack, volume, setVolume, muted, setMuted, ambientLight, setAmbientLight, previewHidden, setPreviewHidden }}>
            {children}
        </VideoStreamContext.Provider>
    );
}
