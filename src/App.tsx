import Overlay from "./components/Overlay/Overlay";
import VideoStream from "./components/VideoStream/VideoStream";
import { VideoStreamProvider } from "./context/VideoStreamContext";
import { AlertsProvider } from "./context/AlertsContext";
import PermissionGate from "./components/PermissionGate/PermissionGate";

function App() {
    return (
        <main className="w-full h-full bg-gradient relative overflow-hidden font-family-segoe">
            <VideoStreamProvider>
                <VideoStream />
                <AlertsProvider>
                    <PermissionGate>
                        <Overlay />
                    </PermissionGate>
                </AlertsProvider>
            </VideoStreamProvider>
        </main>
    );
}

export default App;
