import { useContext } from "preact/hooks";
import Icon from "./Icon";
import { type HoverHandlers } from "./Overlay";
import { FullscreenContext } from "../../context/FullscreenContext";
import { VideoStreamContext } from "../../context/VideoStreamContext";
import { NativeContext } from "../../context/NativeContext";

type DockProps = { active: boolean; hover: HoverHandlers };

const buttonClass = "w-11 h-11 flex items-center justify-center rounded-xl hover:bg-normal cursor-pointer transition disabled:opacity-50 disabled:cursor-wait";

function volumeIcon(volume: number, muted: boolean) {
    if (muted || volume === 0) return "./assets/Dock/volume_mute.svg";
    if (volume < 0.5) return "./assets/Dock/volume_low.svg";
    return "./assets/Dock/volume_high.svg";
}

const Divider = () => <div className="w-8 [border-top:var(--border)]" />;

export default function Dock({ active, hover }: DockProps) {
    const fsctx = useContext(FullscreenContext);
    const vsctx = useContext(VideoStreamContext);
    const nctx = useContext(NativeContext);

    const recording = nctx?.recording ?? "idle";
    const streaming = nctx?.streaming ?? "idle";

    return (
        // Container só posiciona (não captura o mouse); a dock aparece junto com a Toolbar.
        <div className="absolute right-0 inset-y-0 w-20 flex items-center justify-end pr-3 pointer-events-none">
            <div
                {...hover}
                className={`
                    pointer-events-auto
                    flex flex-col items-center gap-2
                    p-2
                    rounded-2xl [border:var(--border)]
                    bg-darker/80 backdrop-blur-xs
                    transition-transform duration-300
                    ${active ? "translate-x-0" : "translate-x-[calc(100%+1rem)]"}
                `}>
                {fsctx && (
                    <button title="Fullscreen (F11)" onClick={() => fsctx.toggleFullscreen()} className={buttonClass}>
                        <Icon src={fsctx.fullscreen ? "./assets/Dock/fullscreen_exit.svg" : "./assets/Dock/fullscreen.svg"} />
                    </button>
                )}

                {vsctx && (
                    <button
                        title={vsctx.previewHidden ? "Show preview" : "Hide preview (saves resources; recording and streaming keep working)"}
                        onClick={() => vsctx.setPreviewHidden(!vsctx.previewHidden)}
                        className={`${buttonClass} ${vsctx.previewHidden ? "bg-normal" : ""}`}>
                        <Icon src={vsctx.previewHidden ? "./assets/Dock/preview_off.svg" : "./assets/Dock/preview.svg"} />
                    </button>
                )}

                {vsctx && (
                    <button
                        title={vsctx.ambientLight ? "Disable ambient light" : "Enable ambient light"}
                        onClick={() => vsctx.setAmbientLight(!vsctx.ambientLight)}
                        className={`${buttonClass} ${vsctx.ambientLight ? "bg-normal" : ""}`}>
                        <Icon src="./assets/Dock/ambient_light.svg" color={vsctx.ambientLight ? "var(--color-accent)" : undefined} />
                    </button>
                )}

                {nctx && (
                    <>
                        <Divider />
                        <button title="Screenshot (F9)" onClick={() => nctx.screenshot()} className={buttonClass}>
                            <Icon src="./assets/Dock/screenshot.svg" />
                        </button>
                        <button
                            title={recording === "active" ? "Stop recording (F10)" : "Record (F10)"}
                            onClick={() => nctx.toggleRecording()}
                            disabled={recording === "starting" || recording === "stopping"}
                            className={`${buttonClass} ${recording === "active" ? "bg-bad/25" : ""}`}>
                            <Icon
                                src={recording === "active" ? "./assets/Dock/record_stop.svg" : "./assets/Dock/record.svg"}
                                color={recording === "idle" ? undefined : "var(--color-bad)"}
                            />
                        </button>
                        <button
                            title={streaming === "active" ? "Stop streaming" : "Go live (stream by link)"}
                            onClick={() => nctx.toggleStreaming()}
                            disabled={streaming === "starting" || streaming === "stopping"}
                            className={`${buttonClass} ${streaming === "active" ? "bg-accent/25" : ""}`}>
                            <Icon src="./assets/Dock/live.svg" color={streaming === "idle" ? undefined : "var(--color-accent)"} />
                        </button>
                        <button title="Settings" onClick={() => nctx.openSettings()} className={buttonClass}>
                            <Icon src="./assets/Dock/settings.svg" />
                        </button>
                    </>
                )}

                {vsctx && (
                    <>
                        <Divider />
                        <input
                            type="range"
                            title={`Volume: ${Math.round(vsctx.volume * 100)}%`}
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
                        <button title={vsctx.muted ? "Unmute" : "Mute"} onClick={() => vsctx.setMuted(!vsctx.muted)} className={buttonClass}>
                            <Icon src={volumeIcon(vsctx.volume, vsctx.muted)} />
                        </button>
                    </>
                )}
            </div>
        </div>
    );
}
