import { ComponentChildren } from "preact";
import { useContext, useState, useEffect } from "preact/hooks";
import Tag from "../../core/components/Tag";
import Resolution from "../../core/utils/Resolution";
import Stream, { infoType } from "../../core/utils/Stream";
import { MediaStreamContext } from "../../context/MediaStreamContext";

function Icon({ src }: { src: string }) {
    return <div className="h-[60%] aspect-square mask-center mask-cover mask-no-repeat bg-gray-50" style={{ maskImage: `url(${src})` }}></div>;
}

function ButtonToggle({ children, on, off }: { children: ComponentChildren; on: () => {} | void; off: () => {} | void }) {
    const [active, setActive] = useState<boolean>(false);

    const handleClick = () => {
        setActive(!active);
    };

    useEffect(() => {
        if (!active) {
            off();
            return;
        }
        on();
    }, [active]);

    return <button onClick={handleClick}>{children}</button>;
}

function InfoBubble({ children, className }: { children: ComponentChildren; className?: string }) {
    return (
        <div className={`h-11 w-max px-3 py-1 bg-black/30 rounded-2xl flex gap-2 items-center justify-center ${className && className}`}>
            {children}
        </div>
    );
}

function StatusBarContent({ fixed, setFixed }: { fixed: boolean; setFixed: (value: boolean) => void }) {
    const ctx = useContext(MediaStreamContext);
    if (!ctx) return null;

    const { mediaStream } = ctx;

    const [info, setInfo] = useState<infoType | null>();

    useEffect(() => {
        if (!mediaStream) return;
        setInfo(Stream.getVideoInfo(mediaStream));
    }, [mediaStream]);

    useEffect(() => {
        //console.log(info);
    }, [info]);

    if (!info || !info.settings.width || !info.settings.height || !mediaStream) return null;

    const width = info.settings.width;
    const height = info.settings.height;
    const resolutionTag = Resolution.getApproximateTag({ width, height });
    const aspectRatioTag = Resolution.getAspectRatio({ width, height });
    const isCapturing = Stream.isCapturing(mediaStream);
    const fps = info.settings.frameRate;

    return (
        <>
            {/* <InfoBubble>
                <></>
            </InfoBubble> */}
            <InfoBubble>
                {isCapturing ? (
                    <>
                        <Icon src="./assets/StatusBar/play.svg" />
                        <span>Capturing</span>
                    </>
                ) : (
                    <>
                        <Icon src="./assets/StatusBar/pause.svg" />
                        <span>Not capturing</span>
                    </>
                )}
            </InfoBubble>
            <InfoBubble>
                <span>{`${info.settings.width} x ${info.settings.height}`}</span>
                {resolutionTag && <Tag text={resolutionTag.text} color={resolutionTag.color}></Tag>}
                {aspectRatioTag && <Tag text={aspectRatioTag}></Tag>}
            </InfoBubble>
            <InfoBubble>{`${fps} FPS`}</InfoBubble>
            <ButtonToggle on={() => setFixed(true)} off={() => setFixed(false)}>
                <InfoBubble className="cursor-pointer">
                    {fixed ? <Icon src="./assets/StatusBar/pin_enable.svg" /> : <Icon src="./assets/StatusBar/pin_disable.svg" />}
                </InfoBubble>
            </ButtonToggle>
        </>
    );
}

export default function StatusBar({ active }: { active: boolean }) {
    const [fixed, setFixed] = useState<boolean>(false);

    return (
        <div className="absolute inset-x-0 h-15 bottom-0">
            <div
                className={`
                        h-full w-full
                        [border-bottom:var(--border)]
                        transition-transform duration-300
                        flex gap-3 items-center justify-end
                        p-3
                        ${active || fixed ? "translate-y-0" : "translate-y-full"}
                    `}>
                <StatusBarContent fixed={fixed} setFixed={setFixed} />
            </div>
        </div>
    );
}
