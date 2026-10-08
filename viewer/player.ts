// Player de baixa latência: lê a resposta contínua de `live.mp4` (fMP4 em pedaços de ~100 ms)
// e alimenta um MediaSource, mantendo o vídeo colado no ao vivo.

type PlayerEvents = {
    /** Primeiro pedaço tocável chegou (hora de dar play). */
    onReady: () => void;
    /** A conexão terminou (transmissão encerrada ou atraso grande): reconectar. */
    onEnded: () => void;
    onFull: () => void;
};

/** Atraso alvo atrás da borda ao vivo, em segundos. */
const TARGET_LAG = 0.3;
/** Acima disso, acelera um pouco para recuperar. */
const CATCH_UP_LAG = 0.8;
/** Acima disso, pula direto para perto da borda. */
const JUMP_LAG = 2;
/** Buffer antigo mantido atrás da posição atual (o resto é descartado). */
const KEEP_BEHIND = 10;

export class LowLatencyPlayer {
    private mediaSource = new MediaSource();
    private sourceBuffer: SourceBuffer | null = null;
    private queue: Uint8Array[] = [];
    private abort = new AbortController();
    private closed = false;
    private started = false;
    private timer: number;
    private objectUrl: string;

    constructor(
        private video: HTMLVideoElement,
        private url: string,
        private codecs: string,
        private events: PlayerEvents
    ) {
        this.objectUrl = URL.createObjectURL(this.mediaSource);
        video.src = this.objectUrl;
        this.mediaSource.addEventListener("sourceopen", () => this.run(), { once: true });
        this.timer = window.setInterval(() => this.keepLive(), 200);
    }

    static supports(codecs: string) {
        return typeof MediaSource !== "undefined" && MediaSource.isTypeSupported(`video/mp4; codecs="${codecs}"`);
    }

    private async run() {
        try {
            this.sourceBuffer = this.mediaSource.addSourceBuffer(`video/mp4; codecs="${this.codecs}"`);
            this.sourceBuffer.addEventListener("updateend", () => this.flush());
            const response = await fetch(this.url, { signal: this.abort.signal, cache: "no-store" });
            if (response.status === 503) {
                this.events.onFull();
                return;
            }
            if (!response.ok || !response.body) throw new Error(`HTTP ${response.status}`);
            const reader = response.body.getReader();
            while (!this.closed) {
                const { value, done } = await reader.read();
                if (done) break;
                this.queue.push(value);
                this.flush();
            }
        } catch {
            // conexão caiu; reconecta abaixo
        }
        if (!this.closed) this.events.onEnded();
    }

    /** Junta o que chegou e entrega num append só (o SourceBuffer aceita um por vez). */
    private flush() {
        const buffer = this.sourceBuffer;
        if (!buffer || buffer.updating || this.queue.length === 0 || this.mediaSource.readyState !== "open") return;
        const total = this.queue.reduce((sum, part) => sum + part.length, 0);
        const chunk = new Uint8Array(total);
        let offset = 0;
        for (const part of this.queue) {
            chunk.set(part, offset);
            offset += part.length;
        }
        this.queue = [];
        try {
            buffer.appendBuffer(chunk);
        } catch {
            // Buffer cheio: descarta o que já passou e tenta de novo.
            this.queue.unshift(chunk);
            this.trim(true);
        }
    }

    private trim(force = false) {
        const buffer = this.sourceBuffer;
        const ranges = this.video.buffered;
        if (!buffer || buffer.updating || ranges.length === 0) return;
        const start = ranges.start(0);
        const until = this.video.currentTime - (force ? 1 : KEEP_BEHIND);
        if (until - start > (force ? 0 : 5)) buffer.remove(start, until);
    }

    /** Mantém a reprodução perto da borda ao vivo. */
    private keepLive() {
        const video = this.video;
        const ranges = video.buffered;
        if (ranges.length === 0) return;
        const end = ranges.end(ranges.length - 1);

        if (!this.started) {
            this.started = true;
            video.currentTime = Math.max(ranges.start(ranges.length - 1), end - TARGET_LAG);
            this.events.onReady();
            return;
        }

        const lag = end - video.currentTime;
        if (lag > JUMP_LAG || video.currentTime < ranges.start(ranges.length - 1)) {
            video.currentTime = end - TARGET_LAG;
        }
        video.playbackRate = lag > CATCH_UP_LAG ? 1.1 : 1;
        this.trim();
    }

    destroy() {
        this.closed = true;
        this.abort.abort();
        window.clearInterval(this.timer);
        try {
            if (this.mediaSource.readyState === "open") this.mediaSource.endOfStream();
        } catch {
            // já encerrado
        }
        this.video.removeAttribute("src");
        this.video.load();
        URL.revokeObjectURL(this.objectUrl);
    }
}
