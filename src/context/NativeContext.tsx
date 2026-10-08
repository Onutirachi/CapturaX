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
export type SettingsTab = "audio" | "apps" | "recording" | "streaming";

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
    toggleStreaming: () => Promise<void>;
    screenshot: () => Promise<void>;
    settingsTab: SettingsTab | null;
    openSettings: (tab?: SettingsTab | null) => void;
};

export const NativeContext = createContext<NativeContextType | null>(null);

const SAVE_DELAY_MS = 250;

function sourceInfo(track: MediaStreamTrack): SourceInfo {
    const settings = track.getSettings();
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

    const configRef = useRef<Config | null>(null);
    const saveTimer = useRef<number | null>(null);
    const recorder = useRef<EncoderSession | null>(null);
    const streamer = useRef<EncoderSession | null>(null);
    const trackRef = useRef<MediaStreamTrack | null>(null);
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

    const watchServer = streaming !== "idle" || settingsTab === "streaming" || !!config?.stream.acceptViewers;
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

    // Dispositivo reaberto (troca de resolução, reconexão): os encoders seguem com a trilha nova.
    useEffect(() => {
        if (!videoTrack) return;
        recorder.current?.setTrack(videoTrack);
        streamer.current?.setTrack(videoTrack);
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

    const startStreaming = async () => {
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
            setStreaming("active");
            alert(alerts.info("You are live: share the link from Settings → Streaming"));
        } catch (error) {
            setStreaming("idle");
            alert(alerts.error(`Could not start the stream (${errorText(error)})`));
        }
    };

    const stopStreaming = async () => {
        setStreaming("stopping");
        try {
            await streamer.current?.stop();
            streamer.current = null;
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
        await (streaming === "active" ? stopStreaming() : startStreaming());
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
                screenshot,
                settingsTab,
                openSettings: (tab = "audio") => setSettingsTab(tab),
            }}>
            {children}
        </NativeContext.Provider>
    );
}
