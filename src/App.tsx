import Overlay from "./components/Overlay/Overlay";
import VideoStream from "./components/VideoStream/VideoStream";
import { VideoStreamProvider } from "./context/VideoStreamContext";
import { AlertsProvider } from "./context/AlertsContext";
import { FrameStatsProvider } from "./context/FrameStatsContext";
import { NativeProvider } from "./context/NativeContext";
import PermissionGate from "./components/PermissionGate/PermissionGate";

function App() {
    return (
        <main className="w-full h-full bg-gradient relative overflow-hidden font-family-segoe">
            <VideoStreamProvider>
                <AlertsProvider>
                    <NativeProvider>
                        <FrameStatsProvider>
                            <VideoStream />
                            <PermissionGate>
                                <Overlay />
                            </PermissionGate>
                        </FrameStatsProvider>
                    </NativeProvider>
                </AlertsProvider>
            </VideoStreamProvider>
        </main>
    );
}

export default App;
