// Janela que aparece ao clicar em transmitir: qualidade na hora e o link para copiar antes de começar.
import { useContext, useEffect } from "preact/hooks";
import { NativeContext } from "../../context/NativeContext";
import type { StreamConfig } from "../../core/native/api";
import Icon from "../Overlay/Icon";
import { Button, CopyField, Note, NumberInput, Segmented, Select } from "./controls";

type Preset = { id: string; label: string; height: number; fps: number; bitrateKbps: number };

const PRESETS: Preset[] = [
    { id: "720p30", label: "720p 30", height: 720, fps: 30, bitrateKbps: 3000 },
    { id: "720p60", label: "720p 60", height: 720, fps: 60, bitrateKbps: 4500 },
    { id: "1080p30", label: "1080p 30", height: 1080, fps: 30, bitrateKbps: 5000 },
    { id: "1080p60", label: "1080p 60", height: 1080, fps: 60, bitrateKbps: 8000 },
];
const CUSTOM = "custom";

function presetOf(stream: StreamConfig) {
    return PRESETS.find((p) => p.height === stream.height && p.fps === stream.fps && p.bitrateKbps === stream.bitrateKbps)?.id ?? CUSTOM;
}

export default function GoLiveDialog() {
    const nctx = useContext(NativeContext);
    const open = nctx?.goLiveOpen ?? false;

    useEffect(() => {
        if (!open) return;
        const onKey = (e: KeyboardEvent) => {
            if (e.key !== "Escape") return;
            e.stopImmediatePropagation();
            nctx?.cancelGoLive();
        };
        document.addEventListener("keydown", onKey, true);
        return () => document.removeEventListener("keydown", onKey, true);
    }, [open]);

    if (!nctx || !open || !nctx.config) return null;
    const stream = nctx.config.stream;
    const server = nctx.server;
    const set = (patch: Partial<StreamConfig>) => nctx.updateConfig((c) => ({ ...c, stream: { ...c.stream, ...patch } }));
    const preset = presetOf(stream);
    const uploadMbps = ((stream.bitrateKbps + stream.audioBitrateKbps) * stream.maxViewers) / 1000;
    const upnp = server?.upnp;
    const starting = nctx.streaming === "starting";

    return (
        <div className="absolute inset-0 z-20 flex items-center justify-center p-6 bg-black/60 backdrop-blur-xs" onPointerDown={(e) => e.target === e.currentTarget && nctx.cancelGoLive()}>
            <div className="flex flex-col gap-5 w-[min(560px,100%)] p-6 rounded-2xl bg-darker [border:var(--border)] shadow-2xl">
                <header className="flex items-center gap-3">
                    <div className="w-9 h-9 flex items-center justify-center rounded-lg bg-accent/20">
                        <Icon src="./assets/Dock/live.svg" color="var(--color-accent)" />
                    </div>
                    <h1 className="text-xl font-semibold flex-1">Go live</h1>
                    <button title="Cancel (Esc)" onClick={() => nctx.cancelGoLive()} className="w-9 h-9 flex items-center justify-center rounded-lg hover:bg-normal cursor-pointer">
                        <Icon src="./assets/Settings/close.svg" />
                    </button>
                </header>

                <section className="flex flex-col gap-2">
                    <h2 className="text-sm text-(--text-muted)">Link to share</h2>
                    {server?.publicUrl ? (
                        <CopyField value={server.publicUrl} />
                    ) : server?.localUrl ? (
                        <>
                            <CopyField value={server.localUrl} />
                            <p className="text-sm text-(--text-muted)">
                                {upnp?.state === "searching"
                                    ? "Opening the port on your router… the internet link shows up here."
                                    : upnp?.state === "mapped" && upnp.cgnat
                                      ? "Only works on your network: your provider uses CGNAT (see Settings → Streaming)."
                                      : "Only works on your network for now (see Settings → Streaming to open the port)."}
                            </p>
                        </>
                    ) : (
                        <p className="text-sm text-(--text-muted)">{server?.error ?? "Starting the link…"}</p>
                    )}
                </section>

                <section className="flex flex-col gap-3">
                    <h2 className="text-sm text-(--text-muted)">Quality</h2>
                    <Segmented
                        value={preset}
                        onChange={(id) => {
                            const chosen = PRESETS.find((p) => p.id === id);
                            if (chosen) set({ height: chosen.height, width: Math.round((chosen.height * 16) / 9), fps: chosen.fps, bitrateKbps: chosen.bitrateKbps });
                            else set({ bitrateKbps: stream.bitrateKbps + 500 }); // sai de um preset para mostrar os campos
                        }}
                        options={[...PRESETS.map((p) => ({ value: p.id, label: p.label })), { value: CUSTOM, label: "Custom" }]}
                    />
                    {preset === CUSTOM && (
                        <div className="flex flex-wrap items-center gap-3">
                            <Select
                                value={String(stream.height)}
                                width="w-28"
                                onChange={(v) => set({ height: Number(v), width: Math.round((Number(v) * 16) / 9) })}
                                options={[480, 720, 1080, 1440].map((h) => ({ value: String(h), label: `${h}p` }))}
                            />
                            <Select value={String(stream.fps)} width="w-28" onChange={(v) => set({ fps: Number(v) })} options={[30, 60].map((f) => ({ value: String(f), label: `${f} FPS` }))} />
                            <NumberInput value={stream.bitrateKbps / 1000} min={0.5} max={50} step={0.5} suffix="Mbps" onChange={(mbps) => set({ bitrateKbps: Math.round(mbps * 1000) })} />
                        </div>
                    )}
                    <div className="flex items-center gap-3 text-sm">
                        <span className="text-(--text-muted)">Max viewers</span>
                        <NumberInput value={stream.maxViewers} min={1} max={100} onChange={(maxViewers) => set({ maxViewers })} />
                    </div>
                    <Note>
                        Needs about <b>{uploadMbps.toFixed(1)} Mbps</b> of upload with {stream.maxViewers} {stream.maxViewers === 1 ? "viewer" : "viewers"} (each one receives the
                        full stream). Viewers see it about half a second to a second behind you.
                    </Note>
                </section>

                <footer className="flex justify-end gap-3">
                    <Button onClick={() => nctx.cancelGoLive()}>Cancel</Button>
                    <Button variant="accent" onClick={() => nctx.confirmGoLive()} disabled={starting}>
                        {starting ? "Starting…" : "Go live"}
                    </Button>
                </footer>
            </div>
        </div>
    );
}
