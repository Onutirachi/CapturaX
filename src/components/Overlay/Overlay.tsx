import { useRef, useState } from "preact/hooks";
import Toolbar from "./Toolbar";
import Dock from "./Dock";
import StatusBar from "./StatusBar";

const HIDETIME = 2000;

export type HoverHandlers = { onPointerEnter: () => void; onPointerLeave: () => void };

export default function Overlay() {
    const [active, setActive] = useState<boolean>(false);
    const hideTimeout = useRef<number | null>(null);

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
            <Toolbar active={active} hover={hover} />
            <Dock active={active} hover={hover} />
            <StatusBar active={active} />
        </>
    );
}
