// context/FullscreenContext.tsx
import { ComponentChildren, createContext } from "preact";
import { useState } from "preact/hooks";

export type FullscreenContextType = {
    fullscreen: boolean;
    setFullscreen: (v: boolean) => void;
};

export const FullscreenContext = createContext<FullscreenContextType | null>(null);

export function FullscreenProvider({ children }: { children: ComponentChildren }) {
    const [fullscreen, setFullscreen] = useState(false);

    return <FullscreenContext.Provider value={{ fullscreen, setFullscreen }}>{children}</FullscreenContext.Provider>;
}
