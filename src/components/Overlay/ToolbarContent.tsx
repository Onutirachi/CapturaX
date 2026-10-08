import { DropDown, DropOption } from "../../core/components/DropDown";
import Tag from "../../core/components/Tag";
import { RESOLUTION_LIST, FRAMERATE_LIST } from "../../core/utils/constants";
import { useState, useEffect, useContext, useRef } from "preact/hooks";
import Stream, { SCREEN_SOURCE } from "../../core/utils/Stream";
import { VideoStreamContext } from "../../context/VideoStreamContext";
import { AlertsContext } from "../../context/AlertsContext";
import { NativeContext } from "../../context/NativeContext";

const NO_AUDIO = "none";
const DEFAULT_OUTPUT = "default";
const AUTO_PICKED_KEY = "audioInputAutoPicked";

export default function ToolbarContent() {
    const [videoDevicesList, setVideoDevicesList] = useState<MediaDeviceInfo[]>([]);
    const [audioDevicesList, setaudioDevicesList] = useState<MediaDeviceInfo[]>([]);
    const [isScreen, setIsScreen] = useState(false);

    const vsctx = useContext(VideoStreamContext);
    const actx = useContext(AlertsContext);
    const nctx = useContext(NativeContext);

    const { setAlert } = actx ?? {};
    const { setVideoStream, setVideoTrack } = vsctx ?? {};
    const config = nctx?.config ?? null;
    // Até a config carregar, assume o padrão (nativo) para não abrir o áudio pelo WebView à toa.
    const nativeAudio = config?.audio.nativePreview ?? true;

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

    // Preview nativo: o WebView não toca áudio.
    useEffect(() => {
        if (nativeAudio) stream?.clearAudio();
        else stream?.updateAudioTrack();
    }, [nativeAudio, stream]);

    // Primeira vez: escolhe o áudio da mesma placa do vídeo (ex.: "HDMI (ezcap Live GAMER ULTRA)").
    const videoLabel = vsctx?.videoTrack?.label ?? "";
    const inputs = nctx?.devices.inputs ?? [];
    useEffect(() => {
        if (!nctx || !config || !nativeAudio || config.audio.previewInput || localStorage.getItem(AUTO_PICKED_KEY)) return;
        const name = videoLabel.replace(/\s*\([0-9a-f]{4}:[0-9a-f]{4}\)\s*$/i, "").trim().toLowerCase();
        if (!name || inputs.length === 0) return;
        localStorage.setItem(AUTO_PICKED_KEY, "1");
        const match = inputs.find((device) => device.name.toLowerCase().includes(name));
        if (match) nctx.updateConfig((c) => ({ ...c, audio: { ...c.audio, previewInput: match.id } }));
    }, [videoLabel, inputs, config, nativeAudio]);

    if (!vsctx || !actx || !stream) return null;
    if (videoDevicesList.length === 0) return null;

    const setAudio = (patch: Partial<NonNullable<typeof config>["audio"]>) => nctx?.updateConfig((c) => ({ ...c, audio: { ...c.audio, ...patch } }));

    return (
        <>
            <DropDown
                title="Video Device"
                onChoose={async (value) => {
                    setIsScreen(value === SCREEN_SOURCE);
                    stream.setVideoDevice(value);
                }}>
                {videoDevicesList.map(({ deviceId, label }) => (
                    <DropOption key={deviceId} value={deviceId} className="flex gap-3">
                        {label}
                    </DropOption>
                ))}
                <DropOption key={SCREEN_SOURCE} value={SCREEN_SOURCE} className="flex gap-3">
                    Screen / window capture
                </DropOption>
            </DropDown>

            {isScreen && (
                <button className="h-11 px-4 rounded-xl bg-accent hover:brightness-110 transition cursor-pointer" onClick={() => stream.chooseScreen()}>
                    Choose screen
                </button>
            )}

            {nativeAudio && config ? (
                <>
                    <DropDown title="Audio Device" value={config.audio.previewInput ?? NO_AUDIO} onChoose={(value) => setAudio({ previewInput: value === NO_AUDIO ? null : value })}>
                        <DropOption value={NO_AUDIO} className="flex gap-3 text-(--text-muted)">
                            No audio
                        </DropOption>
                        {inputs.map(({ id, name }) => (
                            <DropOption key={id} value={id} className="flex gap-3">
                                {name}
                            </DropOption>
                        ))}
                    </DropDown>

                    <DropDown
                        title="Audio Output"
                        value={config.audio.previewOutput ?? DEFAULT_OUTPUT}
                        onChoose={(value) => setAudio({ previewOutput: value === DEFAULT_OUTPUT ? null : value })}>
                        <DropOption value={DEFAULT_OUTPUT} className="flex gap-3">
                            Windows default
                        </DropOption>
                        {(nctx?.devices.outputs ?? []).map(({ id, name }) => (
                            <DropOption key={id} value={id} className="flex gap-3">
                                {name}
                            </DropOption>
                        ))}
                    </DropDown>
                </>
            ) : (
                <DropDown
                    title="Audio Device"
                    storageKey="Audio Device (browser)"
                    onChoose={async (value) => {
                        stream.audioId = value;
                        if (!nativeAudio) stream.updateAudioTrack();
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
