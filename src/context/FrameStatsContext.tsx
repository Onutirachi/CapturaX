import { ComponentChildren, createContext } from "preact";
import { useState } from "preact/hooks";

export type FrameStats = {
    /** Latência medida (ms) entre a captura do frame e a exibição; null se indisponível agora. */
    latency: number | null;
    /** A fonte atual já forneceu latência alguma vez (o balão fica fixo em vez de piscar). */
    latencyAvailable: boolean;
    /** FPS medido (frames realmente exibidos por segundo); null se ainda não há medição. */
    fps: number | null;
};

export type FrameStatsContextType = {
    stats: FrameStats;
    setStats: (v: FrameStats) => void;
};

export const EMPTY_FRAME_STATS: FrameStats = { latency: null, latencyAvailable: false, fps: null };

// Separado do VideoStreamContext: atualiza 2×/s e só a StatusBar precisa re-renderizar.
export const FrameStatsContext = createContext<FrameStatsContextType | null>(null);

export function FrameStatsProvider({ children }: { children: ComponentChildren }) {
    const [stats, setStats] = useState<FrameStats>(EMPTY_FRAME_STATS);

    return <FrameStatsContext.Provider value={{ stats, setStats }}>{children}</FrameStatsContext.Provider>;
}
