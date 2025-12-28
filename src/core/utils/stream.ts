import Resolution from "./Resolution";

export type infoType = {
    settings: MediaTrackSettings;
    capabilities: MediaTrackCapabilities;
};

export default class Stream {
    private mediaStream: MediaStream | null = null;
    private videoDeviceId: string | null = null;
    private resolutionTag: string | null = null;
    private frameRate: number | null = null;
    private audioDeviceId: string | null = null;

    constructor() {}

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
        } catch (error) {
            console.error("Error to get permission: ", error);
        }
    }

    async getVideoDevices(): Promise<MediaDeviceInfo[]> {
        const deviceList = await navigator.mediaDevices.enumerateDevices();
        const videoDevices = deviceList.filter((device) => device.kind === "videoinput");
        return videoDevices;
    }

    async getAudioDevices(): Promise<MediaDeviceInfo[]> {
        const deviceList = await navigator.mediaDevices.enumerateDevices();
        const audioDevices = deviceList.filter((device) => device.kind === "audioinput");
        return audioDevices;
    }

    async getMediaStream(): Promise<MediaStream | null> {
        if (!this.videoDeviceId) {
            console.log("No device id");
            return null;
        }

        const video: MediaTrackConstraints = {
            deviceId: { exact: this.videoDeviceId },
            noiseSuppression: false,
            autoGainControl: false,
            echoCancellation: false,
        };

        if (this.resolutionTag) {
            const resolution = Resolution.getFromTag(this.resolutionTag);
            if (resolution) {
                video.width = { ideal: resolution.width };
                video.height = { ideal: resolution.height };
            }
        }

        const frameRateNumber = Number(this.frameRate);
        if (!Number.isNaN(frameRateNumber)) {
            video.frameRate = {
                ideal: frameRateNumber,
            };
        }

        var audio: MediaTrackConstraints | boolean = false;

        if (this.audioDeviceId) {
            audio = {
                deviceId: { exact: this.audioDeviceId },
                sampleRate: { ideal: 48000 },
                channelCount: { ideal: 2 },
                echoCancellation: false,
                noiseSuppression: false,
                autoGainControl: false,
            };
        }

        if (this.mediaStream) {
            this.mediaStream.getTracks().forEach((track) => track.stop());
        }

        const constraints: MediaStreamConstraints = {
            video,
            audio,
        };

        this.mediaStream = null;
        this.mediaStream = await navigator.mediaDevices.getUserMedia(constraints);

        if (!this.mediaStream) {
            console.log("Fail to get new Media Stream");
            return null;
        }

        const videoInfo = Stream.getVideoInfo(this.mediaStream);
        const audioInfo = Stream.getAudioInfo(this.mediaStream);

        if (videoInfo) {
            //console.log("Video Info", videoInfo);
        }

        if (audioInfo) {
            //console.log("Audio Info", audioInfo);
        }

        return this.mediaStream;
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

    updateMediaStream(info: { videoDeviceId?: string; audioDeviceId?: string; resolutionTag?: string; frameRate?: number; audio?: boolean }) {
        if (info.videoDeviceId !== undefined) this.videoDeviceId = info.videoDeviceId;
        if (info.resolutionTag !== undefined) this.resolutionTag = info.resolutionTag;
        if (info.frameRate !== undefined) this.frameRate = info.frameRate;
        if (info.audioDeviceId !== undefined) this.audioDeviceId = info.audioDeviceId;
    }
}
