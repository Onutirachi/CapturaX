import { DropDown, DropOption } from "../../core/components/DropDown";
import Tag from "../../core/components/Tag";
import { RESOLUTION_LIST, FRAMERATE_LIST } from "../../core/utils/constants";
import { useState, useEffect, useContext } from "preact/hooks";
import Button from "./Button";
import Stream from "../../core/utils/Stream";
import { MediaStreamContext } from "../../context/MediaStreamContext";

const stream = new Stream();

export default function ToolbarContent() {
    const [videoDevicesList, setVideoDevicesList] = useState<MediaDeviceInfo[]>([]);
    const [audioDevicesList, setaudioDevicesList] = useState<MediaDeviceInfo[]>([]);

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
            const videoDevicesList = await stream.getVideoDevices();
            const audioDevices = await stream.getAudioDevices();
            setVideoDevicesList(videoDevicesList);
            setaudioDevicesList(audioDevices);
        };

        loadDevices();
    }, []);

    useEffect(() => {
        // stream.getPermission();
    }, []);

    return (
        <>
            {videoDevicesList.length > 0 && (
                <DropDown
                    title="Video Device"
                    onChoose={async (value) => {
                        stream.updateMediaStream({ videoDeviceId: value });
                        handleChoose();
                    }}>
                    {videoDevicesList.map(({ deviceId, label }) => (
                        <DropOption key={deviceId} value={deviceId} className="flex gap-3">
                            {label}
                        </DropOption>
                    ))}
                </DropDown>
            )}

            {audioDevicesList.length > 0 && (
                <DropDown
                    title="Audio Device"
                    onChoose={async (value) => {
                        stream.updateMediaStream({ audioDeviceId: value });
                        handleChoose();
                    }}>
                    {audioDevicesList.map(({ deviceId, label }) => (
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
