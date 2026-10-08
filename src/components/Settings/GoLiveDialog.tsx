// Janela que aparece ao clicar em transmitir: qualidade na hora e o link para copiar antes de começar.
import { useContext, useEffect } from "preact/hooks";
import { NativeContext } from "../../context/NativeContext";
import Icon from "../Overlay/Icon";
import { Button, CopyField, Note } from "./controls";
import { BitrateControl, FpsSelect, ResolutionSelect, UploadNote } from "./StreamQuality";

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
                    <div className="flex flex-wrap items-center gap-3">
                        <ResolutionSelect />
                        <FpsSelect />
                    </div>
                    <div className="flex items-center gap-3 text-sm">
                        <span className="text-(--text-muted)">Bitrate</span>
                        <BitrateControl />
                    </div>
                    <Note>
                        <UploadNote />
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
