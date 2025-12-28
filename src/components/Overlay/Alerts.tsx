import InfoBubble from "./InfoBubble";
import Icon from "./Icon";
import { AlertsContext } from "../../context/AlertsContext";
import { useContext } from "preact/hooks";

export default function Alerts() {
    const ctx = useContext(AlertsContext);
    if (!ctx) return null;

    const { alert } = ctx; // Access the alert function from context
    if (!alert) return null;

    return (
        <InfoBubble>
            {alert.text}
            <Icon src={alert.icon} color={alert.color}></Icon>
        </InfoBubble>
    );
}
