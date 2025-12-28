import Resolution from "./Resolution";
import { SetAlertType } from "../../context/AlertsContext";
import { SetVideoStreamType } from "../../context/VideoStreamContext";

export type infoType = {
    settings: MediaTrackSettings;
    capabilities: MediaTrackCapabilities;
};

export type StreamType = { setAlert: SetAlertType; setVideoStream: SetVideoStreamType };

export default class Stream {
    private mediaStream: MediaStream = new MediaStream();
    // public videoDeviceId: string | null = null;
    // public audioDeviceId: string | null = null;
    public resolutionTag: string | null = null;
    public frameRate: number | null = null;
    private capabilities: Map<string, MediaTrackCapabilities> = new Map(); // ← CACHE
    private setAlert: SetAlertType;
    private setVideoStream: SetVideoStreamType;

    private videoTrack: MediaStreamTrack | null = null;
    private audioTrack: MediaStreamTrack | null = null;

    constructor({ setAlert, setVideoStream }: StreamType) {
        this.setAlert = setAlert;
        this.setVideoStream = setVideoStream;
        this.setVideoStream(this.mediaStream);
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

    async setVideoTrack(deviceId: string) {
        const currentTrack = this.getTrack(deviceId);

        if (!currentTrack) {
            this.stopVideoTracks(this.mediaStream);
            const tempStream = await navigator.mediaDevices.getUserMedia({ video: { deviceId: { exact: deviceId } } });
            if (!tempStream) return;

            const tempTrack = tempStream.getVideoTracks()[0];

            const capabilities = this.getCapabilities(deviceId, tempTrack);
            console.log(capabilities);
            const constraints: MediaTrackConstraints = {};

            if (capabilities.width?.max) constraints.width = { ideal: capabilities.width.max };
            if (capabilities.height?.max) constraints.height = { ideal: capabilities.height.max };
            if (capabilities.frameRate?.max) constraints.frameRate = { ideal: capabilities.frameRate.max };

            await tempTrack.applyConstraints(constraints);
            console.log(constraints);

            const track = tempTrack.clone();
            this.mediaStream.addTrack(track);

            this.setResolutionTag();
            this.setFramerate();
            this.stopTracks(tempStream);
        }

        this.setResolutionTag();
        this.setFramerate();
    }

    async setAudioTrack(deviceId: string) {
        const checkTrack = this.getTrack(deviceId);
        if (checkTrack) return;

        this.stopAudioTracks(this.mediaStream);

        const tempStream = await navigator.mediaDevices.getUserMedia({
            audio: {
                deviceId: { exact: deviceId },
                sampleRate: { ideal: 48000 },
                channelCount: { ideal: 2 },
                echoCancellation: false,
                noiseSuppression: false,
                autoGainControl: false,
            },
        });
        if (!tempStream) return null;
        const tempTrack = tempStream.getAudioTracks()[0];
        const track = tempTrack.clone();

        this.mediaStream.addTrack(track);
        this.stopTracks(tempStream);
    }

    getTrack(deviceId: string): MediaStreamTrack | null {
        const track = this.mediaStream.getTracks().find((track) => track.id === deviceId);
        if (!track) return null;
        return track;
    }

    setResolutionTag(resolutionTag: string | null = null) {
        if (resolutionTag) {
            this.resolutionTag = resolutionTag;
        }
        const videoTrack = this.mediaStream.getVideoTracks()[0];
        if (!videoTrack) return;

        const resolution = Resolution.getFromTag(resolutionTag);
        if (!resolution) return;

        videoTrack.applyConstraints({
            width: { exact: resolution.width },
            height: { exact: resolution.height },
        });
    }

    setFramerate(frameRate: number | null = null) {
        if (!frameRate) return;
        this.frameRate = frameRate;
        if (Number.isFinite(this.frameRate) && this.frameRate > 0) {
            const videoTrack = this.mediaStream.getVideoTracks()[0];
            if (!videoTrack) return;

            videoTrack.applyConstraints({
                frameRate: { exact: this.frameRate },
            });
        }
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

    handleError(text: string) {
        console.error(text);
        this.setAlert({ text: text, color: "darkred", icon: "./assets/Alerts/error.svg" });
    }

    handleWarning(text: string) {
        console.warn(text);
        this.setAlert({ text: text, color: "goldenrod", icon: "./assets/Alerts/warning.svg" });
    }

    // async startVideoDevice(deviceId: string): Promise<MediaStreamTrack | null> {
    //     const mediaStream = await navigator.mediaDevices.getUserMedia({
    //         video: { deviceId: { exact: deviceId } },
    //     });

    //     const track = mediaStream.getVideoTracks()[0];

    //     let capabilities = this.capabilitiesCache.get(deviceId);
    //     if (!capabilities) {
    //         capabilities = track.getCapabilities() as MediaTrackCapabilities;
    //         this.capabilitiesCache.set(deviceId, capabilities);
    //     }

    //     track.applyConstraints({
    //         width: { ideal: capabilities.width?.max },
    //         height: { ideal: capabilities.height?.max },
    //         frameRate: { ideal: capabilities.frameRate?.max },
    //         noiseSuppression: false,
    //         autoGainControl: false,
    //         echoCancellation: false,
    //     });

    //     return track;
    // }

    // async getMediaStream(): Promise<MediaStream | null> {
    //     this.setAlert(null);
    //     if (!this.videoDeviceId) {
    //         this.handleError("No device id");
    //         return null;
    //     }

    //     const track = await this.startVideoDevice(this.videoDeviceId);
    //     if (!track) return null;

    //     const resolution = Resolution.getFromTag(this.resolutionTag);
    //     if (resolution) {
    //         track.applyConstraints({
    //             width: { ideal: resolution.width },
    //             height: { ideal: resolution.height },
    //         });
    //     }

    //     const frameRateNumber = Number(this.frameRate);
    //     if (!Number.isNaN(frameRateNumber) && frameRateNumber > 0) {
    //         track.applyConstraints({
    //             frameRate: { ideal: frameRateNumber },
    //         });
    //     }

    //     var audio: MediaTrackConstraints | boolean = false;

    //     if (this.audioDeviceId) {
    //         audio = {
    //             deviceId: { exact: this.audioDeviceId },
    //             sampleRate: { ideal: 48000 },
    //             channelCount: { ideal: 2 },
    //             echoCancellation: false,
    //             noiseSuppression: false,
    //             autoGainControl: false,
    //         };
    //     }
    // }

    // async getMediaStream(): Promise<MediaStream | null> {
    //     this.setAlert(null);
    //     if (!this.videoDeviceId) {
    //         this.handleError("No device id");
    //         return null;
    //     }

    //     // Usa cache se já tiver as capabilities deste device
    //     let capabilities = this.capabilitiesCache.get(this.videoDeviceId);
    //     if (!capabilities) {
    //         capabilities = await this.getCapabilities(this.videoDeviceId);
    //         this.capabilitiesCache.set(this.videoDeviceId, capabilities);
    //     }

    //     const video: MediaTrackConstraints = {
    //         deviceId: { exact: this.videoDeviceId },
    //         noiseSuppression: false,
    //         autoGainControl: false,
    //         echoCancellation: false,
    //     };

    //     const resolution = Resolution.getFromTag(this.resolutionTag);
    //     if (resolution) {
    //         video.width = { ideal: resolution.width };
    //         video.height = { ideal: resolution.height };
    //     } else {
    //         video.width = { ideal: capabilities.width?.max };
    //         video.height = { ideal: capabilities.height?.max };
    //     }

    //     const frameRateNumber = Number(this.frameRate);
    //     if (!Number.isNaN(frameRateNumber) && frameRateNumber > 0) {
    //         video.frameRate = { ideal: frameRateNumber };
    //     } else {
    //         video.frameRate = { ideal: capabilities.frameRate?.max };
    //     }

    //     var audio: MediaTrackConstraints | boolean = false;

    //     if (this.audioDeviceId) {
    //         audio = {
    //             deviceId: { exact: this.audioDeviceId },
    //             sampleRate: { ideal: 48000 },
    //             channelCount: { ideal: 2 },
    //             echoCancellation: false,
    //             noiseSuppression: false,
    //             autoGainControl: false,
    //         };
    //     }

    //     this.stopTracks();

    //     const constraints: MediaStreamConstraints = { video, audio };

    //     this.mediaStream = null;

    //     try {
    //         this.mediaStream = await navigator.mediaDevices.getUserMedia(constraints);
    //     } catch (error) {
    //         this.handleError("Resolution/Framerate not supported");
    //         console.error(error);
    //         return null;
    //     }

    //     console.debug({ constraits: constraints, ...Stream.getVideoInfo(this.mediaStream) });
    //     this.debug(constraints, this.mediaStream);

    //     return this.mediaStream;
    // }

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

    // async getCapabilities(deviceId: string): Promise<MediaTrackCapabilities> {
    //     const stream = await navigator.mediaDevices.getUserMedia({
    //         video: { deviceId: { exact: deviceId } },
    //     });

    //     const track = stream.getVideoTracks()[0];
    //     const caps = track.getCapabilities() as MediaTrackCapabilities;
    //     track.stop();
    //     return caps;
    // }

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
