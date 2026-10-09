import { alerts } from "./alerts";
import { chooseMode, type Size, type VideoMode } from "./videoModes";
import { monitorRefreshRate } from "./refreshRate";
import { SetAlertType } from "../../context/AlertsContext";
import { SetVideoStreamType } from "../../context/VideoStreamContext";
import { SetVideoTrackType } from "../../context/VideoStreamContext";
import { NativeScreenCapture } from "../native/screenCapture";
import { errorText } from "../native/api";
import { t } from "../../i18n";

// `resizeMode` ainda não está nos tipos do TypeScript.
type VideoConstraints = MediaTrackConstraints & { resizeMode?: "none" | "crop-and-scale" };

const MAX_WIDTH = 7680;
const MAX_HEIGHT = 4320;
const AUTO_TARGET_FPS = 60;
const FPS_TOLERANCE = 0.97;

/** Valor especial do dropdown de vídeo: seletor de tela do navegador (só fora do app, sem a captura nativa). */
export const SCREEN_SOURCE = "screen";

/** Telas e janelas da captura nativa: `screen:\\.\DISPLAY2`, `window:<hwnd>`. */
export const isCaptureSource = (id: string) => id.startsWith("screen:") || id.startsWith("window:");

export type StreamType = { setAlert: SetAlertType; setVideoStream: SetVideoStreamType; setVideoTrack: SetVideoTrackType };

export default class Stream {
    private mediaStream: MediaStream = new MediaStream();

    private videoId: string | null = null;
    /** device = câmera/placa; screen = seletor do navegador; capture = tela/janela pelo Rust. */
    private source: "device" | "screen" | "capture" = "device";
    private nativeCapture: NativeScreenCapture | null = null;
    /** Taxa de atualização da tela capturada (FPS do automático). */
    private captureRefreshRate = 60;
    public audioId: string | null = null;
    private currentAudioId: string | null = null;
    private resolution: Size | null = null;
    private frameRate: number | null = null;
    /** Modos reais do dispositivo atual (null = desconhecidos, usa a busca antiga). */
    private modes: VideoMode[] | null = null;

    private restartRequested = false;
    private restarting: Promise<void> | null = null;
    private setAlert: SetAlertType;
    private setVideoTrack: SetVideoTrackType;

    private videoTrack: MediaStreamTrack | null = null;
    private audioTrack: MediaStreamTrack | null = null;

    constructor({ setAlert, setVideoStream, setVideoTrack }: StreamType) {
        this.setAlert = setAlert;
        this.setVideoTrack = setVideoTrack;
        // Mede já: na hora de capturar a tela, o seletor precisa abrir logo após o clique.
        monitorRefreshRate();
        setVideoStream(this.mediaStream);
    }

    get isScreen() {
        return this.source === "screen";
    }

    /** Tela ou janela capturada pelo Rust. `refreshRate` = FPS do automático. */
    setCaptureSource(id: string, refreshRate: number) {
        this.source = "capture";
        this.videoId = id;
        this.modes = null;
        this.resolution = null;
        this.captureRefreshRate = refreshRate;
        return this.requestVideoRestart();
    }

    /** `modes`: modos reais do dispositivo, se conhecidos. */
    setVideoDevice(deviceId: string, modes: VideoMode[] | null = null) {
        if (deviceId === SCREEN_SOURCE) {
            this.source = "screen";
            this.videoId = null;
            this.modes = null;
            return this.chooseScreen();
        }
        this.source = "device";
        this.videoId = deviceId;
        this.modes = modes && modes.length > 0 ? modes : null;
        return this.requestVideoRestart();
    }

    /** Modo completo de uma vez (uma reabertura só). `null` = automático. */
    setMode(mode: (Size & { fps: number }) | null) {
        this.resolution = mode ? { width: mode.width, height: mode.height } : null;
        this.frameRate = mode?.fps ?? null;
        return this.requestVideoRestart();
    }

    /** `null` = automático. */
    setResolution(resolution: Size | null) {
        this.resolution = resolution;
        return this.isScreen ? this.applyScreenConstraints() : this.requestVideoRestart();
    }

    /** `null` = automático. */
    setFrameRate(frameRate: number | null) {
        this.frameRate = frameRate;
        if (this.source === "capture") {
            const fps = frameRate ?? this.captureRefreshRate;
            if (this.nativeCapture) return this.nativeCapture.setFps(fps).then(() => this.videoTrack && this.setVideoTrack(this.videoTrack));
            return;
        }
        return this.isScreen ? this.applyScreenConstraints() : this.requestVideoRestart();
    }

    /** Automático na tela = taxa do monitor (sem pedir, o Chromium captura a 30 fps). */
    private async screenConstraints(): Promise<MediaTrackConstraints> {
        const resolution = this.resolution;
        const fps = this.frameRate ?? (await monitorRefreshRate());
        return {
            frameRate: { ideal: fps, max: fps },
            ...(resolution ? { width: { ideal: resolution.width }, height: { ideal: resolution.height } } : {}),
        };
    }

