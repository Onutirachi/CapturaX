import { DropDown, DropOption } from "../../core/components/DropDown";
import Tag from "../../core/components/Tag";
import { RESOLUTION_LIST, FRAMERATE_LIST } from "../../core/utils/constants";
import { useState, useEffect, useContext, useRef } from "preact/hooks";
import Stream from "../../core/utils/Stream";
import { VideoStreamContext } from "../../context/VideoStreamContext";
import { AlertsContext } from "../../context/AlertsContext";

export default function ToolbarContent() {
    const [videoDevicesList, setVideoDevicesList] = useState<MediaDeviceInfo[]>([]);
    const [audioDevicesList, setaudioDevicesList] = useState<MediaDeviceInfo[]>([]);

    const vsctx = useContext(VideoStreamContext);
    const actx = useContext(AlertsContext);

    const { setAlert } = actx ?? {};
    const { setVideoStream, setVideoTrack } = vsctx ?? {};

    const streamRef = useRef<Stream | null>(null);
    if (!streamRef.current && setAlert && setVideoStream && setVideoTrack) {
        streamRef.current = new Stream({ setAlert, setVideoStream, setVideoTrack });
    }
    const stream = streamRef.current;

    useEffect(() => {
        if (!stream) return;
        const loadDevices = async () => {
            const videoDevices = await Stream.getVideoDevices();
            const audioDevices = await Stream.getAudioDevices();
            setVideoDevicesList(videoDevices);
            setaudioDevicesList(audioDevices);
            stream.handleDeviceChange([...videoDevices, ...audioDevices]);
        };

        loadDevices();
        navigator.mediaDevices.addEventListener("devicechange", loadDevices);
        return () => navigator.mediaDevices.removeEventListener("devicechange", loadDevices);
    }, [stream]);

    if (!vsctx || !actx || !stream) return null;
    if (videoDevicesList.length === 0) return null;

    return (
        <>
            <DropDown
                title="Video Device"
                onChoose={async (value) => {
                    stream.setVideoDevice(value);
                }}>
                {videoDevicesList.map(({ deviceId, label }) => (
                    <DropOption key={deviceId} value={deviceId} className="flex gap-3">
                        {label}
                    </DropOption>
                ))}
            </DropDown>

            <DropDown
                title="Audio Device"
                onChoose={async (value) => {
                    stream.audioId = value;
                    stream.updateAudioTrack();
                }}>
                {audioDevicesList.map(({ deviceId, label }) => (
                    <DropOption key={deviceId} value={deviceId} className="flex gap-3">
                        {label}
                    </DropOption>
                ))}
            </DropDown>

            <DropDown
                title="Resolution"
                onChoose={async (value) => {
                    stream.setResolutionTag(value === "Auto" ? null : value);
                }}>
                <DropOption key={"Auto"} value={"Auto"} className="flex gap-3">
                    {`Automatic`}
                    <Tag text="Auto" color="var(--color-accent)" />
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
                    stream.setFrameRate(value === "Auto" ? null : Number(value));
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
