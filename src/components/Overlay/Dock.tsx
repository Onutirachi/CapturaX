import { type ComponentChildren } from "preact";
import { useContext } from "preact/hooks";
import Icon from "./Icon";
import { type HoverHandlers } from "./Overlay";
import { FullscreenContext } from "../../context/FullscreenContext";
import { VideoStreamContext } from "../../context/VideoStreamContext";
import { NativeContext } from "../../context/NativeContext";
import { useT } from "../../i18n";

type DockProps = { active: boolean; hover: HoverHandlers };

const buttonClass = "w-11 h-11 flex items-center justify-center rounded-xl hover:bg-normal cursor-pointer transition disabled:opacity-50 disabled:cursor-wait";

function volumeIcon(volume: number, muted: boolean) {
    if (muted || volume === 0) return "./assets/Dock/volume_mute.svg";
    if (volume < 0.5) return "./assets/Dock/volume_low.svg";
    return "./assets/Dock/volume_high.svg";
}

const Divider = () => <div className="w-8 [border-top:var(--border)]" />;

/** Painel vertical que entra pela lateral junto com a Toolbar. */
function DockPanel({ side, active, hover, children }: DockProps & { side: "left" | "right"; children: ComponentChildren }) {
    const hidden = side === "right" ? "translate-x-[calc(100%+1rem)]" : "-translate-x-[calc(100%+1rem)]";
    return (
        // Container só posiciona (não captura o mouse).
        <div className={`absolute inset-y-0 w-20 flex items-center pointer-events-none ${side === "right" ? "right-0 justify-end pr-3" : "left-0 justify-start pl-3"}`}>
            <div
                {...hover}
                className={`
                    pointer-events-auto
                    flex flex-col items-center gap-2
                    p-2
                    rounded-2xl [border:var(--border)]
                    bg-darker/80 backdrop-blur-xs
                    transition-transform duration-300
                    ${active ? "translate-x-0" : hidden}
                `}>
                {children}
            </div>
        </div>
    );
}

/** Esquerda: como você vê e ouve (tela cheia, luz ambiente, volume). */
export function ViewDock(props: DockProps) {
    const t = useT();
    const fsctx = useContext(FullscreenContext);
    const vsctx = useContext(VideoStreamContext);
    if (!vsctx) return null;

    return (
        <DockPanel side="left" {...props}>
            {fsctx && (
                <button title={t("dock.fullscreen")} onClick={() => fsctx.toggleFullscreen()} className={buttonClass}>
                    <Icon src={fsctx.fullscreen ? "./assets/Dock/fullscreen_exit.svg" : "./assets/Dock/fullscreen.svg"} />
                </button>
            )}
            <button
                title={t(vsctx.ambientLight ? "dock.ambientOff" : "dock.ambientOn")}
                onClick={() => vsctx.setAmbientLight(!vsctx.ambientLight)}
                className={`${buttonClass} ${vsctx.ambientLight ? "bg-normal" : ""}`}>
                <Icon src="./assets/Dock/ambient_light.svg" color={vsctx.ambientLight ? "var(--color-accent)" : undefined} />
            </button>
            <Divider />
            <input
                type="range"
                title={t("dock.volume", { percent: Math.round(vsctx.volume * 100) })}
                min={0}
                max={1}
                step={0.01}
                value={vsctx.muted ? 0 : vsctx.volume}
                onInput={(e) => {
                    vsctx.setVolume(Number(e.currentTarget.value));
                    vsctx.setMuted(false);
                }}
                className="h-24 w-11 cursor-pointer accent-accent [writing-mode:vertical-lr] [direction:rtl]"
            />
            <button title={t(vsctx.muted ? "dock.unmute" : "dock.mute")} onClick={() => vsctx.setMuted(!vsctx.muted)} className={buttonClass}>
                <Icon src={volumeIcon(vsctx.volume, vsctx.muted)} />
            </button>
        </DockPanel>
    );
}

/** Direita: o que você faz com a captura (preview, print, gravar, transmitir, configurações). */
export function ActionDock(props: DockProps) {
    const t = useT();
    const vsctx = useContext(VideoStreamContext);
    const nctx = useContext(NativeContext);
    if (!vsctx || !nctx) return null;

    const recording = nctx.recording;
    const streaming = nctx.streaming;

    return (
        <DockPanel side="right" {...props}>
            <button
                title={t(vsctx.previewHidden ? "dock.showPreview" : "dock.hidePreview")}
                onClick={() => vsctx.setPreviewHidden(!vsctx.previewHidden)}
                className={`${buttonClass} ${vsctx.previewHidden ? "bg-normal" : ""}`}>
                <Icon src={vsctx.previewHidden ? "./assets/Dock/preview_off.svg" : "./assets/Dock/preview.svg"} />
            </button>
            <button title={t("dock.screenshot")} onClick={() => nctx.screenshot()} className={buttonClass}>
                <Icon src="./assets/Dock/screenshot.svg" />
            </button>
            <button
                title={t(recording === "active" ? "dock.stopRecording" : "dock.record")}
                onClick={() => (recording === "active" ? nctx.toggleRecording() : nctx.openRecordDialog())}
                disabled={recording === "starting" || recording === "stopping"}
                className={`${buttonClass} ${recording === "active" ? "bg-bad/25" : ""}`}>
                <Icon src={recording === "active" ? "./assets/Dock/record_stop.svg" : "./assets/Dock/record.svg"} color={recording === "idle" ? undefined : "var(--color-bad)"} />
            </button>
            <button
                title={t(streaming === "active" ? "dock.streamSettings" : "dock.goLive")}
                onClick={() => nctx.toggleStreaming()}
                disabled={streaming === "starting" || streaming === "stopping"}
                className={`${buttonClass} ${streaming === "active" ? "bg-accent/25" : ""}`}>
                <Icon src="./assets/Dock/live.svg" color={streaming === "idle" ? undefined : "var(--color-accent)"} />
            </button>
            <Divider />
            <button title={t("dock.settings")} onClick={() => nctx.openSettings()} className={buttonClass}>
                <Icon src="./assets/Dock/settings.svg" />
            </button>
        </DockPanel>
    );
}
