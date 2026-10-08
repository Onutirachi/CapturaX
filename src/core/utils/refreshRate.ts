// Taxa de atualização do monitor, medida pelo ritmo do requestAnimationFrame (o WebView desenha
// no vsync do monitor). Usada como FPS automático da captura de tela.

const COMMON_RATES = [24, 25, 30, 48, 50, 60, 72, 75, 90, 100, 120, 144, 165, 170, 175, 180, 200, 240, 280, 360];

let cached: Promise<number> | null = null;

export function monitorRefreshRate(): Promise<number> {
    cached ??= new Promise((resolve) => {
        const times: number[] = [];
        const step = (time: number) => {
            times.push(time);
            if (times.length < 40) {
                requestAnimationFrame(step);
                return;
            }
            const intervals = times.slice(1).map((t, i) => t - times[i]).sort((a, b) => a - b);
            const median = intervals[Math.floor(intervals.length / 2)];
            const measured = 1000 / median;
            // Arredonda para a taxa comum mais próxima (143,9 → 144; 59,94 → 60).
            resolve(COMMON_RATES.reduce((best, rate) => (Math.abs(rate - measured) < Math.abs(best - measured) ? rate : best), 60));
        };
        requestAnimationFrame(step);
    });
    return cached;
}
