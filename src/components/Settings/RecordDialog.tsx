// Janela de gravar: fonte (preview, tela ou janela), qualidade e o áudio que vai junto.
import { useContext, useEffect } from "preact/hooks";
import { NativeContext } from "../../context/NativeContext";
import Icon from "../Overlay/Icon";
import { useT } from "../../i18n";
import { Button } from "./controls";
import { AudioSummary, OutputSourcePicker } from "./OutputSource";
import { RecordingQuality } from "./RecordingTab";

export default function RecordDialog() {
    const t = useT();
    const nctx = useContext(NativeContext);
    const open = nctx?.recordDialogOpen ?? false;

    useEffect(() => {
        if (!open) return;
        const onKey = (e: KeyboardEvent) => {
            if (e.key !== "Escape") return;
            e.stopImmediatePropagation();
            nctx?.closeRecordDialog();
        };
        document.addEventListener("keydown", onKey, true);
        return () => document.removeEventListener("keydown", onKey, true);
    }, [open]);

    if (!nctx || !open || !nctx.config) return null;
    const starting = nctx.recording === "starting";

    return (
        <div className="absolute inset-0 z-20 flex items-center justify-center p-6 bg-black/60 backdrop-blur-xs" onPointerDown={(e) => e.target === e.currentTarget && nctx.closeRecordDialog()}>
            <div className="flex flex-col gap-5 w-[min(560px,100%)] max-h-full overflow-y-auto p-6 rounded-2xl bg-darker [border:var(--border)] shadow-2xl">
                <header className="flex items-center gap-3">
                    <div className="w-9 h-9 flex items-center justify-center rounded-lg bg-bad/20">
                        <Icon src="./assets/Dock/record.svg" color="var(--color-bad)" />
                    </div>
                    <h1 className="text-xl font-semibold flex-1">{t("record.title")}</h1>
                    <button title={t("common.closeEsc")} onClick={() => nctx.closeRecordDialog()} className="w-9 h-9 flex items-center justify-center rounded-lg hover:bg-normal cursor-pointer">
                        <Icon src="./assets/Settings/close.svg" />
                    </button>
                </header>

                <section className="flex flex-col gap-2">
                    <h2 className="text-sm text-(--text-muted)">{t("source.title")}</h2>
                    <OutputSourcePicker kind="record" />
                </section>

                <section className="flex flex-col gap-2">
                    <h2 className="text-sm text-(--text-muted)">{t("recording.quality")}</h2>
                    <div className="flex flex-col rounded-xl bg-dark/60 [border:var(--border)] divide-y divide-(--background-color-normal)">
                        <RecordingQuality />
                    </div>
                </section>

                <section className="flex flex-col gap-2">
                    <h2 className="text-sm text-(--text-muted)">{t("record.audio")}</h2>
                    <AudioSummary close={() => nctx.closeRecordDialog()} />
                </section>

                <footer className="flex items-center gap-3">
                    <p className="flex-1 text-sm text-(--text-muted)">{t("record.hotkey")}</p>
                    <Button onClick={() => nctx.closeRecordDialog()}>{t("common.cancel")}</Button>
                    <Button variant="danger" onClick={() => nctx.confirmRecord()} disabled={starting}>
                        {t(starting ? "golive.startingButton" : "record.start")}
                    </Button>
                </footer>
            </div>
        </div>
    );
}
