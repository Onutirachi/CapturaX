import { useState, useRef, useEffect } from "preact/hooks";
import { DropDown, DropOption } from "../../core/components/DropDown";
import Tag from "../../core/components/Tag";
import { RESOLUTION_LIST, FRAMERATE_LIST } from "../../core/utils/constants";
import Stream from "../../core/utils/stream";

const stream = new Stream();

function ToolbarContent() {
    const [deviceList, setDeviceList] = useState<MediaDeviceInfo[]>([]);

    useEffect(() => {
        const loadDevices = async () => {
            const devices = await stream.getDevices();
            setDeviceList(devices);
        };

        loadDevices();
    }, []);

    useEffect(() => {
        console.log(deviceList);
    }, [deviceList]);

    return (
        <>
            {deviceList.length > 0 && (
                <DropDown
                    title="Device"
                    onChoose={(value) => {
                        console.log("Device", value);
                    }}>
                    {deviceList.map(({ deviceId, label }) => (
                        <DropOption key={deviceId} value={deviceId} className="flex gap-3">
                            {label}
                        </DropOption>
                    ))}
                </DropDown>
            )}

            <DropDown
                title="Resolution"
                onChoose={(value) => {
                    console.log("Resolution", value);
                }}>
                <DropOption key={"Auto"} value={"Auto"} className="flex gap-3">
                    {`Automatic`}
                    <Tag text="Auto" color="#ffaa00" />
                </DropOption>
                {[...RESOLUTION_LIST].reverse().map(({ width, height, tag, color }, index) => (
                    <DropOption key={index} value={tag} className="flex gap-3">
                        {`${width} x ${height}`}
                        <Tag text={tag} color={color} />
                    </DropOption>
                ))}
            </DropDown>

            <DropDown
                title="Framerate"
                onChoose={(value) => {
                    console.log("Framerate", value);
                }}>
                <DropOption key={"Auto"} value={"Auto"} className="flex gap-3">
                    {`Automatic`}
                </DropOption>
                {[...FRAMERATE_LIST].reverse().map((fps) => (
                    <DropOption key={fps} value={String(fps)} className="flex gap-3">
                        {`${fps} FPS`}
                    </DropOption>
                ))}
            </DropDown>
        </>
    );
}

function StatusBarContent() {
    return <></>;
}

export default function Overlay() {
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
            <div className="absolute inset-x-0 h-max min-h-25" onPointerEnter={handlePointerEnter} onPointerLeave={handlePointerLeave}>
                <div
                    className={`
                        flex items-center gap-5 flex-wrap
                        p-5
                        h-full w-full bg-darker
                        [border-bottom:var(--border)]
                        transition-transform duration-300
                        
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
