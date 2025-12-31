import Resolution from "./Resolution";
import { SetAlertType } from "../../context/AlertsContext";
import { SetVideoStreamType } from "../../context/VideoStreamContext";
import { SetVideoTrackType } from "../../context/VideoStreamContext";

export type infoType = {
    settings: MediaTrackSettings;
    capabilities: MediaTrackCapabilities;
};

export type StreamType = { setAlert: SetAlertType; setVideoStream: SetVideoStreamType; setVideoTrack: SetVideoTrackType };

export default class Stream {
    private mediaStream: MediaStream = new MediaStream();

    public videoId: string | null = null;
    public audioId: string | null = null;
    private currentVideoId: string | null = null;
    private currentAudioId: string | null = null;
    public resolutionTag: string | null = null;
    public frameRate: number | null = null;

    private capabilities: Map<string, MediaTrackCapabilities> = new Map(); // ← CACHE
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
        mediaStream.getTracks().forEach((track) => {
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

    getCapabilities(deviceId: string, track: MediaStreamTrack) {
        let capabilities = this.capabilities.get(deviceId);
        if (!capabilities) {
            capabilities = track.getCapabilities();
            this.capabilities.set(deviceId, capabilities);
        }
        return capabilities;
    }

    async updateVideoTrack() {
        if (!this.videoId) return;
        if (!this.videoId && this.videoId == this.currentVideoId && this.currentVideoId) return;

        if (this.videoTrack) {
            this.mediaStream.removeTrack(this.videoTrack);
            this.videoTrack.stop();
            this.videoTrack = null;
            this.setVideoTrack(this.videoTrack);
        }

        const tempStream = await navigator.mediaDevices.getUserMedia({ video: { deviceId: { exact: this.videoId } } });
        if (!tempStream) return;

        this.videoTrack = tempStream.getVideoTracks()[0];
        this.setVideoTrack(this.videoTrack);
        this.mediaStream.addTrack(this.videoTrack);

        const capabilities = this.getCapabilities(this.videoId, this.videoTrack);

        const constraints: MediaTrackConstraints = {};

        if (capabilities.width?.max) constraints.width = { ideal: capabilities.width.max };
        if (capabilities.height?.max) constraints.height = { ideal: capabilities.height.max };
        if (capabilities.frameRate?.max) constraints.frameRate = { ideal: capabilities.frameRate.max };

        try {
            await this.videoTrack.applyConstraints(constraints);
        } catch (error) {
            console.error(error);
        }

        this.setResolution();
        this.setFrameRate();
    }

    async setResolution() {
        if (!this.resolutionTag || !this.videoTrack) return;
        const resolution = Resolution.getFromTag(this.resolutionTag);
        if (!resolution) return;
        console.debug("Applying custom resolution");
        try {
            await this.videoTrack.applyConstraints({
                width: { exact: resolution?.width },
                height: { exact: resolution?.height },
            });
        } catch (error) {
            console.error(error);
        }
    }

    async setFrameRate() {
        if (!this.frameRate || !this.videoTrack) return;
        const framerate = this.frameRate ? (Number.isFinite(this.frameRate) && this.frameRate > 0 ? this.frameRate : null) : null;
        if (!framerate) return;
        console.debug("Applying custom framerate");
        try {
            await this.videoTrack.applyConstraints({
                frameRate: { exact: framerate },
            });
        } catch (error) {
            console.error(error);
        }
    }

    async updateAudioTrack() {
        if (!this.audioId) return;
        if (this.audioId == this.currentAudioId && this.currentAudioId) return;

        if (this.audioTrack) {
            this.mediaStream.removeTrack(this.audioTrack);
            this.audioTrack.stop();
            this.audioTrack = null;
            //this.setAudioTrack(this.audioTrack);
        }

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
        if (!tempStream) return;

        this.audioTrack = tempStream.getAudioTracks()[0];
        //this.setAudioTrack(this.audioTrack);
        this.mediaStream.addTrack(this.audioTrack);

        const constraints: MediaTrackConstraints = {};

        try {
            await this.audioTrack.applyConstraints(constraints);
        } catch (error) {
            console.error(error);
        }
    }

    getTrack(deviceId: string): MediaStreamTrack | null {
        const track = this.mediaStream.getTracks().find((track) => track.id === deviceId);
        if (!track) return null;
        return track;
    }

    async setResolutionTag(resolutionTag: string) {
        if (!this.videoTrack) return;

        const resolution = Resolution.getFromTag(resolutionTag);
        if (!resolution) {
            const capabilities = this.videoTrack.getCapabilities();
            const constraints: MediaTrackConstraints = {};

            if (capabilities.width?.max) constraints.width = { ideal: capabilities.width.max };
            if (capabilities.height?.max) constraints.height = { ideal: capabilities.height.max };

            await this.videoTrack.applyConstraints(constraints);
            return;
        }

        this.videoTrack.applyConstraints({
            width: { exact: resolution.width },
            height: { exact: resolution.height },
        });
    }

    async setFramerate(frameRate: number) {
        if (!this.videoTrack) return;
        if (Number.isFinite(frameRate) && frameRate > 0) {
            if (!this.videoTrack) return;
            this.videoTrack.applyConstraints({
                frameRate: { exact: frameRate },
            });
            return;
        }

        const capabilities = this.videoTrack.getCapabilities();
        const constraints: MediaTrackConstraints = {};

        if (capabilities.width?.max) constraints.width = { ideal: capabilities.width.max };
        if (capabilities.height?.max) constraints.height = { ideal: capabilities.height.max };

        await this.videoTrack.applyConstraints(constraints);
        return;
    }

    async checkPermissions() {
        const status = await navigator.permissions.query({ name: "camera" });
        if (status.state === "granted") {
            return true;
        }
        return false;
    }

    async getPermission() {
        try {
            const initialStream = await navigator.mediaDevices.getUserMedia({
                video: true,
                audio: true,
            });

            initialStream.getTracks().forEach((track) => track.stop());
            this.setAlert(null);
        } catch (error) {
            console.error("Error to get permission: ", error);
            this.setAlert({ text: "Permission denied", color: "darkred", icon: "./assets/Alerts/error.svg" });
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
        this.setAlert({ text: text, color: "darkred", icon: "./assets/Alerts/error.svg" });
    }

    handleWarning(text: string) {
        console.warn(text);
        this.setAlert({ text: text, color: "goldenrod", icon: "./assets/Alerts/warning.svg" });
    }

    debug(constraints: MediaStreamConstraints, mediaStream: MediaStream) {
        const videoInfo = Stream.getVideoInfo(mediaStream);
        if (!videoInfo) return;
        let definedRes: { width?: number; height?: number } | null = null;

        if (constraints.video && typeof constraints.video === "object") {
            definedRes = {
                width: typeof constraints.video.width === "object" ? constraints.video.width.ideal : undefined,
                height: typeof constraints.video.height === "object" ? constraints.video.height.ideal : undefined,
            };
        }
        const currentRes = { width: videoInfo.settings.width, height: videoInfo.settings.height };

        console.debug(
            `Defined Resolution: ${definedRes?.width} x ${definedRes?.height}\nCurrent Resolution: ${currentRes?.width} x ${currentRes?.height}`
        );

        if (!currentRes.height || !definedRes?.height) return;

        if (currentRes.height < definedRes?.height) {
            this.handleWarning("Resolution not supported, falling back to closest supported resolution");
        }
    }

    static getVideoInfo(mediaStream: MediaStream): infoType | null {
        const videoTrack = mediaStream.getVideoTracks()[0];

        if (videoTrack) {
            return { settings: videoTrack.getSettings(), capabilities: videoTrack.getCapabilities() };
        }

        return null;
    }

    static getAudioInfo(mediaStream: MediaStream): infoType | null {
        const audioTrack = mediaStream.getAudioTracks()[0];

        if (audioTrack) {
            return { settings: audioTrack.getSettings(), capabilities: audioTrack.getCapabilities() };
        }

        return null;
    }

    static isCapturing(mediaStream: MediaStream): boolean {
        if (!mediaStream) return false;
        return mediaStream.getTracks().some((track) => track.readyState === "live" && track.enabled);
    }
}
