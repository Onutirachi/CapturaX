// context/FullscreenContext.tsx
import { ComponentChildren, createContext } from "preact";
import { useState } from "preact/hooks";

export type AlertType = { text: string; color: string; icon: string } | null;
export type SetAlertType = (v: AlertType | null) => void;

export type AlertsContextType = {
    alert: AlertType;
    setAlert: SetAlertType;
};

export const AlertsContext = createContext<AlertsContextType | null>(null);

export function AlertsProvider({ children }: { children: ComponentChildren }) {
    const [alert, setAlert] = useState<AlertType>(null);

    return <AlertsContext.Provider value={{ alert, setAlert }}>{children}</AlertsContext.Provider>;
}
