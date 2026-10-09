import { useEffect, useRef } from "preact/hooks";
import type { RefObject } from "preact";

export type AmbientLightProps = {
    videoRef: RefObject<HTMLVideoElement>;
};

// Canvas pequeno esticado para a janela. O blur é feito dentro dele (barato, poucos pixels), então
// o upscale não mostra os "quadradinhos" de uma imagem minúscula.
const WIDTH = 96;
const HEIGHT = 54;
/** Margem desenhada fora do canvas: o blur não escurece as bordas. */
const BLEED = 12;
const FRAME_MS = 1000 / 30;
/** Quanto de cada frame novo entra (o resto é o anterior): as cores mudam suavemente. */
const BLEND = 0.35;

/**
 * Brilho atrás do vídeo com as cores das bordas da imagem (aparece nas faixas do object-contain).
 * Barato de propósito: 96×54 a 30 Hz, sem `getImageData` (nada volta da GPU para a CPU) e sem
 * `filter` CSS na janela inteira.
 */
export default function AmbientLight({ videoRef }: AmbientLightProps) {
    const canvasRef = useRef<HTMLCanvasElement>(null);

    useEffect(() => {
        const ctx = canvasRef.current?.getContext("2d", { alpha: false });
        if (!ctx) return;
        ctx.imageSmoothingQuality = "high";
        ctx.filter = "blur(6px) saturate(1.4)";

        let first = true;
        let last = 0;
        let frame = 0;
        const draw = (now: number) => {
            frame = requestAnimationFrame(draw);
            const video = videoRef.current;
            if (now - last < FRAME_MS || !video || video.readyState < HTMLMediaElement.HAVE_CURRENT_DATA) return;
            last = now;
            ctx.globalAlpha = first ? 1 : BLEND;
            first = false;
            ctx.drawImage(video, -BLEED, -BLEED, WIDTH + 2 * BLEED, HEIGHT + 2 * BLEED);
        };
        frame = requestAnimationFrame(draw);

        return () => cancelAnimationFrame(frame);
    }, [videoRef]);

    return <canvas ref={canvasRef} width={WIDTH} height={HEIGHT} aria-hidden="true" className="absolute inset-0 w-full h-full opacity-90 pointer-events-none" />;
}
