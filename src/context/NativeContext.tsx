import { trackSettings } from "../core/native/screenCapture";
import { ComponentChildren, createContext } from "preact";
import { useCallback, useContext, useEffect, useRef, useState } from "preact/hooks";
import { native, errorText, type AudioDevices, type Config, type MediaStatus, type ServerStatus } from "../core/native/api";
import { calibrateClock } from "../core/native/clock";
import EncoderSession from "../core/media/EncoderSession";
import { recordingSettings, resolveSupported, streamSettings, type SourceInfo } from "../core/media/presets";
import { alerts, fileName } from "../core/utils/alerts";
import { AlertsContext } from "./AlertsContext";
import { VideoStreamContext } from "./VideoStreamContext";

type Activity = "idle" | "starting" | "active" | "stopping";
export type SettingsTab = "audio" | "recording" | "streaming";

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

    const configRef = useRef<Config | null>(null);
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
            .catch((error) => alert(alerts.error(`Could not load settings (${errorText(error)})`)));
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
            native.setConfig(configRef.current!).catch((error) => alert(alerts.error(`Could not save settings (${errorText(error)})`)));
        }, SAVE_DELAY_MS);
    }, []);

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
        recorder.current?.setTrack(videoTrack);
        streamer.current?.setTrack(videoTrack);
        if (streamer.current && streamTrackId.current !== videoTrack.id) {
            setMustReconfigure(true);
            setGoLiveOpen(true);
        }
    }, [videoTrack]);

    const startRecording = async () => {
        const track = trackRef.current;
        const current = configRef.current;
        if (!track || !current) {
            alert(alerts.warning("Nothing to record: no video source"));
            return;
        }
        setRecording("starting");
        try {
            const settings = await resolveSupported(recordingSettings(current.recording, sourceInfo(track)));
            await calibrateClock();
            const { id, path } = await native.recordingStart();
            recorder.current = new EncoderSession(id, settings, track, (message) => alert(alerts.error(`Recording error: ${message}`)));
            setRecording("active");
            alert(alerts.info(`Recording ${settings.width}x${settings.height} @ ${settings.fps} fps → ${fileName(path)}`));
        } catch (error) {
            setRecording("idle");
            alert(alerts.error(`Could not start recording (${errorText(error)})`));
        }
    };

    const stopRecording = async () => {
        setRecording("stopping");
        try {
            await recorder.current?.stop();
            recorder.current = null;
            const path = await native.recordingStop();
            alert(path ? alerts.success(`Saved ${fileName(path)}`) : alerts.warning("Recording was empty and was discarded"));
        } catch (error) {
            alert(alerts.error(`Could not finish recording (${errorText(error)})`));
        } finally {
            setRecording("idle");
        }
    };

    const startStreaming = async (restarting = false) => {
        const track = trackRef.current;
        const current = configRef.current;
        if (!track || !current) {
            alert(alerts.warning("Nothing to stream: no video source"));
            return;
        }
        setStreaming("starting");
        try {
            const settings = await resolveSupported(streamSettings(current.stream, sourceInfo(track)));
            await calibrateClock();
            const id = await native.streamStart();
            streamer.current = new EncoderSession(id, settings, track, (message) => alert(alerts.error(`Stream error: ${message}`)));
            streamTrackId.current = track.id;
            setMustReconfigure(false);
            setStreaming("active");
            alert(alerts.info(restarting ? `Stream updated: ${settings.width}x${settings.height} @ ${settings.fps} fps` : "You are live"));
        } catch (error) {
            setStreaming("idle");
            setMustReconfigure(false);
            alert(alerts.error(`Could not start the stream (${errorText(error)})`));
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
            alert(alerts.info("Stream ended"));
        } catch (error) {
            alert(alerts.error(`Could not stop the stream (${errorText(error)})`));
        } finally {
            setStreaming("idle");
        }
    };

    const busy = (activity: Activity) => activity === "starting" || activity === "stopping";
    const toggleRecording = async () => {
        if (busy(recording)) return;
        await (recording === "active" ? stopRecording() : startRecording());
    };
    const toggleStreaming = async () => {
        if (busy(streaming)) return;
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
            alert(alerts.error(`Could not update the stream (${errorText(error)})`));
        }
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

    const screenshot = async () => {
        const track = trackRef.current;
        if (!track) {
            alert(alerts.warning("Nothing to capture: no video source"));
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
            alert(alerts.success(`Screenshot saved${copied ? " and copied" : ""}: ${fileName(path)}`));
        } catch (error) {
            alert(alerts.error(`Could not take screenshot (${errorText(error)})`));
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
                stopStreaming,
                screenshot,
                settingsTab,
                openSettings: (tab = "audio") => setSettingsTab(tab),
            }}>
            {children}
        </NativeContext.Provider>
    );
}
