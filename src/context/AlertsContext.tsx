import { ComponentChildren, createContext } from "preact";
import { useEffect, useState } from "preact/hooks";

/** `timeout` (ms): some sozinho depois desse tempo; sem ele, fica até ser trocado. */
export type AlertType = { text: string; color: string; icon: string; timeout?: number } | null;
export type SetAlertType = (v: AlertType | null) => void;

export type AlertsContextType = {
    alert: AlertType;
    setAlert: SetAlertType;
};

export const AlertsContext = createContext<AlertsContextType | null>(null);

export function AlertsProvider({ children }: { children: ComponentChildren }) {
    const [alert, setAlert] = useState<AlertType>(null);

    useEffect(() => {
        if (!alert?.timeout) return;
        const timer = window.setTimeout(() => setAlert(null), alert.timeout);
        return () => window.clearTimeout(timer);
    }, [alert]);

    return <AlertsContext.Provider value={{ alert, setAlert }}>{children}</AlertsContext.Provider>;
}
