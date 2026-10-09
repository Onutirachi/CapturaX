import preact from "preact";

type TagProps = {
    text: string;
    color?: string;
};

export default function Tag({ text, color }: TagProps) {
    return (
        <div className="tag rounded-sm px-1 text-center w-max h-max box-border bg-(--color) shrink-0 whitespace-nowrap" style={{ "--color": color ? color : "#606060" } as preact.CSSProperties}>
            {text}
        </div>
    );
}
