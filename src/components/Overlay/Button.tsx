import { ComponentChildren } from "preact";

export default function Button({ children, onClick }: { children: ComponentChildren; onClick?: () => {} | void }) {
    return (
        <button
            className="p-3 w-auto min-w-16 h-11 relative rounded-xl cursor-pointer flex items-center justify-center bg-[#ff8000]"
            onClick={onClick}>
            {children}
        </button>
    );
}
