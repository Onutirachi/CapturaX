// Janela da transmissão: antes de começar (qualidade + link para copiar) e ao vivo (mudar a
// qualidade ou parar). Também reabre sozinha quando a fonte muda durante a transmissão.
import { useContext, useEffect } from "preact/hooks";
import { NativeContext } from "../../context/NativeContext";
import Icon from "../Overlay/Icon";
import { Button, CopyField, Note } from "./controls";
import { BitrateControl, StreamQualityPicker, UploadNote } from "./StreamQuality";

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
    const server = nctx.server;
    const upnp = server?.upnp;
    const starting = nctx.streaming === "starting";
    const live = nctx.streaming === "active";

    return (
        <div className="absolute inset-0 z-20 flex items-center justify-center p-6 bg-black/60 backdrop-blur-xs" onPointerDown={(e) => e.target === e.currentTarget && nctx.cancelGoLive()}>
            <div className="flex flex-col gap-5 w-[min(560px,100%)] p-6 rounded-2xl bg-darker [border:var(--border)] shadow-2xl">
                <header className="flex items-center gap-3">
                    <div className="w-9 h-9 flex items-center justify-center rounded-lg bg-accent/20">
                        <Icon src="./assets/Dock/live.svg" color="var(--color-accent)" />
                    </div>
                    <h1 className="text-xl font-semibold flex-1">{live ? "Stream settings" : "Go live"}</h1>
                    <button title="Close (Esc)" onClick={() => nctx.cancelGoLive()} className="w-9 h-9 flex items-center justify-center rounded-lg hover:bg-normal cursor-pointer">
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

                <section className="flex flex-col gap-4">
                    <h2 className="text-sm text-(--text-muted)">Quality</h2>
                    <StreamQualityPicker />
                    <div className="flex items-center gap-4">
                        <span className="text-sm text-(--text-muted) w-14">Bitrate</span>
                        <BitrateControl />
                    </div>
                    <Note>
                        <UploadNote />
                        {live && " Applying restarts the stream; viewers reconnect by themselves in a second or two."}
                    </Note>
                </section>

                <footer className="flex items-center gap-3">
                    {live && (
                        <Button variant="danger" onClick={() => nctx.stopStreaming()}>
                            Stop streaming
                        </Button>
                    )}
                    <div className="flex-1" />
                    <Button onClick={() => nctx.cancelGoLive()}>{live ? "Close" : "Cancel"}</Button>
                    <Button variant="accent" onClick={() => nctx.confirmGoLive()} disabled={starting}>
                        {starting ? "Starting…" : live ? "Apply" : "Go live"}
                    </Button>
                </footer>
            </div>
        </div>
    );
}
