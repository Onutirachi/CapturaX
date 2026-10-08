import type { AlertType } from "../../context/AlertsContext";

export const alerts = {
    error: (text: string): AlertType => ({ text, color: "var(--color-bad)", icon: "./assets/Alerts/error.svg" }),
    warning: (text: string): AlertType => ({ text, color: "goldenrod", icon: "./assets/Alerts/warning.svg" }),
    info: (text: string, timeout = 4000): AlertType => ({ text, color: "var(--color-info)", icon: "./assets/Alerts/info.svg", timeout }),
    success: (text: string, timeout = 5000): AlertType => ({ text, color: "var(--color-accent)", icon: "./assets/Alerts/check.svg", timeout }),
};

/** Só o nome do arquivo de um caminho do Windows. */
export function fileName(path: string) {
    return path.split(/[\\/]/).pop() ?? path;
}
