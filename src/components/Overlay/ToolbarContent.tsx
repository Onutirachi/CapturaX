import { DropDown, DropOption } from "../../core/components/DropDown";
import Tag from "../../core/components/ResolutionTag";
import { RESOLUTION_LIST, FRAMERATE_LIST } from "../../core/utils/constants";
import { useState, useEffect, useContext } from "preact/hooks";
import Button from "./Button";
import Stream from "../../core/utils/stream";
import { MediaStreamContext } from "../../context/MediaStreamContext";

const stream = new Stream();

export default function ToolbarContent() {
    const [deviceList, setDeviceList] = useState<MediaDeviceInfo[]>([]);

    const ctx = useContext(MediaStreamContext);
    if (!ctx) return null;

    const { setMediaStream } = ctx;
    if (!setMediaStream) return null;

    const handleChoose = async () => {
        const mediaStream = await stream.getMediaStream();
        if (!mediaStream) return;
        setMediaStream(mediaStream);
    };

    useEffect(() => {
        const loadDevices = async () => {
            const devices = await stream.getDevices();
            setDeviceList(devices);
        };

        loadDevices();
    }, []);

    useEffect(() => {
        stream.getPermission();
        console.log(deviceList);
    }, [deviceList]);

    return (
        <>
            {deviceList.length > 0 && (
                <DropDown
                    title="Device"
                    onChoose={async (value) => {
                        stream.updateMediaStream({ deviceId: value });
                        handleChoose();
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
                onChoose={async (value) => {
                    stream.updateMediaStream({ resolutionTag: value });
                    handleChoose();
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
                onChoose={async (value) => {
                    stream.updateMediaStream({ frameRate: Number(value) });
                    handleChoose();
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

            <Button>Fullscreen</Button>
        </>
    );
}
