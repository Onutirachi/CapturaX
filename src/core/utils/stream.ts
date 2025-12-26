import { RESOLUTION_LIST } from "./constants";

type MediaStremSettings = {
    deviceId: string;
    resolution?: string;
    frameRate?: number;
    audio?: boolean;
};

export default class Stream {
    private mediaStream: MediaStream | null = null;

    constructor() {}

    async checkPermissions() {
        const status = await navigator.permissions.query({ name: "camera" });
        if (status.state === "granted") {
            return true;
        }
        return false;
    }

    getResolution(tag: string | null) {
        if (!tag) return null;
        const resolution = RESOLUTION_LIST.find((res) => res.tag === tag);
        if (!resolution) return null;
        return { width: resolution.width, height: resolution.height };
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

    async getDevices(): Promise<MediaDeviceInfo[]> {
        const deviceList = await navigator.mediaDevices.enumerateDevices();
        const videoDevices = deviceList.filter((device) => device.kind === "videoinput");
        return videoDevices;
    }

    getMediaStream(): MediaStream | null {
        return this.mediaStream;
    }

    async setMediaStream(settings: MediaStremSettings | null = null) {
        if (!settings) return;

        const deviceId = settings.deviceId || null;
        const resolutionTag = settings.resolution || null;
        const frameRate = settings.frameRate || null;
        const audio = settings.audio || true;

        if (!deviceId) throw new Error("No device ID");

        if (this.mediaStream) {
            this.mediaStream.getTracks().forEach((track) => track.stop());
        }

        // Construir constraints baseadas nas seleções

        const video: MediaTrackConstraints = {
            deviceId: { exact: deviceId },
        };

        const resolution = this.getResolution(resolutionTag);
        if (resolution) {
            video.width = { ideal: resolution.width };
            video.height = { ideal: resolution.height };
        }

        const frameRateNumber = Number(frameRate);
        if (Number.isNaN(frameRateNumber)) {
            video.frameRate = { ideal: Number(frameRateNumber) };
        }

        const constraints: MediaStreamConstraints = {
            video,
            audio: audio,
        };

        this.mediaStream = await navigator.mediaDevices.getUserMedia(constraints);
        return this.mediaStream;
    }
}
