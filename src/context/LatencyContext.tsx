import { ComponentChildren, createContext } from "preact";
import { useState } from "preact/hooks";

/** Latência medida (ms) entre a captura do frame e a exibição; null se indisponível. */
export type LatencyType = number | null;

export type LatencyContextType = {
    latency: LatencyType;
    setLatency: (v: LatencyType) => void;
};

// Separado do VideoStreamContext: atualiza 2×/s e só a StatusBar precisa re-renderizar.
export const LatencyContext = createContext<LatencyContextType | null>(null);

export function LatencyProvider({ children }: { children: ComponentChildren }) {
    const [latency, setLatency] = useState<LatencyType>(null);

    return <LatencyContext.Provider value={{ latency, setLatency }}>{children}</LatencyContext.Provider>;
}
