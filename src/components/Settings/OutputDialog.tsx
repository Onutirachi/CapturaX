// Janela "Gravar / Transmitir", em colunas: fonte e áudio (comuns às duas), gravação e
// transmissão, cada uma com o seu checkbox. Embaixo, só Fechar e Começar. O que se escolhe aqui
// (inclusive os checkboxes) fica salvo, mesmo fechando sem começar.
import { type ComponentChildren } from "preact";
import { useContext, useEffect, useState } from "preact/hooks";
import { NativeContext } from "../../context/NativeContext";
import Icon from "../Overlay/Icon";
import { errorText, useT } from "../../i18n";
import { Button, CopyField } from "./controls";
import { OutputAudio, OutputSourcePicker } from "./OutputSource";
import { RecordingQuality } from "./RecordingTab";
import { BitrateControl, StreamQualityPicker, UploadNote } from "./StreamQuality";

const CHOICE_KEY = "outputChoice";
type Choice = { record: boolean; stream: boolean };

function loadChoice(): Choice {
    try {
        const stored = JSON.parse(localStorage.getItem(CHOICE_KEY) ?? "null") as Choice | null;
        if (stored && typeof stored.record === "boolean" && typeof stored.stream === "boolean") return stored;
    } catch {
        // valor corrompido: usa o padrão
    }
    return { record: true, stream: false };
}

function StreamLink() {
    const t = useT();
    const server = useContext(NativeContext)!.server;
    const upnp = server?.upnp;
    if (!server?.localUrl) return <p className="text-sm text-(--text-muted)">{server?.error ? errorText(server.error) : t("golive.starting")}</p>;
    return (
        <div className="flex flex-col gap-2">
            {server.publicUrl && (
                <div className="flex items-center gap-3">
                    <span className="w-20 shrink-0 text-sm">{t("golive.internet")}</span>
                    <CopyField value={server.publicUrl} fullWidth />
                </div>
            )}
            <div className="flex items-center gap-3">
                <span className="w-20 shrink-0 text-sm">{t("golive.localNetwork")}</span>
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
        </div>
    );
}

/** Coluna com título; gravação/transmissão têm checkbox (parado) ou status + Parar (rodando). */
function Column({ title, header, children }: { title?: string; header?: ComponentChildren; children: ComponentChildren }) {
    return (
        <section className="flex flex-col gap-4 p-5 min-w-0">
            <div className="flex items-center gap-3 min-h-9">{header ?? <h2 className="font-semibold">{title}</h2>}</div>
            {children}
        </section>
    );
}

function OutputHeader({ label, checked, onCheck, running, badge, badgeColor, onStop }: {
    label: string;
    checked: boolean;
    onCheck: (value: boolean) => void;
    running: boolean;
    badge: string;
    badgeColor: string;
    onStop: () => void;
}) {
    const t = useT();
    if (running) {
        return (
            <>
                <span className="px-2 py-0.5 rounded-md text-xs font-semibold" style={{ background: badgeColor }}>
                    {badge}
                </span>
                <h2 className="flex-1 font-semibold">{label}</h2>
                <Button variant="danger" onClick={onStop}>
                    {t("output.stop")}
                </Button>
            </>
        );
    }
    return (
        <label className="flex items-center gap-3 cursor-pointer select-none">
            <input type="checkbox" checked={checked} onChange={(e) => onCheck(e.currentTarget.checked)} className="w-5 h-5 accent-accent cursor-pointer" />
            <h2 className="font-semibold">{label}</h2>
        </label>
    );
}