    private async applyScreenConstraints() {
        await this.videoTrack?.applyConstraints(await this.screenConstraints()).catch(() => {});
        if (this.videoTrack) this.setVideoTrack(this.videoTrack);
    }

    private closeVideoTrack() {
        this.nativeCapture?.stop();
        this.nativeCapture = null;
        if (!this.videoTrack) return;
        this.mediaStream.removeTrack(this.videoTrack);
        this.videoTrack.stop();
        this.videoTrack = null;
        this.setVideoTrack(null);
    }

    /** Abre o seletor do Windows para escolher monitor ou janela. */
    async chooseScreen() {
        this.closeVideoTrack();
        try {
            const stream = await navigator.mediaDevices.getDisplayMedia({ video: await this.screenConstraints(), audio: false });
            const track = stream.getVideoTracks()[0];
            track.addEventListener("ended", () => this.handleVideoEnded(track));
            this.videoTrack = track;
            this.mediaStream.addTrack(track);
            this.setVideoTrack(track);
            this.setAlert(null);
        } catch (error) {
            const name = error instanceof DOMException ? error.name : String(error);
            if (name === "NotAllowedError") this.handleWarning(t("stream.screenCancelled"));
            else this.handleError(t("stream.screenFailed", { detail: name }));
        }
    }

    // Os dropdowns disparam várias mudanças seguidas na inicialização; agrupa tudo em
    // reaberturas sequenciais do dispositivo em vez de abrir várias em paralelo.
    private requestVideoRestart() {
        this.restartRequested = true;
        if (!this.restarting) {
            this.restarting = (async () => {
                // Espera um tick para juntar as mudanças disparadas no mesmo render.
                await new Promise((resolve) => setTimeout(resolve, 0));
                while (this.restartRequested) {
                    this.restartRequested = false;
                    await this.openVideoTrack();
                }
            })().finally(() => (this.restarting = null));
        }
        return this.restarting;
    }

    /**
     * Tentativas em ordem de preferência. `resizeMode: "none"` obriga o navegador a usar um modo
     * nativo do dispositivo; sem isso o Chromium mantém o modo atual e só reescala por software.
     */
    private buildAttempts(): { constraints: VideoConstraints; scaled: boolean }[] {
        // Com os modos reais: pede exatamente o modo escolhido (ou o automático) da lista.
        const mode = this.modes ? chooseMode(this.modes, this.resolution, this.frameRate) : null;
        if (mode) {
            const base = { deviceId: { exact: this.videoId! }, width: { exact: mode.width }, height: { exact: mode.height } };
            return [
                { constraints: { ...base, frameRate: { min: mode.fps * FPS_TOLERANCE, ideal: mode.fps }, resizeMode: "none" }, scaled: false },
                { constraints: { ...base, frameRate: { ideal: mode.fps }, resizeMode: "none" }, scaled: false },
            ];
        }

        const resolution = this.resolution;
        const size: VideoConstraints = resolution
            ? { width: { exact: resolution.width }, height: { exact: resolution.height } }
            : { width: { ideal: MAX_WIDTH }, height: { ideal: MAX_HEIGHT } };

        // Automático: o maior possível mantendo ~60 fps. Com FPS escolhido: o mais próximo dele.
        const targetFps = this.frameRate ?? AUTO_TARGET_FPS;
        // Margem para modos como 59.94 / 29.97.
        const minFps = targetFps * FPS_TOLERANCE;

        const base = { deviceId: { exact: this.videoId! }, ...size };
        const attempts: { constraints: VideoConstraints; scaled: boolean }[] = [
            { constraints: { ...base, frameRate: { min: minFps, ideal: targetFps }, resizeMode: "none" }, scaled: false },
            { constraints: { ...base, frameRate: { ideal: targetFps }, resizeMode: "none" }, scaled: false },
        ];
        // Resolução escolhida que não existe nativamente: último recurso é escalar por software.
        if (resolution) attempts.push({ constraints: { ...base, frameRate: { ideal: targetFps }, resizeMode: "crop-and-scale" }, scaled: true });
        return attempts;
    }

    private async openCapture(id: string) {
        this.closeVideoTrack();
        try {
            let track: MediaStreamTrack | null = null;
            const capture = await NativeScreenCapture.start(id, this.frameRate ?? this.captureRefreshRate, true, () => track && this.handleVideoEnded(track));
            // Trocou de fonte enquanto abria.
            if (this.videoId !== id || this.source !== "capture") return capture.stop();
            track = capture.track;
            this.nativeCapture = capture;
            this.videoTrack = track;
            this.mediaStream.addTrack(track);
            this.setVideoTrack(track);
            this.setAlert(null);
        } catch (error) {
            this.handleError(errorText(error));
        }
    }

