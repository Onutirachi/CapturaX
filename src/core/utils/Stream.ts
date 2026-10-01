import Resolution from "./Resolution";
import { SetAlertType } from "../../context/AlertsContext";
import { SetVideoStreamType } from "../../context/VideoStreamContext";
import { SetVideoTrackType } from "../../context/VideoStreamContext";

export type infoType = {
    settings: MediaTrackSettings;
    capabilities: MediaTrackCapabilities;
};

// `resizeMode` ainda não está nos tipos do TypeScript.
type VideoConstraints = MediaTrackConstraints & { resizeMode?: "none" | "crop-and-scale" };

const MAX_WIDTH = 7680;
const MAX_HEIGHT = 4320;
const AUTO_TARGET_FPS = 60;
const FPS_TOLERANCE = 0.97;

export type StreamType = { setAlert: SetAlertType; setVideoStream: SetVideoStreamType; setVideoTrack: SetVideoTrackType };

export default class Stream {
    private mediaStream: MediaStream = new MediaStream();

    private videoId: string | null = null;
    public audioId: string | null = null;
    private currentAudioId: string | null = null;
    private resolutionTag: string | null = null;
    private frameRate: number | null = null;

    private restartRequested = false;
    private restarting: Promise<void> | null = null;
    private setAlert: SetAlertType;
    private setVideoTrack: SetVideoTrackType;

    private videoTrack: MediaStreamTrack | null = null;
    private audioTrack: MediaStreamTrack | null = null;

    constructor({ setAlert, setVideoStream, setVideoTrack }: StreamType) {
        this.setAlert = setAlert;
        this.setVideoTrack = setVideoTrack;
        setVideoStream(this.mediaStream);
    }

    stopTracks(mediaStream: MediaStream) {
        mediaStream.getTracks().forEach((track) => {
            track.stop();
            mediaStream.removeTrack(track);
        });
    }

    stopVideoTracks(mediaStream: MediaStream) {
        mediaStream.getVideoTracks().forEach((track) => {
            track.stop();
            mediaStream.removeTrack(track);
        });
    }

    stopAudioTracks(mediaStream: MediaStream) {
        mediaStream.getAudioTracks().forEach((track) => {
            track.stop();
            mediaStream.removeTrack(track);
        });
    }

    setVideoDevice(deviceId: string) {
        this.videoId = deviceId;
        return this.requestVideoRestart();
    }

    /** `null` = automático. */
    setResolutionTag(resolutionTag: string | null) {
        this.resolutionTag = resolutionTag;
        return this.requestVideoRestart();
    }

    /** `null` = automático. */
    setFrameRate(frameRate: number | null) {
        this.frameRate = frameRate;
        return this.requestVideoRestart();
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
        const resolution = this.resolutionTag ? Resolution.getFromTag(this.resolutionTag) : null;
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

    private async openVideoTrack() {
        if (!this.videoId) return;

        // Fecha antes de abrir: muitas placas de captura não aceitam duas sessões ao mesmo tempo,
        // e é a reabertura que faz o driver trocar de modo.
        if (this.videoTrack) {
            this.mediaStream.removeTrack(this.videoTrack);
            this.videoTrack.stop();
            this.videoTrack = null;
            this.setVideoTrack(null);
        }

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
                if (scaled) this.handleWarning(`${width}x${height} is not native on this device; scaling in software`);
                else this.setAlert(null);
                return;
            } catch (error) {
                if (error instanceof DOMException && error.name === "OverconstrainedError") continue;
                this.handleError(`Could not open video device (${error instanceof DOMException ? error.name : error})`);
                return;
            }
        }

        this.handleError("This device does not support the selected resolution/framerate");
    }

    private handleVideoEnded(track: MediaStreamTrack) {
        if (this.videoTrack !== track) return; // fechada por nós numa reabertura
        this.mediaStream.removeTrack(track);
        this.videoTrack = null;
        this.setVideoTrack(null);
        this.handleWarning("Video device disconnected");
    }

    private handleAudioEnded(track: MediaStreamTrack) {
        if (this.audioTrack !== track) return;
        this.mediaStream.removeTrack(track);
        this.audioTrack = null;
        this.currentAudioId = null;
        this.handleWarning("Audio device disconnected");
    }

    /** Chamado quando a lista de dispositivos muda: reabre o que caiu e voltou. */
    handleDeviceChange(devices: MediaDeviceInfo[]) {
        const available = (id: string | null) => id !== null && devices.some((device) => device.deviceId === id);
        if (!this.videoTrack && available(this.videoId)) this.requestVideoRestart();
        if (!this.audioTrack && available(this.audioId)) this.updateAudioTrack();
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
            this.handleError(`Could not open audio device (${error instanceof DOMException ? error.name : error})`);
        }
    }

    async checkPermissions() {
        const status = await navigator.permissions.query({ name: "camera" as PermissionName });
        if (status.state === "granted") {
            return true;
        }
        return false;
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
        this.setAlert({ text: text, color: "darkred", icon: "./assets/Alerts/error.svg" });
    }

    handleWarning(text: string) {
        console.warn(text);
        this.setAlert({ text: text, color: "goldenrod", icon: "./assets/Alerts/warning.svg" });
    }
}
