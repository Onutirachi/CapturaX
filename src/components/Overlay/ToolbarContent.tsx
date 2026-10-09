import { DropDown, DropOption } from "../../core/components/DropDown";
import Tag from "../../core/components/Tag";
import { RESOLUTION_LIST, FRAMERATE_LIST } from "../../core/utils/constants";
import { useState, useEffect, useContext, useRef } from "preact/hooks";
import Stream, { SCREEN_SOURCE } from "../../core/utils/Stream";
import { nativeCaptureAvailable } from "../../core/native/screenCapture";
import Icon from "./Icon";
import { VideoStreamContext } from "../../context/VideoStreamContext";
import { AlertsContext } from "../../context/AlertsContext";
import { NativeContext, type OutputKind } from "../../context/NativeContext";
import { native } from "../../core/native/api";
import Resolution from "../../core/utils/Resolution";
import { monitorRefreshRate } from "../../core/utils/refreshRate";
import { chooseMode, chooseSmoothest, choicesOf, deviceName, findDevice, pairedAudio, formatFps, fpsColor, modeKey, parseModeKey, parseSizeKey, sizeKey, type Size, type VideoDeviceModes } from "../../core/utils/videoModes";
import { useT } from "../../i18n";

const NO_AUDIO = "none";
const AUTO_AUDIO = "auto";
const AUTO = "Auto";
const SMOOTHEST = "Smoothest";

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
    // Telas e janelas ficam na gravação/transmissão; aqui só o seletor do navegador, fora do app.
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
        if (!videoTrack) return;
        vsctx?.setPairedAudioId(paired?.id ?? null);
        native.audioSetPairedInput(paired?.id ?? null).catch(() => {});
    }, [videoTrack, paired?.id]);

    if (!vsctx || !actx || !stream) return null;
    if (deviceModes === null) return null;

    const modesOf = (id: string | null) => {
        const label = videoDevicesList.find((d) => d.deviceId === id)?.label ?? "";
        return findDevice(deviceModes, label)?.modes ?? null;
    };
    const autoFps = isScreen ? refreshRate : null;
    const modes = isScreen ? null : modesOf(videoId);
    // Vendo o que a gravação/transmissão manda: o preview fica travado até voltar.
    const locked = !!nctx?.feedback;
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
                disabled={locked}
                onChoose={async (value) => {
                    setIsScreen(value === SCREEN_SOURCE);
                    setVideoId(value);
                    const deviceModes = value === SCREEN_SOURCE ? null : modesOf(value);
                    vsctx.setDeviceModes(deviceModes);
                    stream.setVideoDevice(value, deviceModes);
                }}>
                {videoDevicesList.map(({ deviceId, label }) => (
                    <DropOption key={deviceId} value={deviceId}>
                        {deviceName(label)}
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
                        disabled={locked}
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
                    disabled={locked}
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
                    <DropDown
                        title={t("toolbar.resolution")}
                        storageKey="Resolution (size)"
                        disabled={locked}
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

                    <DropDown
                        title={t("toolbar.framerate")}
                        storageKey="Framerate (fps)"
                        disabled={locked}
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

            <FeedbackButton kind="record" />
            <FeedbackButton kind="stream" />
        </>
    );
}

/**
 * "Ver gravação" / "Ver live": aparece enquanto a ação acontece e troca o preview pelo que está
 * saindo (o menu de cima trava até voltar). Apagado quando a fonte já é a do preview.
 */
function FeedbackButton({ kind }: { kind: OutputKind }) {
    const t = useT();
    const nctx = useContext(NativeContext);
    const output = nctx?.outputs[kind];
    if (!nctx || !output) return null;
    const samePreview = output.source === null;
    const active = nctx.feedback === kind;
    return (
        <button
            disabled={samePreview}
            title={samePreview ? t("toolbar.feedbackSame") : active ? t("toolbar.feedbackBack") : t("toolbar.feedbackHint", { source: output.label })}
            onClick={() => nctx.setFeedback(active ? null : kind)}
            className={`h-11 px-3 flex items-center gap-2 rounded-xl [border:var(--border)] transition cursor-pointer disabled:opacity-40 disabled:cursor-not-allowed ${
                active ? (kind === "record" ? "bg-bad/30" : "bg-accent/30") : "bg-dark hover:bg-normal"
            }`}>
            <Icon src={kind === "record" ? "./assets/Dock/record.svg" : "./assets/Dock/live.svg"} color={kind === "record" ? "var(--color-bad)" : "var(--color-accent)"} />
            {kind === "record" ? t("toolbar.viewRecording") : t("toolbar.viewLive")}
        </button>
    );
}
