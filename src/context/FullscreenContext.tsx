// context/FullscreenContext.tsx
import { ComponentChildren, createContext } from "preact";
import { useEffect, useRef, useState } from "preact/hooks";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";

export type FullscreenContextType = {
    fullscreen: boolean;
    setFullscreen: (v: boolean) => Promise<void>;
    toggleFullscreen: () => Promise<void>;
};

export const FullscreenContext = createContext<FullscreenContextType | null>(null);

export function FullscreenProvider({ children }: { children: ComponentChildren }) {
    const [fullscreen, setFullscreenState] = useState(false);
    const wasMaximized = useRef(false);

    const setFullscreen = async (value: boolean) => {
        const win = getCurrentWindow();
        if (value === (await win.isFullscreen())) return;

        // Janela sem decorations maximizada entra em fullscreen presa à área de trabalho
        // (o tao limita ao rcWork), então desmaximiza antes e restaura ao sair. As animações
        // do Windows ficam desligadas durante a troca para não aparecer o "encolhe e cresce".
        const needsRestore = value ? await win.isMaximized() : wasMaximized.current;
        if (needsRestore) await invoke("set_window_animations", { enabled: false });

        if (value && needsRestore) {
            wasMaximized.current = true;
            await win.unmaximize();
        }

        await win.setFullscreen(value);
        setFullscreenState(value);

        if (!value && needsRestore) {
            wasMaximized.current = false;
            await win.maximize();
        }

        if (needsRestore) await invoke("set_window_animations", { enabled: true });
    };

    const toggleFullscreen = async () => {
        await setFullscreen(!(await getCurrentWindow().isFullscreen()));
    };

    useEffect(() => {
        // Mantém o estado certo se a janela sair do fullscreen por fora (ex.: atalho do sistema).
        const syncFullscreen = async () => setFullscreenState(await getCurrentWindow().isFullscreen());

        const handleKeyDown = (e: KeyboardEvent) => {
            if (e.key === "F11") {
                e.preventDefault();
                toggleFullscreen();
            } else if (e.key === "Escape") {
                setFullscreen(false);
            }
        };

        syncFullscreen();
        window.addEventListener("resize", syncFullscreen);
        window.addEventListener("keydown", handleKeyDown);
        return () => {
            window.removeEventListener("resize", syncFullscreen);
            window.removeEventListener("keydown", handleKeyDown);
        };
    }, []);

    return <FullscreenContext.Provider value={{ fullscreen, setFullscreen, toggleFullscreen }}>{children}</FullscreenContext.Provider>;
}
