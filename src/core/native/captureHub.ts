// Uma captura de tela/janela por vez (o Rust só faz uma), compartilhada entre gravação e
// transmissão quando as duas usam a mesma fonte.
import { NativeScreenCapture } from "./screenCapture";

type Active = {
    id: string;
    capture: Promise<NativeScreenCapture>;
    users: number;
    fps: number;
    onEnded: Set<() => void>;
};

let active: Active | null = null;

/** Erro com código, traduzido por `errorText` (`error.capture_busy`). */
const busy = () => new Error(JSON.stringify({ code: "capture_busy" }));

/**
 * Pega a trilha da tela/janela `id` (abre a captura se for a primeira). `onEnded`: a janela foi
 * fechada ou a tela sumiu. Devolve também `release`, que fecha a captura quando ninguém mais usa.
 */
export async function acquireCapture(id: string, fps: number, onEnded: () => void) {
    if (active && active.id !== id) throw busy();
    if (!active) {
        const entry: Active = { id, capture: Promise.resolve(null as never), users: 0, fps, onEnded: new Set() };
        entry.capture = NativeScreenCapture.start(id, fps, true, () => entry.onEnded.forEach((listener) => listener()));
        active = entry;
        entry.capture.catch(() => {
            if (active === entry) active = null;
        });
    }
    const entry = active;
    entry.users++;
    entry.onEnded.add(onEnded);
    let capture: NativeScreenCapture;
    try {
        capture = await entry.capture;
    } catch (error) {
        entry.users--;
        throw error;
    }
    // Quem pede mais FPS manda (o outro só recebe quadros a mais).
    if (fps > entry.fps) {
        entry.fps = fps;
        capture.setFps(fps).catch(() => {});
    }
    let released = false;
    const release = () => {
        if (released) return;
        released = true;
        entry.onEnded.delete(onEnded);
        entry.users--;
        if (entry.users <= 0 && active === entry) {
            active = null;
            capture.stop();
        }
    };
    return { track: capture.track, release };
}
