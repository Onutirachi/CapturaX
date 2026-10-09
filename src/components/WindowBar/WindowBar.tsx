import { getCurrentWindow } from "@tauri-apps/api/window";
import { Button, ButtonContent } from "./Button";
import { useState, useEffect, useContext } from "preact/hooks";
import { FullscreenContext } from "../../context/FullscreenContext";
import { useT } from "../../i18n";

const MINIMIZE_ICON = "./assets/WindowBar/minimize.svg";
const MAXIMIZE_ICON = "./assets/WindowBar/maximize.svg";
const UNMAXIMIZE_ICON = "./assets/WindowBar/unmaximize.svg";
const CLOSE_ICON = "./assets/WindowBar/close.svg";

export default function WindowBar() {
    const t = useT();
    const [maximizeIcon, setMaximizeIcon] = useState<string>(MAXIMIZE_ICON);
    const fsctx = useContext(FullscreenContext);

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

    if (fsctx?.fullscreen) return null;

    return (
        <div className="w-full h-8 bg-darker flex items-center [border-bottom:var(--border)] box-border" data-tauri-drag-region>
            <img src="./assets/Brand/logo.svg" alt="CapturaX" draggable={false} className="h-3.5 ml-3 pointer-events-none select-none" />
            <div className="h-full flex w-max ml-auto items-center gap-1 p-1">
                <Button title={t("window.minimize")} onClick={handleMinimize} hoverColor="var(--background-color-dark)">
                    <ButtonContent icon={MINIMIZE_ICON} color="var(--background-color-light)" hoverColor="var(--color-side)" />
                </Button>
                <Button title={t("window.maximize")} onClick={handleMaximize} hoverColor="var(--background-color-dark)">
                    <ButtonContent icon={maximizeIcon} color="var(--background-color-light)" hoverColor="var(--color-side)" />
                </Button>
                <Button title={t("window.close")} onClick={handleClose} hoverColor="var(--color-bad)">
                    <ButtonContent icon={CLOSE_ICON} color="var(--background-color-light)" hoverColor="var(--color-side)" />
                </Button>
            </div>
        </div>
    );
}
