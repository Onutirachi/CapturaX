import { useState } from "preact/hooks";
import StatusBarContent from "./StatusBarContent";

export default function StatusBar({ active }: { active: boolean }) {
    const [fixed, setFixed] = useState<boolean>(false);

    return (
        <div className="absolute inset-x-0 h-15 bottom-0">
            <div
                className={`
                        h-full w-full
                        transition-transform duration-300
                        flex gap-3 items-center justify-end
                        p-3
                        ${active || fixed ? "translate-y-0" : "translate-y-full"}
                    `}>
                <StatusBarContent fixed={fixed} setFixed={setFixed} />
            </div>
        </div>
    );
}
