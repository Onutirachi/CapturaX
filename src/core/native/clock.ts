// Alinha o relógio do WebView com o relógio QPC do Rust (o mesmo que carimba o áudio).
import { native } from "./api";

/** Tempo absoluto de alta resolução, igual na janela e em Workers. */
export function nowAbsUs() {
    return (performance.timeOrigin + performance.now()) * 1000;
}

let offsetUs: number | null = null;

/** Mede `qpc - nowAbsUs()` pegando a ida-e-volta mais rápida de algumas tentativas. */
export async function calibrateClock() {
    let best = Infinity;
    for (let i = 0; i < 7; i++) {
        const before = nowAbsUs();
        const qpc = await native.clockNow();
        const after = nowAbsUs();
        if (after - before < best) {
            best = after - before;
            offsetUs = qpc - (before + after) / 2;
        }
    }
    return offsetUs!;
}

export async function absToQpcUs(absUs: number) {
    if (offsetUs === null) await calibrateClock();
    return absUs + offsetUs!;
}
