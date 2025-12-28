import { ComponentChildren } from "preact";

export default function InfoBubble({ children, className }: { children: ComponentChildren; className?: string }) {
    return (
        <div className={`h-11 w-max px-3 py-1 bg-black/30 rounded-2xl flex gap-2 items-center justify-center ${className && className}`}>
            {children}
        </div>
    );
}
