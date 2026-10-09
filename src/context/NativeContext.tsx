import { trackSettings } from "../core/native/screenCapture";
import { ComponentChildren, createContext } from "preact";
import { useCallback, useContext, useEffect, useRef, useState } from "preact/hooks";
import { native, errorText, type AudioDevices, type CaptureSources, type Config, type MediaStatus, type OutputSourceId, type ServerStatus } from "../core/native/api";
import { acquireCapture } from "../core/native/captureHub";
import { nativeCaptureAvailable } from "../core/native/screenCapture";
import { sourceLabel, sourceRefreshRate, validSource } from "../core/utils/outputSources";
import { calibrateClock } from "../core/native/clock";
import EncoderSession from "../core/media/EncoderSession";
import { recordingSettings, resolveSupported, streamSettings, type SourceInfo } from "../core/media/presets";
import { alerts, fileName } from "../core/utils/alerts";
import { AlertsContext } from "./AlertsContext";
import { VideoStreamContext } from "./VideoStreamContext";
import { applyLanguageSetting, t } from "../i18n";

type Activity = "idle" | "starting" | "active" | "stopping";
export type OutputKind = "record" | "stream";

/** O que uma gravação/transmissão em andamento está mandando. */
export type OutputInfo = { source: OutputSourceId; track: MediaStreamTrack; label: string; width: number; height: number; fps: number };

/** Trilha aberta para uma saída (tela/janela própria, ou a do preview). */
type OpenOutput = { source: OutputSourceId; track: MediaStreamTrack; release: () => void };
export type SettingsTab = "general" | "audio" | "recording" | "streaming";

export type NativeContextType = {
    config: Config | null;
    /** Atualiza na hora na interface e salva no Rust logo em seguida (agrupando mudanças seguidas). */
    updateConfig: (update: (config: Config) => Config) => void;
    devices: AudioDevices;
    status: MediaStatus | null;
    /** Servidor da transmissão (só consultado quando relevante). */
    server: ServerStatus | null;
    recording: Activity;
    streaming: Activity;
    toggleRecording: () => Promise<void>;
    /** Abre a janela da transmissão (parado: "Go live"; ao vivo: ajustes + parar). */
    toggleStreaming: () => Promise<void>;
    goLiveOpen: boolean;
    /** Parado: começa. Ao vivo: aplica a qualidade nova (reinicia a transmissão). */
    confirmGoLive: () => Promise<void>;
    cancelGoLive: () => void;
    /** A fonte mudou ao vivo: a janela só sai aplicando uma qualidade nova ou parando. */
    mustReconfigure: boolean;
    /** Janela de gravação (fonte + qualidade antes de começar). */
    recordDialogOpen: boolean;
    openRecordDialog: () => void;
    closeRecordDialog: () => void;
    confirmRecord: () => Promise<void>;
    /** Telas e janelas disponíveis como fonte da gravação/transmissão. */
    captureSources: CaptureSources;
    refreshCaptureSources: () => void;
    /** Gravação/transmissão em andamento (fonte, tamanho, FPS). */
    outputs: Record<OutputKind, OutputInfo | null>;
    /** Preview mostrando o que sai na gravação/transmissão (menu de cima travado). */
    feedback: OutputKind | null;
    setFeedback: (kind: OutputKind | null) => void;
    stopStreaming: () => Promise<void>;
    screenshot: () => Promise<void>;
    settingsTab: SettingsTab | null;
    openSettings: (tab?: SettingsTab | null) => void;
};

export const NativeContext = createContext<NativeContextType | null>(null);

const SAVE_DELAY_MS = 250;

function sourceInfo(track: MediaStreamTrack): SourceInfo {
    const settings = trackSettings(track);
    return { width: settings.width ?? 1920, height: settings.height ?? 1080, fps: settings.frameRate ?? 60 };
}

