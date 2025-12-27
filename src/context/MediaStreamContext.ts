import { createContext } from "preact";
import { Dispatch, StateUpdater } from "preact/hooks";

export type MediaStreamContextType = {
    mediaStream: MediaStream | null;
    setMediaStream: Dispatch<StateUpdater<MediaStream | null>>;
};

export const MediaStreamContext = createContext<MediaStreamContextType | null>(null);
