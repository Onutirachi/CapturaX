import { ComponentChildren } from "preact";

type ButtonProps = {
    children: ComponentChildren;
    title: string;
    onClick: () => void;
};

export default function Button({ children, title, onClick }: ButtonProps) {
    return (
        <button onClick={onClick} title={title} className="flex items-center justify-center h-full w-11 rounded-md">
            {children}
        </button>
    );
}
