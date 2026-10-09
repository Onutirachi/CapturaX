import { type ComponentChildren } from "preact";
import { useEffect, useRef, useState } from "preact/hooks";

/** Velocidade da rolagem (px/s); textos maiores levam mais tempo. */
const SPEED = 70;

/**
 * Texto numa linha só que, se não couber, rola de um lado para o outro (e volta) em vez de
 * quebrar ou cortar. Se couber, fica parado.
 */
export default function ScrollText({ children }: { children: ComponentChildren }) {
    const outer = useRef<HTMLSpanElement>(null);
    const inner = useRef<HTMLSpanElement>(null);
    const [overflow, setOverflow] = useState(0);

    useEffect(() => {
        const measure = () => {
            if (!outer.current || !inner.current) return;
            setOverflow(Math.max(0, Math.ceil(inner.current.scrollWidth - outer.current.clientWidth)));
        };
        measure();
        const observer = new ResizeObserver(measure);
        if (outer.current) observer.observe(outer.current);
        if (inner.current) observer.observe(inner.current);
        return () => observer.disconnect();
    }, []);

    return (
        <span ref={outer} className="block min-w-0 overflow-hidden">
            <span
                ref={inner}
                className={`inline-block whitespace-nowrap ${overflow > 0 ? "scroll-text" : ""}`}
                style={overflow > 0 ? { "--scroll-distance": `-${overflow}px`, "--scroll-duration": `${Math.max(2.5, (overflow / SPEED) * 1.6 + 1.5)}s` } : undefined}>
                {children}
            </span>
        </span>
    );
}