    private async openVideoTrack() {
        if (this.source === "capture" && this.videoId) return this.openCapture(this.videoId);
        if (!this.videoId || this.isScreen) return;

        // Fecha antes de abrir: muitas placas de captura não aceitam duas sessões ao mesmo tempo,
        // e é a reabertura que faz o driver trocar de modo.
        this.closeVideoTrack();

        for (const { constraints, scaled } of this.buildAttempts()) {
            try {
                const tempStream = await navigator.mediaDevices.getUserMedia({ video: constraints });
                const track = tempStream.getVideoTracks()[0];
                track.addEventListener("ended", () => this.handleVideoEnded(track));
                this.videoTrack = track;
                this.mediaStream.addTrack(this.videoTrack);
                this.setVideoTrack(this.videoTrack);

                const { width, height, frameRate } = this.videoTrack.getSettings();
                console.debug(`Video opened at ${width}x${height} @ ${frameRate} fps`, constraints);
                if (scaled) this.handleWarning(t("stream.scaled", { size: `${width}x${height}` }));
                else this.setAlert(null);
                return;
            } catch (error) {
                if (error instanceof DOMException && error.name === "OverconstrainedError") continue;
                this.handleError(t("stream.videoFailed", { detail: error instanceof DOMException ? error.name : String(error) }));
                return;
            }
        }

        this.handleError(t("stream.unsupportedMode"));
    }

    private handleVideoEnded(track: MediaStreamTrack) {
        if (this.videoTrack !== track) return; // fechada por nós numa reabertura
        this.mediaStream.removeTrack(track);
        this.videoTrack = null;
        this.setVideoTrack(null);
        this.nativeCapture?.stop();
        this.nativeCapture = null;
        const message =
            this.source === "capture" && this.videoId?.startsWith("window:")
                ? t("stream.windowClosed")
                : this.source === "capture" || this.isScreen
                  ? t("stream.screenStopped")
                  : t("stream.videoDisconnected");
        this.handleWarning(message);
    }

    private handleAudioEnded(track: MediaStreamTrack) {
        if (this.audioTrack !== track) return;
        this.mediaStream.removeTrack(track);
        this.audioTrack = null;
        this.currentAudioId = null;
        this.handleWarning(t("stream.audioDisconnected"));
    }

    /** Chamado quando a lista de dispositivos muda: reabre o que caiu e voltou. */
    handleDeviceChange(devices: MediaDeviceInfo[]) {
        const available = (id: string | null) => id !== null && devices.some((device) => device.deviceId === id);
        if (this.source === "device" && !this.videoTrack && available(this.videoId)) this.requestVideoRestart();
        if (!this.audioTrack && available(this.audioId)) this.updateAudioTrack();
    }

    /** Para de tocar o áudio pelo WebView (o preview nativo do Rust assumiu). */
    clearAudio() {
        if (this.audioTrack) {
            this.mediaStream.removeTrack(this.audioTrack);
            this.audioTrack.stop();
            this.audioTrack = null;
        }
        this.currentAudioId = null;
    }

    async updateAudioTrack() {
        if (!this.audioId) return;
        if (this.audioId == this.currentAudioId && this.currentAudioId) return;

        if (this.audioTrack) {
            this.mediaStream.removeTrack(this.audioTrack);
            this.audioTrack.stop();
            this.audioTrack = null;
        }

        try {
            const tempStream = await navigator.mediaDevices.getUserMedia({
                audio: {
                    deviceId: { exact: this.audioId },
                    sampleRate: { ideal: 48000 },
                    channelCount: { ideal: 2 },
                    echoCancellation: false,
                    noiseSuppression: false,
                    autoGainControl: false,
                },
            });
            const track = tempStream.getAudioTracks()[0];
            track.addEventListener("ended", () => this.handleAudioEnded(track));
            this.audioTrack = track;
            this.mediaStream.addTrack(track);
            this.currentAudioId = this.audioId;
        } catch (error) {
            this.handleError(t("stream.audioFailed", { detail: error instanceof DOMException ? error.name : String(error) }));
        }
    }

    static async getVideoDevices(): Promise<MediaDeviceInfo[]> {
        const deviceList = await navigator.mediaDevices.enumerateDevices();
        const videoDevices = deviceList.filter((device) => device.kind === "videoinput");
        return videoDevices;
    }

    static async getAudioDevices(): Promise<MediaDeviceInfo[]> {
        const deviceList = await navigator.mediaDevices.enumerateDevices();
        const audioDevices = deviceList.filter((device) => device.kind === "audioinput");
        return audioDevices;
    }

    handleError(text: string) {
        console.error(text);
        this.setAlert(alerts.error(text));
    }

    handleWarning(text: string) {
        console.warn(text);
        this.setAlert(alerts.warning(text));
    }
}
