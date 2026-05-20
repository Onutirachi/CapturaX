import { useContext, useState, useEffect } from "preact/hooks";
import Tag from "../../core/components/Tag";
import Resolution from "../../core/utils/Resolution";
import { VideoStreamContext } from "../../context/VideoStreamContext";

import Icon from "./Icon";
import ButtonToggle from "../../core/components/ButtonToggle";
import InfoBubble from "./InfoBubble";
import Alerts from "./Alerts";

export default function StatusBarContent({ fixed, setFixed }: { fixed: boolean; setFixed: (value: boolean) => void }) {
    const vsctx = useContext(VideoStreamContext);
    if (!vsctx) return null;
    const { videoTrack } = vsctx;

    if (!videoTrack) {
        return (
            <>
                <Alerts />
                <ButtonToggle on={() => setFixed(true)} off={() => setFixed(false)} storeState="statusBarFixed">
                    <InfoBubble className="cursor-pointer">
                        {fixed ? <Icon src="./assets/StatusBar/pin_enable.svg" /> : <Icon src="./assets/StatusBar/pin_disable.svg" />}
                    </InfoBubble>
                </ButtonToggle>
            </>
        );
    }

    const settings = videoTrack.getSettings();
    const width = settings.width || 0;
    const height = settings.height || 0;
    const resolutionTag = Resolution.getApproximateTag({ width, height });
    const aspectRatioTag = Resolution.getAspectRatio({ width, height });
    const isCapturing = videoTrack.readyState === "live" ? true : false;
    const fps = settings.frameRate;

    return (
        <>
            <Alerts />
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
                <span>{`${settings.width} x ${settings.height}`}</span>
                {resolutionTag && <Tag text={resolutionTag.text} color={resolutionTag.color}></Tag>}
                {aspectRatioTag && <Tag text={aspectRatioTag}></Tag>}
            </InfoBubble>
            <InfoBubble>{`${fps} FPS`}</InfoBubble>
            <ButtonToggle on={() => setFixed(true)} off={() => setFixed(false)} storeState="statusBarFixed">
                <InfoBubble className="cursor-pointer">
                    {fixed ? <Icon src="./assets/StatusBar/pin_enable.svg" /> : <Icon src="./assets/StatusBar/pin_disable.svg" />}
                </InfoBubble>
            </ButtonToggle>
        </>
    );
}