export function NativeProvider({ children }: { children: ComponentChildren }) {
    const vsctx = useContext(VideoStreamContext);
    const setAlert = useContext(AlertsContext)?.setAlert;
    const videoTrack = vsctx?.videoTrack ?? null;

    const [config, setConfig] = useState<Config | null>(null);
    const [devices, setDevices] = useState<AudioDevices>({ inputs: [], outputs: [] });
    const [status, setStatus] = useState<MediaStatus | null>(null);
    const [recording, setRecording] = useState<Activity>("idle");
    const [streaming, setStreaming] = useState<Activity>("idle");
    const [settingsTab, setSettingsTab] = useState<SettingsTab | null>(null);
    const [server, setServer] = useState<ServerStatus | null>(null);
    const [goLiveOpen, setGoLiveOpen] = useState(false);
    const [mustReconfigure, setMustReconfigure] = useState(false);
    const [recordDialogOpen, setRecordDialogOpen] = useState(false);
    const [captureSources, setCaptureSources] = useState<CaptureSources>({ screens: [], windows: [] });
    const [outputs, setOutputs] = useState<Record<OutputKind, OutputInfo | null>>({ record: null, stream: null });
    const [feedback, setFeedbackState] = useState<OutputKind | null>(null);
    const opened = useRef<Record<OutputKind, OpenOutput | null>>({ record: null, stream: null });
    const sourcesRef = useRef(captureSources);
    sourcesRef.current = captureSources;

    const configRef = useRef<Config | null>(null);
    const feedbackRef = useRef<OutputKind | null>(null);
    feedbackRef.current = feedback;
    const saveTimer = useRef<number | null>(null);
    const recorder = useRef<EncoderSession | null>(null);
    const streamer = useRef<EncoderSession | null>(null);
    const trackRef = useRef<MediaStreamTrack | null>(null);
    /** Trilha com que a transmissão foi configurada (outra fonte → perguntar a qualidade de novo). */
    const streamTrackId = useRef<string | null>(null);
    trackRef.current = videoTrack;

    const alert = useCallback((value: Parameters<NonNullable<typeof setAlert>>[0]) => setAlert?.(value), [setAlert]);

    useEffect(() => {
        native
            .getConfig()
            .then((loaded) => {
                configRef.current = loaded;
                setConfig(loaded);
            })
            .catch((error) => alert(alerts.error(t("native.loadFailed", { detail: errorText(error) }))));
        calibrateClock().catch(() => {});
    }, []);

    const updateConfig = useCallback((update: (config: Config) => Config) => {
        if (!configRef.current) return;
        const next = update(configRef.current);
        configRef.current = next;
        setConfig(next);
        if (saveTimer.current !== null) window.clearTimeout(saveTimer.current);
        saveTimer.current = window.setTimeout(() => {
            saveTimer.current = null;
            native.setConfig(configRef.current!).catch((error) => alert(alerts.error(t("native.saveFailed", { detail: errorText(error) }))));
        }, SAVE_DELAY_MS);
    }, []);

    const refreshCaptureSources = useCallback(() => {
        if (nativeCaptureAvailable()) native.captureSources().then(setCaptureSources).catch(() => {});
    }, []);
    useEffect(refreshCaptureSources, []);

    /** Abre a fonte de uma saída: a trilha do preview ou uma tela/janela (compartilhada se for a mesma). */
    const openOutput = async (kind: OutputKind, wanted: OutputSourceId, fps: number | null, onEnded: () => void): Promise<OpenOutput | null> => {
        // A lista pode estar velha (janela aberta agora): atualiza antes de validar.
        const sources = wanted ? await native.captureSources().catch(() => sourcesRef.current) : sourcesRef.current;
        if (wanted) setCaptureSources(sources);
        const source = validSource(wanted, sources);
        if (source === null) {
            const track = trackRef.current;
            return track ? { source: null, track, release: () => {} } : null;
        }
        const { track, release } = await acquireCapture(source, fps ?? sourceRefreshRate(source, sources), onEnded);
        opened.current[kind] = { source, track, release };
        return { source, track, release };
    };

    const closeOutput = (kind: OutputKind) => {
        opened.current[kind]?.release();
        opened.current[kind] = null;
        setOutputs((current) => ({ ...current, [kind]: null }));
        setFeedbackState((current) => (current === kind ? null : current));
    };

    const describeOutput = (kind: OutputKind, out: OpenOutput, settings: { width: number; height: number; fps: number }) =>
        setOutputs((current) => ({
            ...current,
            [kind]: { source: out.source, track: out.track, label: sourceLabel(out.source, sourcesRef.current, trackRef.current), ...settings },
        }));

    const setFeedback = (kind: OutputKind | null) => setFeedbackState(kind && opened.current[kind] ? kind : null);

    // Dispositivos de áudio do Windows; a lista do WebView avisa quando algo é plugado.
    useEffect(() => {
        const refresh = () =>
            native
                .audioDevices()
                .then(setDevices)
                .catch((error) => console.error("Could not list audio devices", error));
        refresh();
        navigator.mediaDevices.addEventListener("devicechange", refresh);
        return () => navigator.mediaDevices.removeEventListener("devicechange", refresh);
    }, []);

    useEffect(() => {
        const poll = () =>
            native
                .mediaStatus()
                .then(setStatus)
                .catch(() => {});
        poll();
        const interval = window.setInterval(poll, 1000);
        return () => window.clearInterval(interval);
    }, []);

    const watchServer = streaming !== "idle" || goLiveOpen || settingsTab === "streaming" || !!config?.stream.acceptViewers;
    useEffect(() => {
        if (!watchServer) return;
        const poll = () =>
            native
                .streamStatus()
                .then(setServer)
                .catch(() => {});
        poll();
        const interval = window.setInterval(poll, 2000);
        return () => window.clearInterval(interval);
    }, [watchServer]);

    // Idioma: o escolhido nas configurações ou o do Windows.
    const language = config?.language;
    useEffect(() => {
        if (config) applyLanguageSetting(language ?? null);
    }, [language, !!config]);

    // Volume do preview nativo acompanha o slider da Dock.
    const volume = vsctx?.volume ?? 1;
    const muted = vsctx?.muted ?? false;
    useEffect(() => {
        native.setPreviewVolume(volume, muted).catch(() => {});
    }, [volume, muted]);

    // Dispositivo reaberto (troca de fonte/modo, reconexão): os encoders seguem com a trilha nova.
    // Ao vivo, a qualidade escolhida pode não servir mais: reabre a janela para escolher de novo.
    useEffect(() => {
        if (!videoTrack) return;
        if (!opened.current.record) recorder.current?.setTrack(videoTrack);
        if (!opened.current.stream) streamer.current?.setTrack(videoTrack);
        for (const kind of ["record", "stream"] as const) {
            setOutputs((current) => (current[kind] && current[kind]!.source === null ? { ...current, [kind]: { ...current[kind]!, track: videoTrack, label: sourceLabel(null, sourcesRef.current, videoTrack) } } : current));
        }
        if (streamer.current && !opened.current.stream && streamTrackId.current !== videoTrack.id) {
            setMustReconfigure(true);
            setGoLiveOpen(true);
        }
    }, [videoTrack]);

    const startRecording = async () => {
        const current = configRef.current;
        if (!current) return;
        setRecording("starting");
        let out: OpenOutput | null = null;
        try {
            const fps = current.recording.preset === "custom" ? current.recording.fps : null;
            out = await openOutput("record", current.recording.source, fps, () => {
                alert(alerts.warning(t("native.recordSourceEnded")));
                stopRecordingRef.current();
            });
            if (!out) {
                setRecording("idle");
                alert(alerts.warning(t("native.nothingToRecord")));
                return;
            }
            const track = out.track;
            const settings = await resolveSupported(recordingSettings(current.recording, sourceInfo(track)));
            await calibrateClock();
            const { id, path } = await native.recordingStart();
            recorder.current = new EncoderSession(id, settings, track, (message) => alert(alerts.error(t("native.recordingError", { detail: message }))));
            describeOutput("record", out, settings);
            setRecording("active");
            alert(alerts.info(t("native.recordingStarted", { size: `${settings.width}x${settings.height}`, fps: settings.fps, file: fileName(path) })));
        } catch (error) {
            setRecording("idle");
            closeOutput("record");
            alert(alerts.error(t("native.recordingStartFailed", { detail: errorText(error) })));
        }
    };

    const stopRecording = async () => {
        setRecording("stopping");
        try {
            await recorder.current?.stop();
            recorder.current = null;
            const path = await native.recordingStop();
            alert(path ? alerts.success(t("native.recordingSaved", { file: fileName(path) })) : alerts.warning(t("native.recordingEmpty")));
        } catch (error) {
            alert(alerts.error(t("native.recordingStopFailed", { detail: errorText(error) })));
        } finally {
            closeOutput("record");
            setRecording("idle");
        }
    };
    const stopRecordingRef = useRef(stopRecording);
    stopRecordingRef.current = stopRecording;

    const startStreaming = async (restarting = false) => {
        const current = configRef.current;
        if (!current) return;
        setStreaming("starting");
        try {
            const out = await openOutput("stream", current.stream.source, current.stream.fps || null, () => {
                alert(alerts.warning(t("native.streamSourceEnded")));
                stopStreamingRef.current();
            });
            if (!out) {
                setStreaming("idle");
                setMustReconfigure(false);
                alert(alerts.warning(t("native.nothingToStream")));
                return;
            }
            const track = out.track;
            const settings = await resolveSupported(streamSettings(current.stream, sourceInfo(track)));
            await calibrateClock();
            const id = await native.streamStart();
            streamer.current = new EncoderSession(id, settings, track, (message) => alert(alerts.error(t("native.streamError", { detail: message }))));
            describeOutput("stream", out, settings);
            streamTrackId.current = track.id;
            setMustReconfigure(false);
            setStreaming("active");
            alert(alerts.info(restarting ? t("native.streamUpdated", { size: `${settings.width}x${settings.height}`, fps: settings.fps }) : t("native.live")));
        } catch (error) {
            setStreaming("idle");
            setMustReconfigure(false);
            closeOutput("stream");
            alert(alerts.error(t("native.streamStartFailed", { detail: errorText(error) })));
        }
    };

    const stopStreaming = async () => {
        setGoLiveOpen(false);
        setMustReconfigure(false);
        setStreaming("stopping");
        try {
            await streamer.current?.stop();
            streamer.current = null;
            streamTrackId.current = null;
            await native.streamStop();
            alert(alerts.info(t("native.streamEnded")));
        } catch (error) {
            alert(alerts.error(t("native.streamStopFailed", { detail: errorText(error) })));
        } finally {
            closeOutput("stream");
            setStreaming("idle");
        }
    };
    const stopStreamingRef = useRef(stopStreaming);
    stopStreamingRef.current = stopStreaming;

    const busy = (activity: Activity) => activity === "starting" || activity === "stopping";
    const toggleRecording = async () => {
        if (busy(recording)) return;
        await (recording === "active" ? stopRecording() : startRecording());
    };
    const toggleStreaming = async () => {
        if (busy(streaming)) return;
        refreshCaptureSources();
        setGoLiveOpen(true);
        // Parado: liga o servidor já, para o link (e a porta no roteador) estar pronto na janela.
        if (streaming !== "active") native.streamPrepare().catch(() => {});
    };

    /** Nova qualidade ao vivo: o formato do vídeo muda, então a transmissão recomeça (quem assiste
     * reconecta sozinho em 1–2 s). O servidor fica no ar durante a troca. */
    const restartStreaming = async () => {
        setStreaming("starting");
        try {
            await native.streamPrepare();
            await streamer.current?.stop();
            streamer.current = null;
            await native.streamStop();
        } catch (error) {
            alert(alerts.error(t("native.streamUpdateFailed", { detail: errorText(error) })));
        }
        closeOutput("stream");
        await startStreaming(true);
    };
    const confirmGoLive = async () => {
        if (busy(streaming)) return;
        // Garante que a qualidade escolhida na janela já foi salva antes de começar.
        if (saveTimer.current !== null) {
            window.clearTimeout(saveTimer.current);
            saveTimer.current = null;
            await native.setConfig(configRef.current!).catch(() => {});
        }
        setGoLiveOpen(false);
        await (streaming === "active" ? restartStreaming() : startStreaming());
    };
    const cancelGoLive = () => {
        if (mustReconfigure) return;
        setGoLiveOpen(false);
        // Ao vivo, fechar a janela só mantém como está.
        if (streaming !== "active") native.streamUnprepare().catch(() => {});
    };

    const openRecordDialog = () => {
        if (busy(recording)) return;
        refreshCaptureSources();
        setRecordDialogOpen(true);
    };
    const confirmRecord = async () => {
        if (busy(recording)) return;
        if (saveTimer.current !== null) {
            window.clearTimeout(saveTimer.current);
            saveTimer.current = null;
            await native.setConfig(configRef.current!).catch(() => {});
        }
        setRecordDialogOpen(false);
        await startRecording();
    };

    const screenshot = async () => {
        const track = (feedbackRef.current && opened.current[feedbackRef.current]?.track) || trackRef.current;
        if (!track) {
            alert(alerts.warning(t("native.nothingToCapture")));
            return;
        }
        try {
            const bitmap = await new ImageCapture(track).grabFrame();
            const canvas = new OffscreenCanvas(bitmap.width, bitmap.height);
            canvas.getContext("2d")!.drawImage(bitmap, 0, 0);
            bitmap.close();
            const blob = await canvas.convertToBlob({ type: "image/png" });
            const path = await native.saveScreenshot(new Uint8Array(await blob.arrayBuffer()));
            let copied = true;
            await navigator.clipboard.write([new ClipboardItem({ "image/png": blob })]).catch(() => (copied = false));
            alert(alerts.success(t(copied ? "native.screenshotCopied" : "native.screenshotSaved", { file: fileName(path) })));
        } catch (error) {
            alert(alerts.error(t("native.screenshotFailed", { detail: errorText(error) })));
        }
    };

    // Atalhos: F9 = print, F10 = gravar/parar.
    const actions = useRef({ screenshot, toggleRecording });
    actions.current = { screenshot, toggleRecording };
    useEffect(() => {
        const onKey = (event: KeyboardEvent) => {
            if (event.repeat) return;
            if (event.key === "F9") {
                event.preventDefault();
                actions.current.screenshot();
            } else if (event.key === "F10") {
                event.preventDefault();
                actions.current.toggleRecording();
            }
        };
        window.addEventListener("keydown", onKey);
        return () => window.removeEventListener("keydown", onKey);
    }, []);

    return (
        <NativeContext.Provider
            value={{
                config,
                updateConfig,
                devices,
                status,
                server,
                recording,
                streaming,
                toggleRecording,
                toggleStreaming,
                goLiveOpen,
                confirmGoLive,
                cancelGoLive,
                mustReconfigure,
                recordDialogOpen,
                openRecordDialog,
                closeRecordDialog: () => setRecordDialogOpen(false),
                confirmRecord,
                captureSources,
                refreshCaptureSources,
                outputs,
                feedback,
                setFeedback,
                stopStreaming,
                screenshot,
                settingsTab,
                openSettings: (tab = "audio") => setSettingsTab(tab),
            }}>
            {children}
        </NativeContext.Provider>
    );
}
