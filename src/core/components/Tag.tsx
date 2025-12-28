import preact from "preact";

type TagProps = {
    text: string;
    color?: string;
};

export default function Tag({ text, color }: TagProps) {
    return (
        <div className="rounded-sm px-1 w-max h-max box-border bg-(--color)" style={{ "--color": color ? color : "#606060" } as preact.CSSProperties}>
            {text}
        </div>
    );
}
