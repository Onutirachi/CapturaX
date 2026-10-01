import { useContext } from "preact/hooks";
import Icon from "./Icon";
import { type HoverHandlers } from "./Overlay";
import { FullscreenContext } from "../../context/FullscreenContext";
import { VideoStreamContext } from "../../context/VideoStreamContext";

type DockProps = { active: boolean; hover: HoverHandlers };

const buttonClass = "w-11 h-11 flex items-center justify-center rounded-xl hover:bg-normal cursor-pointer transition";

function volumeIcon(volume: number, muted: boolean) {
    if (muted || volume === 0) return "./assets/Dock/volume_mute.svg";
    if (volume < 0.5) return "./assets/Dock/volume_low.svg";
    return "./assets/Dock/volume_high.svg";
}

export default function Dock({ active, hover }: DockProps) {
    const fsctx = useContext(FullscreenContext);
    const vsctx = useContext(VideoStreamContext);

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
                    <>
                        <button
                            title={vsctx.ambientLight ? "Disable ambient light" : "Enable ambient light"}
                            onClick={() => vsctx.setAmbientLight(!vsctx.ambientLight)}
                            className={`${buttonClass} ${vsctx.ambientLight ? "bg-normal" : ""}`}>
                            <Icon src="./assets/Dock/ambient_light.svg" color={vsctx.ambientLight ? "#ffaa00" : undefined} />
                        </button>

                        <div className="w-8 [border-top:var(--border)]" />
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
                            className="h-32 w-11 cursor-pointer accent-good [writing-mode:vertical-lr] [direction:rtl]"
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
