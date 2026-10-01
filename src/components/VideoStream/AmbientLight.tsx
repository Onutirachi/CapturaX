import { useEffect, useRef } from "preact/hooks";
import type { RefObject } from "preact";

export type AmbientLightProps = {
    videoRef: RefObject<HTMLVideoElement>;
};

// Canvas minúsculo esticado para a janela: a interpolação do upscale já faz o "blur".
const WIDTH = 32;
const HEIGHT = 18;
const UPDATE_MS = 100;

/**
 * Brilho atrás do vídeo com as cores das bordas da imagem (aparece nas faixas do object-contain).
 * Barato de propósito: desenha o frame num canvas 32×18 a 10 Hz, sem `getImageData` (nada volta da
 * GPU para a CPU) e sem `filter: blur`.
 */
export default function AmbientLight({ videoRef }: AmbientLightProps) {
    const canvasRef = useRef<HTMLCanvasElement>(null);

    useEffect(() => {
        const ctx = canvasRef.current?.getContext("2d", { alpha: false });
        if (!ctx) return;
        ctx.imageSmoothingQuality = "high";

        const interval = window.setInterval(() => {
            const video = videoRef.current;
            if (!video || document.hidden || video.readyState < HTMLMediaElement.HAVE_CURRENT_DATA) return;
            ctx.drawImage(video, 0, 0, WIDTH, HEIGHT);
        }, UPDATE_MS);

        return () => window.clearInterval(interval);
    }, [videoRef]);

    return <canvas ref={canvasRef} width={WIDTH} height={HEIGHT} aria-hidden="true" className="absolute inset-0 w-full h-full opacity-60 pointer-events-none" />;
}
