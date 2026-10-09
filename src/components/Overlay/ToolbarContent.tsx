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
import { chooseMode, chooseSmoothest, choicesOf, deviceName, findDevice, pairedAudio, formatFps, fpsColor, modeKey, parseModeKey, parseSizeKey, sizeKey, type Size, type VideoDeviceModes } from "../../core/utils/videoModes";
import { t as translate, useT } from "../../i18n";

const NO_AUDIO = "none";
const DEFAULT_OUTPUT = "default";
const AUTO_AUDIO = "auto";
const AUTO = "Auto";
const SMOOTHEST = "Smoothest";

/** "Screen 2 - 27G2G4": o número que o Windows dá à tela + o nome do monitor. */
const screenLabel = (screen: CaptureSources["screens"][number]) =>
    screen.name ? translate("toolbar.screenNamed", { number: screen.number, name: screen.name }) : translate("toolbar.screen", { number: screen.number });

/** Bandeirinha da resolução: o nome padrão, ou a proporção em cinza. */
const resolutionTag = (size: Size) => Resolution.getTag(size);

export default function ToolbarContent() {
    const t = useT();
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
    // Só aparece quando é diferente do automático (ex.: placa com 1440p60 e 1080p120).
    const smoothest = modes ? chooseSmoothest(modes) : null;
    const showSmoothest = !!smoothest && !!auto && modeKey(smoothest) !== modeKey(auto);

    // Sem modos fixos (tela ou dispositivo desconhecido): listas do app, na tela até o monitor.
    const screenSize = { width: Math.round(window.screen.width * devicePixelRatio), height: Math.round(window.screen.height * devicePixelRatio) };
    const sizes: Size[] = RESOLUTION_LIST.filter((r) => !isScreen || (r.width <= screenSize.width && r.height <= screenSize.height));
    const fpsOptions = FRAMERATE_LIST.filter((f) => autoFps === null || f <= autoFps);

    const setAudio = (patch: Partial<NonNullable<typeof config>["audio"]>) => nctx?.updateConfig((c) => ({ ...c, audio: { ...c.audio, ...patch } }));

    return (
        <>
            <DropDown
                title={t("toolbar.videoDevice")}
                storageKey="Video Device"
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
                {nativeCapture && videoDevicesList.length > 0 && <DropHeader>{t("toolbar.devices")}</DropHeader>}
                {videoDevicesList.map(({ deviceId, label }) => (
                    <DropOption key={deviceId} value={deviceId}>
                        {deviceName(label)}
                    </DropOption>
                ))}
                {nativeCapture && captureSources.screens.length > 0 && <DropHeader>{t("toolbar.screens")}</DropHeader>}
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
                {nativeCapture && captureSources.windows.length > 0 && <DropHeader>{t("toolbar.windows")}</DropHeader>}
                {captureSources.windows.map((window) => (
                    <DropOption key={window.id} value={window.id}>
                        {window.title}
                        {window.app && <Tag text={window.app} />}
                    </DropOption>
                ))}
                {!nativeCapture && (
                    <DropOption key={SCREEN_SOURCE} value={SCREEN_SOURCE}>
                        {t("toolbar.browserScreen")}
                    </DropOption>
                )}
            </DropDown>

            {isScreen && (
                <button className="h-11 px-4 rounded-xl bg-accent hover:brightness-110 transition cursor-pointer" onClick={() => stream.chooseScreen()}>
                    {t("toolbar.chooseScreen")}
                </button>
            )}

            {nativeAudio && config ? (
                <>
                    <DropDown
                        title={t("toolbar.audioDevice")}
                        value={config.audio.previewInputAuto ? AUTO_AUDIO : (config.audio.previewInput ?? NO_AUDIO)}
                        onChoose={(value) =>
                            setAudio(value === AUTO_AUDIO ? { previewInputAuto: true } : { previewInputAuto: false, previewInput: value === NO_AUDIO ? null : value })
                        }>
                        <DropOption value={AUTO_AUDIO}>{t("toolbar.autoNamed", { name: paired ? paired.name : t("toolbar.noAudioLower") })}</DropOption>
                        <DropOption value={NO_AUDIO} className="text-(--text-muted)">
                            {t("toolbar.noAudio")}
                        </DropOption>
                        {inputs.map(({ id, name }) => (
                            <DropOption key={id} value={id}>
                                {name}
                            </DropOption>
                        ))}
                    </DropDown>

                    <DropDown
                        title={t("toolbar.audioOutput")}
                        value={config.audio.previewOutput ?? DEFAULT_OUTPUT}
                        onChoose={(value) => setAudio({ previewOutput: value === DEFAULT_OUTPUT ? null : value })}>
                        <DropOption value={DEFAULT_OUTPUT}>
                            {t("toolbar.windowsDefault")}
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
                    title={t("toolbar.audioDevice")}
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
                    title={t("toolbar.mode")}
                    storageKey="Video mode"
                    onChoose={async (value) => stream.setMode(value === AUTO ? null : value === SMOOTHEST ? (modes && chooseSmoothest(modes)) : parseModeKey(value))}>
                    <DropOption key={AUTO} value={AUTO}>
                        {auto ? t("toolbar.autoNamed", { name: `${auto.width} x ${auto.height}` }) : t("toolbar.auto")}
                        {auto && <Tag text={resolutionTag(auto).text} color={resolutionTag(auto).color} />}
                        {auto && <Tag text={`${formatFps(auto.fps)} FPS`} color={fpsColor(auto.fps)} />}
                    </DropOption>
                    {showSmoothest && (
                        <DropOption key={SMOOTHEST} value={SMOOTHEST}>
                            {t("toolbar.smoothest", { size: `${smoothest.width} x ${smoothest.height}` })}
                            <Tag text={resolutionTag(smoothest).text} color={resolutionTag(smoothest).color} />
                            <Tag text={`${formatFps(smoothest.fps)} FPS`} color={fpsColor(smoothest.fps)} />
                        </DropOption>
                    )}
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
                            title={t("toolbar.resolution")}
                            storageKey="Resolution (size)"
                            onChoose={async (value) => stream.setResolution(value === AUTO ? null : parseSizeKey(value))}>
                            <DropOption key={AUTO} value={AUTO}>
                                {isScreen ? t("toolbar.autoNamed", { name: `${screenSize.width} x ${screenSize.height}` }) : t("toolbar.auto")}
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
                        title={t("toolbar.framerate")}
                        storageKey="Framerate (fps)"
                        onChoose={async (value) => stream.setFrameRate(value === AUTO ? null : Number(value))}>
                        <DropOption key={AUTO} value={AUTO}>
                            {t("toolbar.auto")}
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
