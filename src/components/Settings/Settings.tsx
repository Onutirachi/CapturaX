import { useContext, useEffect } from "preact/hooks";
import { NativeContext, type SettingsTab } from "../../context/NativeContext";
import Icon from "../Overlay/Icon";
import AudioTab from "./AudioTab";
import AppsTab from "./AppsTab";
import RecordingTab from "./RecordingTab";
import StreamingTab from "./StreamingTab";

const TABS: { id: SettingsTab; label: string; icon: string }[] = [
    { id: "audio", label: "Audio", icon: "./assets/Settings/audio.svg" },
    { id: "apps", label: "App audio", icon: "./assets/Settings/apps.svg" },
    { id: "recording", label: "Recording", icon: "./assets/Dock/record.svg" },
    { id: "streaming", label: "Streaming", icon: "./assets/Dock/live.svg" },
];

export default function Settings() {
    const nctx = useContext(NativeContext);
    const tab = nctx?.settingsTab ?? null;

    useEffect(() => {
        if (!tab) return;
        // Captura no document: o Esc fecha as configurações sem sair do fullscreen.
        const onKey = (e: KeyboardEvent) => {
            // Com a janela de "Go live" por cima, o Esc fecha só ela.
            if (e.key !== "Escape" || nctx?.goLiveOpen) return;
            e.stopPropagation();
            nctx?.openSettings(null);
        };
        document.addEventListener("keydown", onKey, true);
        return () => document.removeEventListener("keydown", onKey, true);
    }, [tab, nctx?.goLiveOpen]);

    if (!nctx || !tab || !nctx.config) return null;

    return (
        <div className="absolute inset-0 z-10 flex items-center justify-center p-6 bg-black/60 backdrop-blur-xs" onPointerDown={(e) => e.target === e.currentTarget && nctx.openSettings(null)}>
            <div className="flex w-[min(960px,100%)] h-[min(680px,100%)] rounded-2xl overflow-hidden bg-darker [border:var(--border)] shadow-2xl">
                <nav className="w-52 shrink-0 flex flex-col gap-1 p-3 bg-dark/60 [border-right:var(--border)]">
                    <img src="./assets/Brand/logo.svg" alt="CapturaX" className="h-5 self-start m-3 mb-5" draggable={false} />
                    {TABS.map(({ id, label, icon }) => (
                        <button
                            key={id}
                            onClick={() => nctx.openSettings(id)}
                            className={`flex items-center gap-3 h-10 px-3 rounded-lg cursor-pointer transition ${tab === id ? "bg-accent/20 text-accent-light" : "hover:bg-normal/60"}`}>
                            <div className="w-6 h-6 flex items-center justify-center">
                                <Icon src={icon} color={tab === id ? "var(--color-accent-light)" : undefined} />
                            </div>
                            {label}
                        </button>
                    ))}
                </nav>
                <div className="flex-1 flex flex-col min-w-0">
                    <header className="flex items-center justify-between h-14 px-6 [border-bottom:var(--border)]">
                        <h1 className="text-xl font-semibold">{TABS.find((t) => t.id === tab)?.label}</h1>
                        <button title="Close (Esc)" onClick={() => nctx.openSettings(null)} className="w-9 h-9 flex items-center justify-center rounded-lg hover:bg-normal cursor-pointer">
                            <Icon src="./assets/Settings/close.svg" />
                        </button>
                    </header>
                    <div className="flex-1 overflow-y-auto p-6">
                        {tab === "audio" && <AudioTab />}
                        {tab === "apps" && <AppsTab />}
                        {tab === "recording" && <RecordingTab />}
                        {tab === "streaming" && <StreamingTab />}
                    </div>
                </div>
            </div>
        </div>
    );
}
