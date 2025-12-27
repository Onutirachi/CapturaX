import { RESOLUTION_LIST } from "./constants";

export default class Stream {
    private mediaStream: MediaStream | null = null;

    private deviceId: string | null = null;
    private resolutionTag: string | null = null;
    private frameRate: number | null = null;
    private audio: boolean | null = null;

    constructor() {}

    async checkPermissions() {
        const status = await navigator.permissions.query({ name: "camera" });
        if (status.state === "granted") {
            return true;
        }
        return false;
    }

    getResolutionFromTag(tag: string | null) {
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

    async getMediaStream(): Promise<MediaStream | null> {
        if (!this.deviceId) {
            console.log("No device id");
            return null;
        }

        const video: MediaTrackConstraints = {
            deviceId: { exact: this.deviceId },
        };

        const resolution = this.getResolutionFromTag(this.resolutionTag);
        if (resolution) {
            video.width = { exact: resolution.width };
            video.height = { exact: resolution.height };
        }

        const frameRateNumber = Number(this.frameRate);
        if (!Number.isNaN(frameRateNumber)) {
            video.frameRate = { ideal: frameRateNumber };
        }

        if (this.mediaStream) {
            this.mediaStream.getTracks().forEach((track) => track.stop());
        }

        const constraints: MediaStreamConstraints = {
            video,
            audio: this.audio ? true : false,
        };

        this.mediaStream = await navigator.mediaDevices.getUserMedia(constraints);
        return this.mediaStream;
    }

    updateMediaStream(info: { deviceId?: string; resolutionTag?: string; frameRate?: number; audio?: boolean }) {
        if (info.deviceId !== undefined) this.deviceId = info.deviceId;
        if (info.resolutionTag !== undefined) this.resolutionTag = info.resolutionTag;
        if (info.frameRate !== undefined) this.frameRate = info.frameRate;
        if (info.audio !== undefined) this.audio = info.audio;

        const mediaStreamInfo = {
            deviceId: this.deviceId,
            resolutionTag: this.resolutionTag,
            frameRate: this.frameRate,
            audio: this.audio,
        };
        console.log(mediaStreamInfo);
        console.log(info);
    }
}
