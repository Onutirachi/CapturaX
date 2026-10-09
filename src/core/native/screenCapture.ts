// Captura de tela/janela feita no Rust (Windows Graphics Capture) virando uma trilha de vídeo comum.
// Os quadros chegam por memória compartilhada do WebView2 (ver src-tauri/src/capture/bridge.rs):
// cada buffer tem um cabeçalho de HEADER bytes; o primeiro u32 = 1 quer dizer "quadro esperando".
// Copiamos o quadro para um VideoFrame e zeramos o cabeçalho para o Rust reutilizar o buffer.
import { native } from "./api";

const HEADER = 64;

type BufferMessage = { capturax: "buffer"; generation: number; slot: number };
type FrameMessage = { capturax: "frame"; generation: number; slot: number; width: number; height: number; timestamp: number };
type EndedMessage = { capturax: "ended" };
type Message = BufferMessage | FrameMessage | EndedMessage;

const isMessage = (data: unknown): data is Message => typeof data === "object" && data !== null && "capturax" in data;

/** Tamanho/FPS das trilhas geradas aqui (o `getSettings()` delas não traz esses dados). */
const generatedSettings = new WeakMap<MediaStreamTrack, MediaTrackSettings>();

/** `track.getSettings()` que também funciona para a captura nativa. */
export function trackSettings(track: MediaStreamTrack): MediaTrackSettings {
    return { ...track.getSettings(), ...generatedSettings.get(track) };
}

export const nativeCaptureAvailable = () => typeof MediaStreamTrackGenerator !== "undefined" && !!window.chrome?.webview;

/** Uma captura por vez (o Rust também só faz uma). */
export class NativeScreenCapture {
    /** A mais recente: parar uma antiga não pode derrubar a do Rust, que já é outra. */
    private static current: NativeScreenCapture | null = null;
    private buffers = new Map<string, ArrayBuffer>();
    private generator: MediaStreamTrackGenerator;
    private writer: WritableStreamDefaultWriter<VideoFrame>;
    private closed = false;

    private onBuffer = (event: WebView2SharedBufferEvent) => {
        const data = event.additionalData;
        if (!isMessage(data) || data.capturax !== "buffer") return;
        // Gerações antigas não voltam mais.
        for (const key of this.buffers.keys()) if (!key.startsWith(`${data.generation}:`)) this.buffers.delete(key);
        this.buffers.set(`${data.generation}:${data.slot}`, event.getBuffer());
    };

    private onMessage = (event: WebView2MessageEvent) => {
        const data = event.data;
        if (!isMessage(data)) return;
        if (data.capturax === "ended") {
            this.stop();
            this.onEnded();
            return;
        }
        if (data.capturax !== "frame" || this.closed) return;
        const buffer = this.buffers.get(`${data.generation}:${data.slot}`);
        if (!buffer || buffer.byteLength === 0) return;
        const size = data.width * data.height * 4;
        const settings = generatedSettings.get(this.track);
        if (settings && (settings.width !== data.width || settings.height !== data.height)) generatedSettings.set(this.track, { ...settings, width: data.width, height: data.height });
        let frame: VideoFrame | null = null;
        try {
            // O construtor copia os bytes: o buffer pode ser liberado logo em seguida.
            frame = new VideoFrame(new Uint8Array(buffer, HEADER, size), { format: "BGRX", codedWidth: data.width, codedHeight: data.height, timestamp: data.timestamp });
        } catch (error) {
            console.error("Screen capture frame dropped", error);
        }
        new Uint32Array(buffer, 0, 1)[0] = 0;
        // Preview atrasado (aba em segundo plano etc.): descarta em vez de acumular.
        if (frame && (this.writer.desiredSize ?? 1) <= 0) frame.close();
        else if (frame) this.writer.write(frame).catch(() => frame?.close());
    };

    readonly track: MediaStreamTrack;

    private constructor(private onEnded: () => void) {
        this.generator = new MediaStreamTrackGenerator({ kind: "video" });
        this.writer = this.generator.writable.getWriter();
        this.track = this.generator;
        window.chrome!.webview!.addEventListener("sharedbufferreceived", this.onBuffer);
        window.chrome!.webview!.addEventListener("message", this.onMessage);
    }

    /** `id` vem de `native.captureSources()`. `onEnded`: a janela fechou ou a tela sumiu. */
    static async start(id: string, fps: number, cursor: boolean, onEnded: () => void) {
        const capture = new NativeScreenCapture(onEnded);
        NativeScreenCapture.current = capture;
        try {
            const { width, height } = await native.captureStart(id, fps, cursor);
            generatedSettings.set(capture.track, { width, height, frameRate: fps });
        } catch (error) {
            capture.stop();
            throw error;
        }
        return capture;
    }

    setFps(fps: number) {
        generatedSettings.set(this.track, { ...generatedSettings.get(this.track), frameRate: fps });
        return native.captureSetFps(fps);
    }

    stop() {
        if (this.closed) return;
        this.closed = true;
        window.chrome?.webview?.removeEventListener("sharedbufferreceived", this.onBuffer as (event: never) => void);
        window.chrome?.webview?.removeEventListener("message", this.onMessage as (event: never) => void);
        this.buffers.clear();
        this.writer.close().catch(() => {});
        this.generator.stop();
        if (NativeScreenCapture.current === this) {
            NativeScreenCapture.current = null;
            native.captureStop().catch(() => {});
        }
    }
}
