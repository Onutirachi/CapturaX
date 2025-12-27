import { useState, useRef } from "preact/hooks";
import ToolbarContent from "./ToolbarContent";

function StatusBarContent() {
    return <></>;
}

export default function Toolbar() {
    const [active, setActive] = useState(false);
    const hideTimeout = useRef<number | null>(null);

    const HIDETIME = 2000;

    const handlePointerEnter = () => {
        if (hideTimeout.current !== null) {
            clearTimeout(hideTimeout.current);
            hideTimeout.current = null;
        }
        setActive(true);
    };

    const handlePointerLeave = () => {
        hideTimeout.current = window.setTimeout(() => {
            setActive(false);
        }, HIDETIME);
    };
    return (
        <>
            <div className="absolute top-0 left-0 inset-x-0 h-max min-h-25" onPointerEnter={handlePointerEnter} onPointerLeave={handlePointerLeave}>
                <div
                    className={`
                        flex items-center gap-5 flex-wrap
                        p-5
                        h-full w-full
                        [border-bottom:var(--border)]
                        transition-transform duration-300
                        bg-darker/80 backdrop-blur-xs
                    
                        ${active ? "translate-y-0" : "-translate-y-full"}
                    `}>
                    <ToolbarContent />
                </div>
            </div>

            <div className="absolute inset-x-0 h-15 bottom-0">
                <div
                    className={`
                        h-full w-full bg-darker
                        [border-bottom:var(--border)]
                        transition-transform duration-300
                        ${active ? "translate-y-0" : "translate-y-full"}
                    `}>
                    <StatusBarContent />
                </div>
            </div>
        </>
    );
}
