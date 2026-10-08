import { DropDown, DropOption } from "../../core/components/DropDown";
import Tag from "../../core/components/Tag";
import { RESOLUTION_LIST, FRAMERATE_LIST } from "../../core/utils/constants";
import { useState, useEffect, useContext, useRef } from "preact/hooks";
import Stream, { SCREEN_SOURCE } from "../../core/utils/Stream";
import { VideoStreamContext } from "../../context/VideoStreamContext";
import { AlertsContext } from "../../context/AlertsContext";
import { NativeContext } from "../../context/NativeContext";
import { native } from "../../core/native/api";
import Resolution from "../../core/utils/Resolution";
import { monitorRefreshRate } from "../../core/utils/refreshRate";
import { chooseMode, choicesOf, deviceName, findDevice, formatFps, fpsColor, modeKey, parseModeKey, parseSizeKey, sizeKey, type Size, type VideoDeviceModes } from "../../core/utils/videoModes";

const NO_AUDIO = "none";
const DEFAULT_OUTPUT = "default";
const AUTO_PICKED_KEY = "audioInputAutoPicked";
const AUTO = "Auto";

/** Bandeirinha da resolução: o nome padrão, ou a proporção em cinza. */
const resolutionTag = (size: Size) => Resolution.getTag(size);

export default function ToolbarContent() {
    const [videoDevicesList, setVideoDevicesList] = useState<MediaDeviceInfo[]>([]);
    const [audioDevicesList, setaudioDevicesList] = useState<MediaDeviceInfo[]>([]);
    const [isScreen, setIsScreen] = useState(false);
    /** Modos reais de cada dispositivo (null = ainda carregando). */
    const [deviceModes, setDeviceModes] = useState<VideoDeviceModes[] | null>(null);
    const [videoId, setVideoId] = useState<string | null>(null);
    const [refreshRate, setRefreshRate] = useState(60);

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
            // Modos antes de abrir o dispositivo: a escolha automática depende deles.
            setDeviceModes(await native.videoModes().catch(() => []));
            setVideoDevicesList(videoDevices);
            setaudioDevicesList(audioDevices);
            stream.handleDeviceChange([...videoDevices, ...audioDevices]);
        };

        loadDevices();
        navigator.mediaDevices.addEventListener("devicechange", loadDevices);
        return () => navigator.mediaDevices.removeEventListener("devicechange", loadDevices);
    }, [stream]);

    useEffect(() => {
        monitorRefreshRate().then(setRefreshRate);
    }, []);

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
    if (videoDevicesList.length === 0 || deviceModes === null) return null;

    const modesOf = (id: string | null) => {
        const label = videoDevicesList.find((d) => d.deviceId === id)?.label ?? "";
        return findDevice(deviceModes, label)?.modes ?? null;
    };
    const modes = isScreen ? null : modesOf(videoId);
    const auto = modes ? chooseMode(modes, null, null) : null;

    // Sem modos fixos (tela ou dispositivo desconhecido): listas do app, na tela até o monitor.
    const screenSize = { width: Math.round(window.screen.width * devicePixelRatio), height: Math.round(window.screen.height * devicePixelRatio) };
    const sizes: Size[] = RESOLUTION_LIST.filter((r) => !isScreen || (r.width <= screenSize.width && r.height <= screenSize.height));
    const fpsOptions = FRAMERATE_LIST.filter((f) => !isScreen || f <= refreshRate);

    const setAudio = (patch: Partial<NonNullable<typeof config>["audio"]>) => nctx?.updateConfig((c) => ({ ...c, audio: { ...c.audio, ...patch } }));

    return (
        <>
            <DropDown
                title="Video Device"
                onChoose={async (value) => {
                    setIsScreen(value === SCREEN_SOURCE);
                    setVideoId(value);
                    stream.setVideoDevice(value, modesOf(value));
                }}>
                {videoDevicesList.map(({ deviceId, label }) => (
                    <DropOption key={deviceId} value={deviceId} className="flex gap-3">
                        {deviceName(label)}
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
                            {deviceName(label)}
                        </DropOption>
                    ))}
                </DropDown>
            )}

            {modes ? (
                // Modos reais da placa: resolução e FPS juntos, só o que existe.
                <DropDown
                    title="Mode"
                    storageKey="Video mode"
                    onChoose={async (value) => stream.setMode(value === AUTO ? null : parseModeKey(value))}>
                    <DropOption key={AUTO} value={AUTO} className="flex gap-3">
                        {auto ? `Automatic (${auto.width} x ${auto.height})` : "Automatic"}
                        <Tag text="Auto" color="var(--color-accent)" />
                        {auto && <Tag text={`${formatFps(auto.fps)} FPS`} color={fpsColor(auto.fps)} />}
                    </DropOption>
                    {choicesOf(modes).map((mode) => {
                        const tag = resolutionTag(mode);
                        return (
                            <DropOption key={modeKey(mode)} value={modeKey(mode)} className="flex gap-3">
                                {`${mode.width} x ${mode.height}`}
                                {tag && <Tag text={tag.text} color={tag.color} />}
                                <Tag text={`${formatFps(mode.fps)} FPS`} color={fpsColor(mode.fps)} />
                            </DropOption>
                        );
                    })}
                </DropDown>
            ) : (
                <>
                    <DropDown
                        title="Resolution"
                        storageKey="Resolution (size)"
                        onChoose={async (value) => stream.setResolution(value === AUTO ? null : parseSizeKey(value))}>
                        <DropOption key={AUTO} value={AUTO} className="flex gap-3">
                            Automatic
                            <Tag text="Auto" color="var(--color-accent)" />
                        </DropOption>
                        {sizes.map((size) => {
                            const tag = resolutionTag(size);
                            return (
                                <DropOption key={sizeKey(size)} value={sizeKey(size)} className="flex gap-3">
                                    {`${size.width} x ${size.height}`}
                                    {tag && <Tag text={tag.text} color={tag.color} />}
                                </DropOption>
                            );
                        })}
                    </DropDown>

                    <DropDown
                        title="Framerate"
                        storageKey="Framerate (fps)"
                        onChoose={async (value) => stream.setFrameRate(value === AUTO ? null : Number(value))}>
                        <DropOption key={AUTO} value={AUTO} className="flex gap-3">
                            {isScreen ? `Automatic (${refreshRate} FPS)` : "Automatic"}
                        </DropOption>
                        {fpsOptions.map((fps) => (
                            <DropOption key={fps} value={String(fps)} className="flex gap-3">
                                <Tag text={`${fps} FPS`} color={fpsColor(fps)} />
                            </DropOption>
                        ))}
                    </DropDown>
                </>
            )}
        </>
    );
}