export default function OutputDialog() {
    const t = useT();
    const nctx = useContext(NativeContext);
    const open = nctx?.outputDialogOpen ?? false;
    const [choice, setChoice] = useState<Choice>(loadChoice);
    const choose = (patch: Partial<Choice>) =>
        setChoice((current) => {
            const next = { ...current, ...patch };
            try {
                localStorage.setItem(CHOICE_KEY, JSON.stringify(next));
            } catch {
                // sem armazenamento: só não lembra da próxima vez
            }
            return next;
        });

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
    const { recording, streaming } = nctx;
    const busy = [recording, streaming].some((a) => a === "starting" || a === "stopping");
    const recordingOn = recording !== "idle";
    const live = streaming !== "idle";
    // Fonte e dispositivos de áudio só mudam com tudo parado.
    const running = recordingOn || live;
    const toStart = { record: choice.record && !recordingOn, stream: choice.stream && !live };
    const close = () => nctx.closeOutputDialog();

    return (
        <div className="absolute inset-0 z-20 flex items-center justify-center p-6 bg-black/60 backdrop-blur-xs" onPointerDown={(e) => e.target === e.currentTarget && close()}>
            <div className="flex flex-col w-[min(1180px,100%)] max-h-full rounded-2xl bg-darker [border:var(--border)] shadow-2xl overflow-hidden">
                <header className="flex items-center gap-3 px-6 py-4 [border-bottom:var(--border)]">
                    <div className="w-9 h-9 flex items-center justify-center rounded-lg bg-accent/20">
                        <Icon src="./assets/Dock/live.svg" color="var(--color-accent)" />
                    </div>
                    <h1 className="text-xl font-semibold flex-1">{t("output.title")}</h1>
                    <button title={t("common.closeEsc")} onClick={close} className="w-9 h-9 flex items-center justify-center rounded-lg hover:bg-normal cursor-pointer">
                        <Icon src="./assets/Settings/close.svg" />
                    </button>
                </header>

                <div className="flex-1 min-h-0 overflow-y-auto overflow-x-hidden grid lg:grid-cols-3 divide-y lg:divide-y-0 lg:divide-x divide-(--background-color-normal)">
                    <Column title={t("output.sourceAndAudio")}>
                        <p className="text-sm text-(--text-muted) -mt-2">{t("output.sharedHint")}</p>
                        <OutputSourcePicker disabled={running} />
                        {running && <p className="text-sm text-(--text-muted)">{t("output.sourceLocked")}</p>}
                        <OutputAudio locked={running} />
                    </Column>

                    <Column
                        header={
                            <OutputHeader
                                label={t("settings.recording")}
                                checked={choice.record}
                                onCheck={(record) => choose({ record })}
                                running={recordingOn}
                                badge="REC"
                                badgeColor="var(--color-bad)"
                                onStop={() => nctx.stopRecording()}
                            />
                        }>
                        {recordingOn ? (
                            <p className="text-sm text-(--text-muted)">{t("output.recordingNow")}</p>
                        ) : choice.record ? (
                            <RecordingQuality />
                        ) : (
                            <p className="text-sm text-(--text-muted)">{t("output.recordOff")}</p>
                        )}
                    </Column>

                    <Column
                        header={
                            <OutputHeader
                                label={t("settings.streaming")}
                                checked={choice.stream}
                                onCheck={(stream) => choose({ stream })}
                                running={live}
                                badge={t("status.live")}
                                badgeColor="var(--color-accent)"
                                onStop={() => nctx.stopStreaming()}
                            />
                        }>
                        {live || choice.stream ? (
                            <>
                                <StreamLink />
                                <div className="pt-2">
                                    <StreamQualityPicker fullWidth />
                                </div>
                                <BitrateControl />
                                <p className="text-sm text-(--text-muted)">
                                    <UploadNote />
                                </p>
                                {live && (
                                    <div className="flex items-center gap-3">
                                        <p className="flex-1 text-sm text-(--text-muted)">{t("golive.applyNote")}</p>
                                        <Button variant="accent" disabled={busy} onClick={() => nctx.applyStreamQuality()}>
                                            {t("output.applyQuality")}
                                        </Button>
                                    </div>
                                )}
                            </>
                        ) : (
                            <p className="text-sm text-(--text-muted)">{t("output.streamOff")}</p>
                        )}
                    </Column>
                </div>

                <footer className="flex items-center justify-end gap-3 px-6 py-4 [border-top:var(--border)]">
                    <Button onClick={close}>{t("common.close")}</Button>
                    <Button variant="accent" disabled={busy || (!toStart.record && !toStart.stream)} onClick={() => nctx.startOutputs(toStart)}>
                        {t("output.start")}
                    </Button>
                </footer>
            </div>
        </div>
    );
}
