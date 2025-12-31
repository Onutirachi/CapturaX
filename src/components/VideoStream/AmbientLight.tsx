import { useEffect, useRef } from "preact/hooks";
import type { RefObject } from "preact";

// Tipos do componente
export type AmbientLightProps = {
    videoRef: RefObject<HTMLVideoElement>;
    intensity?: number;
    blur?: string;
    updateInterval?: number;
};

// Componente AmbientLight - retorna apenas 1 div
export default function AmbientLight({ videoRef, intensity = 0.6, blur = "blur-3xl", updateInterval = 16 }: AmbientLightProps) {
    const canvasRef = useRef<HTMLCanvasElement>(null);
    const bgRef = useRef<HTMLDivElement>(null);
    const rafRef = useRef<number | null>(null);
    const lastUpdateRef = useRef<number>(0);

    useEffect(() => {
        const video = videoRef?.current;
        const canvas = canvasRef.current;
        const bg = bgRef.current;

        if (!video || !canvas || !bg) return;

        const ctx = canvas.getContext("2d", { willReadFrequently: true });
        if (!ctx) return;

        canvas.width = 32;
        canvas.height = 18;

        const updateAmbilight = (timestamp: number) => {
            if (video.paused || video.ended) {
                rafRef.current = requestAnimationFrame(updateAmbilight);
                return;
            }

            if (timestamp - lastUpdateRef.current < updateInterval) {
                rafRef.current = requestAnimationFrame(updateAmbilight);
                return;
            }
            lastUpdateRef.current = timestamp;

            ctx.drawImage(video, 0, 0, canvas.width, canvas.height);

            const imageData = ctx.getImageData(0, 0, canvas.width, canvas.height);
            const data = imageData.data;

            let r = 0,
                g = 0,
                b = 0;
            const step = 4;
            let count = 0;

            for (let i = 0; i < data.length; i += step * 4) {
                r += data[i];
                g += data[i + 1];
                b += data[i + 2];
                count++;
            }

            r = Math.floor(r / count);
            g = Math.floor(g / count);
            b = Math.floor(b / count);

            bg.style.background = `radial-gradient(ellipse at center, 
        rgba(${r}, ${g}, ${b}, ${intensity}) 0%, 
        rgba(${r}, ${g}, ${b}, ${intensity * 0.7}) 40%, 
        rgba(${r}, ${g}, ${b}, ${intensity * 0.2}) 70%,
        transparent 100%)`;

            rafRef.current = requestAnimationFrame(updateAmbilight);
        };

        rafRef.current = requestAnimationFrame(updateAmbilight);

        return () => {
            if (rafRef.current) {
                cancelAnimationFrame(rafRef.current);
            }
        };
    }, [videoRef, intensity, updateInterval]);

    return (
        <div ref={bgRef} className={`absolute inset-0 ${blur} scale-110 transition-opacity duration-300 pointer-events-none`} style={{ zIndex: -1 }}>
            <canvas ref={canvasRef} className="hidden" aria-hidden="true" />
        </div>
    );
}
