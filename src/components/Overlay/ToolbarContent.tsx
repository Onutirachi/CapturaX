import { DropDown, DropHeader, DropOption } from "../../core/components/DropDown";
import Tag from "../../core/components/Tag";
import { RESOLUTION_LIST, FRAMERATE_LIST } from "../../core/utils/constants";
import { useState, useEffect, useContext, useRef } from "preact/hooks";
import Stream, { SCREEN_SOURCE, isCaptureSource } from "../../core/utils/Stream";
import { nativeCaptureAvailable } from "../../core/native/screenCapture";
import { VideoStreamContext } from "../../context/VideoStreamContext";
import { AlertsContext } from "../../context/AlertsContext";
import { NativeContext } from "../../context/NativeContext";
import { native, type CaptureSources } from "../../core/native/api";
import Resolution from "../../core/utils/Resolution";
import { monitorRefreshRate } from "../../core/utils/refreshRate";
import { chooseMode, choicesOf, deviceName, findDevice, pairedAudio, formatFps, fpsColor, modeKey, parseModeKey, parseSizeKey, sizeKey, type Size, type VideoDeviceModes } from "../../core/utils/videoModes";

const NO_AUDIO = "none";
const DEFAULT_OUTPUT = "default";
const AUTO_AUDIO = "auto";
const AUTO = "Auto";

