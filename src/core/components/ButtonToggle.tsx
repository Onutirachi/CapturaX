import { useState, useEffect } from "preact/hooks";
import { ComponentChildren } from "preact";

type ButtonToggleProps = { children: ComponentChildren; on: () => {} | void; off: () => {} | void; storeState?: string };

export default function ButtonToggle({ children, on, off, storeState }: ButtonToggleProps) {
    const [active, setActive] = useState<boolean>(false);

    const handleClick = () => {
        setActive(!active);
    };

    useEffect(() => {
        if (!storeState) return;
        const store = localStorage.getItem(storeState);
        if (!store) {
            localStorage.setItem(storeState, "false");
        }

        setActive(store === "true" ? true : false);
    }, [storeState]);

    useEffect(() => {
        if (!active) {
            localStorage.setItem(storeState!, "false");
            off();
            return;
        }
        localStorage.setItem(storeState!, "true");
        on();
    }, [active]);

    return <button onClick={handleClick}>{children}</button>;
}
