import { getCurrentWindow } from "@tauri-apps/api/window";
import { Button, ButtonContent } from "./Button";
import { useState, useEffect } from "preact/hooks";

const MINIMIZE_ICON = "./assets/WindowBar/minimize.svg";
const MAXIMIZE_ICON = "./assets/WindowBar/maximize.svg";
const UNMAXIMIZE_ICON = "./assets/WindowBar/unmaximize.svg";
const CLOSE_ICON = "./assets/WindowBar/close.svg";

export default function WindowBar() {
    const [maximizeIcon, setMaximizeIcon] = useState<string>(MAXIMIZE_ICON);

    const updateMaximizeIcon = async () => {
        const isMaximized = await getCurrentWindow().isMaximized();
        if (isMaximized) {
            setMaximizeIcon(UNMAXIMIZE_ICON);
        } else {
            setMaximizeIcon(MAXIMIZE_ICON);
        }
    };

    const handleMinimize = async () => {
        await getCurrentWindow().minimize();
    };

    const handleMaximize = async () => {
        const isMaximized = await getCurrentWindow().isMaximized();

        if (isMaximized) {
            await getCurrentWindow().unmaximize();
            return;
        }

        await getCurrentWindow().maximize();
    };

    const handleClose = async () => {
        await getCurrentWindow().close();
    };

    useEffect(() => {
        updateMaximizeIcon();

        window.addEventListener("resize", updateMaximizeIcon);

        return () => {
            window.removeEventListener("resize", updateMaximizeIcon);
        };
    }, []);

    return (
        <div className="w-full h-8 bg-darker flex items-center [border-bottom:var(--border)] box-border" data-tauri-drag-region>
            <div className="h-full flex w-max ml-auto items-center gap-1 p-1">
                <Button title="Minimizar" onClick={handleMinimize} hoverColor="var(--background-color-dark)">
                    <ButtonContent icon={MINIMIZE_ICON} color="var(--background-color-light)" hoverColor="var(--color-side)" />
                </Button>
                <Button title="Maximizar" onClick={handleMaximize} hoverColor="var(--background-color-dark)">
                    <ButtonContent icon={maximizeIcon} color="var(--background-color-light)" hoverColor="var(--color-side)" />
                </Button>
                <Button title="Fechar" onClick={handleClose} hoverColor="var(--color-bad)">
                    <ButtonContent icon={CLOSE_ICON} color="var(--background-color-light)" hoverColor="var(--color-side)" />
                </Button>
            </div>
        </div>
    );
}
