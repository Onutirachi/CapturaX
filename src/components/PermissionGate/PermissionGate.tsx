import { type ComponentChildren } from "preact";
import { useCallback, useEffect, useState } from "preact/hooks";
import { openUrl } from "@tauri-apps/plugin-opener";

type PermissionStatus = "checking" | "granted" | "blocked" | "notFound" | "inUse" | "error";

const MESSAGES: Record<Exclude<PermissionStatus, "checking" | "granted">, { title: string; text: string }> = {
    blocked: {
        title: "Camera access blocked",
        text: "Windows is blocking camera access for desktop apps. Allow it in the privacy settings and try again.",
    },
    notFound: {
        title: "No camera found",
        text: "Connect a camera or capture device. CapturaX will detect it automatically.",
    },
    inUse: {
        title: "Camera in use",
        text: "Another app is using the camera. Close it and try again.",
    },
    error: {
        title: "Could not access the camera",
        text: "Something went wrong while opening the camera. Try again.",
    },
};

function statusFromError(error: unknown): PermissionStatus {
    const name = error instanceof DOMException ? error.name : "";
    if (name === "NotAllowedError" || name === "SecurityError") return "blocked";
    if (name === "NotFoundError" || name === "OverconstrainedError") return "notFound";
    if (name === "NotReadableError" || name === "AbortError") return "inUse";
    return "error";
}

async function releaseStream(constraints: MediaStreamConstraints) {
    const stream = await navigator.mediaDevices.getUserMedia(constraints);
    stream.getTracks().forEach((track) => track.stop());
}

export default function PermissionGate({ children }: { children: ComponentChildren }) {
    const [status, setStatus] = useState<PermissionStatus>("checking");

    const request = useCallback(async () => {
        try {
            await releaseStream({ video: true });
        } catch (error) {
            console.error("Error to get camera permission: ", error);
            setStatus(statusFromError(error));
            return;
        }
        // Microfone é opcional: sem ele o app funciona só com vídeo.
        await releaseStream({ audio: true }).catch((error) => console.warn("Microphone unavailable: ", error));
        setStatus("granted");
    }, []);

    useEffect(() => {
        request();
    }, [request]);

    useEffect(() => {
        if (status === "granted" || status === "checking") return;
        navigator.mediaDevices.addEventListener("devicechange", request);
        return () => navigator.mediaDevices.removeEventListener("devicechange", request);
    }, [status, request]);

    if (status === "granted") return <>{children}</>;
    if (status === "checking") return null;

    const { title, text } = MESSAGES[status];

    return (
        <div className="absolute inset-0 flex items-center justify-center p-5">
            <div className="flex flex-col items-center gap-4 max-w-md p-8 text-center rounded-xl bg-(--background-color-darker)/80 backdrop-blur-xs [border:var(--border)]">
                <div className="w-10 h-10 mask-center mask-contain mask-no-repeat bg-bad mask-[url(./assets/Alerts/error.svg)]" />
                <h1 className="text-xl font-semibold">{title}</h1>
                <p className="text-(--text-muted)">{text}</p>
                <div className="flex gap-3 mt-2">
                    {status === "blocked" && (
                        <button
                            className="px-4 py-2 rounded-lg bg-(--background-color-normal) hover:brightness-125 transition"
                            onClick={() => openUrl("ms-settings:privacy-webcam")}>
                            Open privacy settings
                        </button>
                    )}
                    <button className="px-4 py-2 rounded-lg bg-good hover:brightness-110 transition" onClick={request}>
                        Try again
                    </button>
                </div>
            </div>
        </div>
    );
}
