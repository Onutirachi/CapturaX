import preact from "preact";

export default function Icon({ src, color }: { src: string; color?: string }) {
    return (
        <div
            className="h-[60%] aspect-square mask-center mask-cover mask-no-repeat bg-(--color) mask-(--src)"
            style={{ "--src": `url(${src})`, "--color": color || "var(--text)" } as preact.CSSProperties}></div>
    );
}
