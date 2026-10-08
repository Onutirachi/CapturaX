import ToolbarContent from "./ToolbarContent";
import { type HoverHandlers } from "./Overlay";

type ToolbarProps = { active: boolean; hover: HoverHandlers };

export default function Toolbar({ active, hover }: ToolbarProps) {
    return (
        <>
            <div className="absolute top-0 left-0 inset-x-0 h-max min-h-25 z-5" {...hover}>
                <div
                    className={`
                        flex items-center gap-5 flex-wrap
                        p-5
                        h-full w-full
                        [border-bottom:var(--border)]
                        transition-transform duration-300
                        bg-darker/80 backdrop-blur-xs
                    
                        ${active ? "translate-y-0" : "-translate-y-full"}
                    `}>
                    <ToolbarContent />
                </div>
            </div>
        </>
    );
}
