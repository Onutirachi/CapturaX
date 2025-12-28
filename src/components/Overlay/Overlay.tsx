import { useState } from "preact/hooks";
import Toolbar from "./Toolbar";
import StatusBar from "./StatusBar";

export default function Overlay() {
    const [active, setActive] = useState<boolean>(false);

    return (
        <>
            <Toolbar active={active} setActive={setActive} />
            <StatusBar active={active} />
        </>
    );
}
