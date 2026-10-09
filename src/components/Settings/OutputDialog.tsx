// Janela "Gravar / Transmitir": uma fonte e um áudio para as duas saídas, a qualidade de cada uma,
// o link da transmissão e os botões Gravar / Gravar e transmitir / Transmitir. O que se escolhe
// aqui é salvo na hora, mesmo fechando sem começar. Reabre sozinha (travada) quando a fonte do
// preview muda ao vivo.
import { type ComponentChildren } from "preact";
import { useContext, useEffect } from "preact/hooks";
import { NativeContext } from "../../context/NativeContext";
import Icon from "../Overlay/Icon";
import { errorText, useT } from "../../i18n";
import { Button, CopyField, Note } from "./controls";
import { OutputAudio, OutputSourcePicker } from "./OutputSource";
import { RecordingQuality } from "./RecordingTab";
import { BitrateControl, StreamQualityPicker, UploadNote } from "./StreamQuality";

function Block({ title, aside, children }: { title: string; aside?: ComponentChildren; children: ComponentChildren }) {
    return (
        <section className="flex flex-col gap-2">
            <div className="flex items-center gap-2">
                <h2 className="flex-1 text-sm text-(--text-muted)">{title}</h2>
                {aside}
            </div>
            {children}
        </section>
    );
}

function StreamLink() {
    const t = useT();
    const server = useContext(NativeContext)!.server;
    const upnp = server?.upnp;
    if (!server?.localUrl) return <p className="text-sm text-(--text-muted)">{server?.error ? errorText(server.error) : t("golive.starting")}</p>;
    return (
        <>
            {server.publicUrl && (
                <div className="flex items-center gap-3">
                    <span className="w-24 shrink-0 text-sm">{t("golive.internet")}</span>
                    <CopyField value={server.publicUrl} fullWidth />
                </div>
            )}
            <div className="flex items-center gap-3">
                <span className="w-24 shrink-0 text-sm">{t("golive.localNetwork")}</span>
                <CopyField value={server.localUrl} fullWidth />
            </div>
            <p className="text-sm text-(--text-muted)">
                {server.cgnat
                    ? t("golive.cgnat")
                    : upnp?.state === "mapped"
                      ? t("golive.mapped")
                      : upnp?.state === "searching"
                        ? t("golive.searching")
                        : t("golive.needsPort", { port: server.port })}
            </p>
        </>
    );
}

const Running = ({ text, color }: { text: string; color: string }) => (
    <span className="px-2 py-0.5 rounded-md text-xs font-semibold" style={{ background: color }}>
        {text}
    </span>
);

export default function OutputDialog() {
    const t = useT();
    const nctx = useContext(NativeContext);
    const open = nctx?.outputDialogOpen ?? false;

    useEffect(() => {
        if (!open) return;
        const onKey = (e: KeyboardEvent) => {
            if (e.key !== "Escape") return;
            e.stopImmediatePropagation();
            nctx?.closeOutputDialog();
        };
        document.addEventListener("keydown", onKey, true);
        return () => document.removeEventListener("keydown", onKey, true);
    }, [open, nctx?.closeOutputDialog]);

    if (!nctx || !open || !nctx.config) return null;
    const recording = nctx.recording;
    const streaming = nctx.streaming;
    const busy = recording === "starting" || recording === "stopping" || streaming === "starting" || streaming === "stopping";
    const recordingOn = recording === "active";
    const live = streaming === "active";
    // Fonte e dispositivos de áudio só mudam com tudo parado.
    const running = recording !== "idle" || streaming !== "idle";
    const locked = nctx.mustReconfigure;
    const close = () => nctx.closeOutputDialog();

    return (
        <div className="absolute inset-0 z-20 flex items-center justify-center p-6 bg-black/60 backdrop-blur-xs" onPointerDown={(e) => e.target === e.currentTarget && close()}>
            <div className="flex flex-col gap-5 w-[min(600px,100%)] max-h-full overflow-y-auto p-6 rounded-2xl bg-darker [border:var(--border)] shadow-2xl">
                <header className="flex items-center gap-3">
                    <div className="w-9 h-9 flex items-center justify-center rounded-lg bg-accent/20">
                        <Icon src="./assets/Dock/live.svg" color="var(--color-accent)" />
                    </div>
                    <h1 className="text-xl font-semibold flex-1">{t("output.title")}</h1>
                    {!locked && (
                        <button title={t("common.closeEsc")} onClick={close} className="w-9 h-9 flex items-center justify-center rounded-lg hover:bg-normal cursor-pointer">
                            <Icon src="./assets/Settings/close.svg" />
                        </button>
                    )}
                </header>

                {locked && <Note tone="warning">{t("golive.sourceChanged")}</Note>}

                <Block title={t("source.title")}>
                    <OutputSourcePicker disabled={running} />
                    {running && <p className="text-sm text-(--text-muted)">{t("output.sourceLocked")}</p>}
                </Block>

                <Block title={t("record.audio")}>
                    <OutputAudio locked={running} close={close} />
                </Block>

                <Block
                    title={t("settings.recording")}
                    aside={
                        recordingOn && (
                            <>
                                <Running text="REC" color="var(--color-bad)" />
                                <Button variant="danger" onClick={() => nctx.stopRecording()}>
                                    {t("output.stopRecording")}
                                </Button>
                            </>
                        )
                    }>
                    {!recordingOn && (
                        <div className="flex flex-col rounded-xl bg-dark/60 [border:var(--border)] divide-y divide-(--background-color-normal)">
                            <RecordingQuality />
                        </div>
                    )}
                </Block>

                <Block
                    title={t("settings.streaming")}
                    aside={
                        live && (
                            <>
                                <Running text={t("status.live")} color="var(--color-accent)" />
                                <Button variant="danger" onClick={() => nctx.stopStreaming()}>
                                    {t("golive.stop")}
                                </Button>
                            </>
                        )
                    }>
                    <StreamLink />
                    <StreamQualityPicker fullWidth />
                    <BitrateControl />
                    <Note>
                        <UploadNote />
                        {live && ` ${t("golive.applyNote")}`}
                    </Note>
                    {live && (
                        <div className="flex justify-end">
                            <Button variant="accent" disabled={busy} onClick={() => nctx.applyStreamQuality()}>
                                {t("output.applyQuality")}
                            </Button>
                        </div>
                    )}
                </Block>

                {!locked && (
                    <footer className="flex flex-wrap items-center justify-end gap-3">
                        <Button onClick={close}>{t("common.close")}</Button>
                        {!recordingOn && !live && (
                            <>
                                <Button variant="danger" disabled={busy} onClick={() => nctx.startOutputs({ record: true, stream: false })}>
                                    {t("output.record")}
                                </Button>
                                <Button variant="accent" disabled={busy} onClick={() => nctx.startOutputs({ record: true, stream: true })}>
                                    {t("output.recordAndStream")}
                                </Button>
                                <Button variant="accent" disabled={busy} onClick={() => nctx.startOutputs({ record: false, stream: true })}>
                                    {t("output.stream")}
                                </Button>
                            </>
                        )}
                        {live && !recordingOn && (
                            <Button variant="danger" disabled={busy} onClick={() => nctx.startOutputs({ record: true, stream: false })}>
                                {t("output.recordToo")}
                            </Button>
                        )}
                        {recordingOn && !live && (
                            <Button variant="accent" disabled={busy} onClick={() => nctx.startOutputs({ record: false, stream: true })}>
                                {t("output.streamToo")}
                            </Button>
                        )}
                    </footer>
                )}
            </div>
        </div>
    );
}
