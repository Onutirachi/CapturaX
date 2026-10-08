import { useContext, useEffect, useRef, useState } from "preact/hooks";
import { CLOSE_DROPDOWNS_EVENT } from "../../core/components/DropDown";
import Toolbar from "./Toolbar";
import Dock from "./Dock";
import StatusBar from "./StatusBar";
import Settings from "../Settings/Settings";
import GoLiveDialog from "../Settings/GoLiveDialog";
import { NativeContext } from "../../context/NativeContext";

const HIDETIME = 2000;

export type HoverHandlers = { onPointerEnter: () => void; onPointerLeave: () => void };

/** Selo discreto no canto enquanto grava/transmite, para não esquecer com as barras escondidas. */
function ActivityBadge({ hidden }: { hidden: boolean }) {
    const nctx = useContext(NativeContext);
    const recording = nctx?.recording === "active";
    const live = nctx?.streaming === "active";
    if (!recording && !live) return null;
    return (
        <div className={`absolute top-3 left-3 flex gap-2 pointer-events-none transition-opacity duration-300 ${hidden ? "opacity-0" : "opacity-90"}`}>
            {recording && (
                <span className="flex items-center gap-1.5 px-2 py-0.5 rounded-md bg-black/50 text-xs font-semibold">
                    <span className="w-2 h-2 rounded-full bg-bad animate-pulse" />
                    REC
                </span>
            )}
            {live && <span className="px-2 py-0.5 rounded-md bg-accent/80 text-xs font-semibold">LIVE</span>}
        </div>
    );
}

export default function Overlay() {
    const [active, setActive] = useState<boolean>(false);
    const hideTimeout = useRef<number | null>(null);

    // A Toolbar escondeu: fecha os menus abertos nela (senão ficariam boiando sobre o vídeo).
    useEffect(() => {
        if (!active) window.dispatchEvent(new Event(CLOSE_DROPDOWNS_EVENT));
    }, [active]);

    // Toolbar e Dock compartilham o mesmo estado: passar o mouse em qualquer um mostra os dois.
    const hover: HoverHandlers = {
        onPointerEnter: () => {
            if (hideTimeout.current !== null) {
                clearTimeout(hideTimeout.current);
                hideTimeout.current = null;
            }
            setActive(true);
        },
        onPointerLeave: () => {
            hideTimeout.current = window.setTimeout(() => setActive(false), HIDETIME);
        },
    };

    return (
        <>
            <ActivityBadge hidden={active} />
            <Toolbar active={active} hover={hover} />
            <Dock active={active} hover={hover} />
            <StatusBar active={active} />
            <Settings />
            <GoLiveDialog />
        </>
    );
}
