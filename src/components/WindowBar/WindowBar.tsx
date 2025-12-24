import { getCurrentWindow } from "@tauri-apps/api/window";
import Button from "./Button";
import { useState, useEffect } from "preact/hooks";

function ButtonContent({ icon }: { icon: string }) {
    const style = `w-full h-[80%] flex items-center justify-center bg-amber-100 mask-contain mask-center mask-no-repeat`;

    return <div className={style} style={{ maskImage: `url(${icon})` }}></div>;
}

export default function WindowBar() {
    const [maximizeIcon, setMaximizeIcon] = useState<string>("./assets/WindowBar/maximize.svg");

    const updateMaximizeIcon = async () => {
        const isMaximized = await getCurrentWindow().isMaximized();
        if (isMaximized) {
            setMaximizeIcon("./assets/WindowBar/unmaximize.svg");
        } else {
            setMaximizeIcon("./assets/WindowBar/maximize.svg");
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
        <div className="w-full h-7 bg-darker flex" data-tauri-drag-region>
            <div className="h-full flex w-max ml-auto">
                <Button title="Minimizar" onClick={handleMinimize}>
                    <ButtonContent icon={"/assets/WindowBar/minimize.svg"} />
                </Button>
                <Button title="Maximizar" onClick={handleMaximize}>
                    <ButtonContent icon={maximizeIcon} />
                </Button>
                <Button title="Fechar" onClick={handleClose}>
                    <ButtonContent icon={"/assets/WindowBar/close.svg"} />
                </Button>
            </div>
        </div>
    );
}
