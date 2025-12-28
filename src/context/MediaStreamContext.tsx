import { ComponentChildren, createContext } from "preact";
import { useState } from "preact/hooks";

export type MediaStreamContextType = {
    mediaStream: MediaStream | null;
    setMediaStream: (v: MediaStream | null) => void;
};

export const MediaStreamContext = createContext<MediaStreamContextType | null>(null);

export function MediaStreamProvider({ children }: { children: ComponentChildren }) {
    const [mediaStream, setMediaStream] = useState<MediaStream | null>(null);

    return <MediaStreamContext.Provider value={{ mediaStream, setMediaStream }}>{children}</MediaStreamContext.Provider>;
}
