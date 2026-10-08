// Worker de codificação: lê frames da trilha de vídeo, reduz resolução/FPS se preciso e
// codifica na GPU (VideoEncoder com prefer-hardware). Fora da thread da interface para não
// atrasar o preview.
import { encoderConfig, type EncoderSettings } from "./presets";

export type ToWorker =
    | { type: "start"; readable: ReadableStream<VideoFrame>; settings: EncoderSettings }
    | { type: "source"; readable: ReadableStream<VideoFrame> }
    | { type: "stop" };

export type FromWorker =
    | { type: "config"; codec: string; description: ArrayBuffer; width: number; height: number }
    | { type: "chunk"; data: ArrayBuffer; absUs: number; key: boolean }
    | { type: "error"; message: string }
    | { type: "stopped"; dropped: number };

const post = (message: FromWorker, transfer: Transferable[] = []) => (self as unknown as Worker).postMessage(message, transfer);
const nowAbsUs = () => (performance.timeOrigin + performance.now()) * 1000;

let settings: EncoderSettings;
let encoder: VideoEncoder | null = null;
let reader: ReadableStreamDefaultReader<VideoFrame> | null = null;
let canvas: OffscreenCanvas | null = null;
let context: OffscreenCanvasRenderingContext2D | null = null;
let configSent = false;
let stopped = false;
let dropped = 0;

// Tempo dos frames: `frame.timestamp` só vale como diferença; ancoramos no relógio absoluto
// quando o primeiro frame de cada fonte chega.
let anchorOffset: number | null = null;
let lastAbsUs = -Infinity;
/** Os frames vão ao encoder com tempo relativo (começa em 0); somamos de volta na saída. */
let baseAbsUs: number | null = null;
let lastKeptUs = -Infinity;
let lastKeyUs = -Infinity;

function toBytes(source: AllowSharedBufferSource): ArrayBuffer {
    if (source instanceof ArrayBuffer) return source.slice(0);
    if (ArrayBuffer.isView(source)) return source.buffer.slice(source.byteOffset, source.byteOffset + source.byteLength) as ArrayBuffer;
    return new Uint8Array(source as SharedArrayBuffer).slice().buffer;
}

function createEncoder() {
    encoder = new VideoEncoder({
        output: (chunk, metadata) => {
            const description = metadata?.decoderConfig?.description;
            if (!configSent && description) {
                configSent = true;
                const copy = toBytes(description);
                post({ type: "config", codec: settings.codec, description: copy, width: settings.width, height: settings.height }, [copy]);
            }
            const data = new ArrayBuffer(chunk.byteLength);
            chunk.copyTo(data);
            post({ type: "chunk", data, absUs: chunk.timestamp + baseAbsUs!, key: chunk.type === "key" }, [data]);
        },
        error: (error) => post({ type: "error", message: error.message }),
    });
    encoder.configure(encoderConfig(settings));
}

function scaled(frame: VideoFrame, timestamp: number): VideoFrame {
    if (frame.displayWidth === settings.width && frame.displayHeight === settings.height) {
        return new VideoFrame(frame, { timestamp });
    }
    if (!canvas) {
        canvas = new OffscreenCanvas(settings.width, settings.height);
        context = canvas.getContext("2d", { alpha: false })!;
        context.imageSmoothingQuality = "high";
    }
    // Mantém a proporção (barras pretas se a fonte mudar de formato no meio).
    const scale = Math.min(settings.width / frame.displayWidth, settings.height / frame.displayHeight);
    const w = frame.displayWidth * scale;
    const h = frame.displayHeight * scale;
    context!.fillStyle = "#000";
    context!.fillRect(0, 0, settings.width, settings.height);
    context!.drawImage(frame, (settings.width - w) / 2, (settings.height - h) / 2, w, h);
    return new VideoFrame(canvas, { timestamp });
}

function handle(frame: VideoFrame) {
    if (stopped || !encoder || encoder.state !== "configured") {
        frame.close();
        return;
    }
    anchorOffset ??= nowAbsUs() - frame.timestamp;
    // Sempre crescente, mesmo se a fonte trocar e o relógio dela começar de outro ponto.
    const absUs = Math.max(frame.timestamp + anchorOffset, lastAbsUs + 1);
    lastAbsUs = absUs;

    // Limite de FPS (com folga para relógios que oscilam um pouco).
    if (absUs - lastKeptUs < (1_000_000 / settings.fps) * 0.85) {
        frame.close();
        return;
    }
    // GPU não está dando conta: descarta em vez de acumular atraso.
    if (encoder.encodeQueueSize > 4) {
        dropped++;
        frame.close();
        return;
    }
    lastKeptUs = absUs;

    const key = absUs - lastKeyUs >= settings.keyframeSecs * 1_000_000;
    if (key) lastKeyUs = absUs;
    baseAbsUs ??= absUs;
    const input = scaled(frame, absUs - baseAbsUs);
    frame.close();
    encoder.encode(input, { keyFrame: key });
    input.close();
}

async function pump(readable: ReadableStream<VideoFrame>) {
    const current = readable.getReader();
    reader = current;
    anchorOffset = null;
    try {
        while (!stopped && reader === current) {
            const { value, done } = await current.read();
            if (done) break;
            handle(value);
        }
    } catch {
        // trilha encerrada
    } finally {
        current.releaseLock();
    }
}

self.onmessage = async (event: MessageEvent<ToWorker>) => {
    const message = event.data;
    switch (message.type) {
        case "start":
            settings = message.settings;
            try {
                createEncoder();
            } catch (error) {
                post({ type: "error", message: String(error) });
                return;
            }
            pump(message.readable);
            break;
        case "source":
            await reader?.cancel().catch(() => {});
            pump(message.readable);
            break;
        case "stop":
            stopped = true;
            await reader?.cancel().catch(() => {});
            if (encoder && encoder.state === "configured") {
                await encoder.flush().catch(() => {});
                encoder.close();
            }
            post({ type: "stopped", dropped });
            break;
    }
};