/** "Screen 2 - 27G2G4": o número que o Windows dá à tela + o nome do monitor. */
const screenLabel = (screen: CaptureSources["screens"][number]) => `Screen ${screen.number}${screen.name ? ` - ${screen.name}` : ""}`;

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
    const [captureSources, setCaptureSources] = useState<CaptureSources>({ screens: [], windows: [] });
    const nativeCapture = nativeCaptureAvailable();

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

    // Telas e janelas: ao abrir o app e sempre que o menu de vídeo abre (janelas vêm e vão).
    const loadCaptureSources = () => {
        if (nativeCapture) native.captureSources().then(setCaptureSources).catch(() => {});
    };
    useEffect(loadCaptureSources, []);

    // Preview nativo: o WebView não toca áudio.
    useEffect(() => {
        if (nativeAudio) stream?.clearAudio();
        else stream?.updateAudioTrack();
    }, [nativeAudio, stream]);

    // Áudio automático: o do mesmo aparelho do vídeo atual (tela/janela ou aparelho sem áudio = nenhum).
    const videoTrack = vsctx?.videoTrack ?? null;
    const inputs = nctx?.devices.inputs ?? [];
    const paired = videoTrack && deviceModes ? pairedAudio(findDevice(deviceModes, videoTrack.label), inputs) : null;
    useEffect(() => {
        // Sem trilha (trocando de fonte): mantém o anterior em vez de cortar o som por um instante.
        if (videoTrack) native.audioSetPairedInput(paired?.id ?? null).catch(() => {});
    }, [videoTrack, paired?.id]);

    if (!vsctx || !actx || !stream) return null;
    if ((videoDevicesList.length === 0 && !nativeCapture) || deviceModes === null) return null;

    const modesOf = (id: string | null) => {
        const label = videoDevicesList.find((d) => d.deviceId === id)?.label ?? "";
        return findDevice(deviceModes, label)?.modes ?? null;
    };
    const capturing = videoId !== null && isCaptureSource(videoId);
    // FPS do automático na captura: a taxa da tela (janela: a maior entre as telas).
    const captureRefresh = (id: string) =>
        captureSources.screens.find((s) => s.id === id)?.refreshRate ?? Math.max(60, ...captureSources.screens.map((s) => s.refreshRate));
    const autoFps = capturing ? captureRefresh(videoId!) : isScreen ? refreshRate : null;
    const modes = isScreen || capturing ? null : modesOf(videoId);
    const auto = modes ? chooseMode(modes, null, null) : null;

    // Sem modos fixos (tela ou dispositivo desconhecido): listas do app, na tela até o monitor.
    const screenSize = { width: Math.round(window.screen.width * devicePixelRatio), height: Math.round(window.screen.height * devicePixelRatio) };
    const sizes: Size[] = RESOLUTION_LIST.filter((r) => !isScreen || (r.width <= screenSize.width && r.height <= screenSize.height));
    const fpsOptions = FRAMERATE_LIST.filter((f) => autoFps === null || f <= autoFps);

    const setAudio = (patch: Partial<NonNullable<typeof config>["audio"]>) => nctx?.updateConfig((c) => ({ ...c, audio: { ...c.audio, ...patch } }));

    return (
        <>
            <DropDown
                title="Video Device"
                onOpen={loadCaptureSources}
                onChoose={async (value) => {
                    setIsScreen(value === SCREEN_SOURCE);
                    setVideoId(value);
                    if (isCaptureSource(value)) {
                        vsctx.setDeviceModes(null);
                        stream.setCaptureSource(value, captureRefresh(value));
                        return;
                    }
                    const deviceModes = value === SCREEN_SOURCE ? null : modesOf(value);
                    vsctx.setDeviceModes(deviceModes);
                    stream.setVideoDevice(value, deviceModes);
                }}>
                {nativeCapture && videoDevicesList.length > 0 && <DropHeader>Devices</DropHeader>}
                {videoDevicesList.map(({ deviceId, label }) => (
                    <DropOption key={deviceId} value={deviceId}>
                        {deviceName(label)}
                    </DropOption>
                ))}
                {nativeCapture && captureSources.screens.length > 0 && <DropHeader>Screens</DropHeader>}
                {captureSources.screens.map((screen) => {
                    const tag = resolutionTag(screen);
                    return (
                        <DropOption key={screen.id} value={screen.id}>
                            {screenLabel(screen)}
                            <Tag text={tag.text} color={tag.color} />
                            <Tag text={`${screen.refreshRate} FPS`} color={fpsColor(screen.refreshRate)} />
                        </DropOption>
                    );
                })}
                {nativeCapture && captureSources.windows.length > 0 && <DropHeader>Windows</DropHeader>}
                {captureSources.windows.map((window) => (
                    <DropOption key={window.id} value={window.id}>
                        {window.title}
                        {window.app && <Tag text={window.app} />}
                    </DropOption>
                ))}
                {!nativeCapture && (
                    <DropOption key={SCREEN_SOURCE} value={SCREEN_SOURCE}>
                        Screen / window capture
                    </DropOption>
                )}
            </DropDown>

            {isScreen && (
                <button className="h-11 px-4 rounded-xl bg-accent hover:brightness-110 transition cursor-pointer" onClick={() => stream.chooseScreen()}>
                    Choose screen
                </button>
            )}

            {nativeAudio && config ? (
                <>
                    <DropDown
                        title="Audio Device"
                        value={config.audio.previewInputAuto ? AUTO_AUDIO : (config.audio.previewInput ?? NO_AUDIO)}
                        onChoose={(value) =>
                            setAudio(value === AUTO_AUDIO ? { previewInputAuto: true } : { previewInputAuto: false, previewInput: value === NO_AUDIO ? null : value })
                        }>
                        <DropOption value={AUTO_AUDIO}>{`Auto (${paired ? paired.name : "no audio"})`}</DropOption>
                        <DropOption value={NO_AUDIO} className="text-(--text-muted)">
                            No audio
                        </DropOption>
                        {inputs.map(({ id, name }) => (
                            <DropOption key={id} value={id}>
                                {name}
                            </DropOption>
                        ))}
                    </DropDown>

                    <DropDown
                        title="Audio Output"
                        value={config.audio.previewOutput ?? DEFAULT_OUTPUT}
                        onChoose={(value) => setAudio({ previewOutput: value === DEFAULT_OUTPUT ? null : value })}>
                        <DropOption value={DEFAULT_OUTPUT}>
                            Windows default
                        </DropOption>
                        {(nctx?.devices.outputs ?? []).map(({ id, name }) => (
                            <DropOption key={id} value={id}>
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
                        <DropOption key={deviceId} value={deviceId}>
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
                    <DropOption key={AUTO} value={AUTO}>
                        {auto ? `Auto (${auto.width} x ${auto.height})` : "Auto"}
                        {auto && <Tag text={resolutionTag(auto).text} color={resolutionTag(auto).color} />}
                        {auto && <Tag text={`${formatFps(auto.fps)} FPS`} color={fpsColor(auto.fps)} />}
                    </DropOption>
                    {choicesOf(modes).map((mode) => {
                        const tag = resolutionTag(mode);
                        return (
                            <DropOption key={modeKey(mode)} value={modeKey(mode)}>
                                {`${mode.width} x ${mode.height}`}
                                <Tag text={tag.text} color={tag.color} />
                                <Tag text={`${formatFps(mode.fps)} FPS`} color={fpsColor(mode.fps)} />
                            </DropOption>
                        );
                    })}
                </DropDown>
            ) : (
                <>
                    {/* Tela/janela pelo Rust: sempre no tamanho real; só o FPS é escolhido. */}
                    {!capturing && (
                        <DropDown
                            title="Resolution"
                            storageKey="Resolution (size)"
                            onChoose={async (value) => stream.setResolution(value === AUTO ? null : parseSizeKey(value))}>
                            <DropOption key={AUTO} value={AUTO}>
                                {isScreen ? `Auto (${screenSize.width} x ${screenSize.height})` : "Auto"}
                                {isScreen && <Tag text={resolutionTag(screenSize).text} color={resolutionTag(screenSize).color} />}
                            </DropOption>
                            {sizes.map((size) => {
                                const tag = resolutionTag(size);
                                return (
                                    <DropOption key={sizeKey(size)} value={sizeKey(size)}>
                                        {`${size.width} x ${size.height}`}
                                        <Tag text={tag.text} color={tag.color} />
                                    </DropOption>
                                );
                            })}
                        </DropDown>
                    )}

                    <DropDown
                        title="Framerate"
                        storageKey="Framerate (fps)"
                        onChoose={async (value) => stream.setFrameRate(value === AUTO ? null : Number(value))}>
                        <DropOption key={AUTO} value={AUTO}>
                            Auto
                            {autoFps !== null && <Tag text={`${autoFps} FPS`} color={fpsColor(autoFps)} />}
                        </DropOption>
                        {fpsOptions.map((fps) => (
                            <DropOption key={fps} value={String(fps)}>
                                <Tag text={`${fps} FPS`} color={fpsColor(fps)} />
                            </DropOption>
                        ))}
                    </DropDown>
                </>
            )}
        </>
    );
}
