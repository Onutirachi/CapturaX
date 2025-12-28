import Overlay from "./components/Overlay/Overlay";
import VideoStream from "./components/VideoStream/VideoStream";
import { MediaStreamProvider } from "./context/MediaStreamContext";

function App() {
    return (
        <MediaStreamProvider>
            <main className="w-full h-full bg-gradient relative overflow-hidden font-family-segoe">
                <VideoStream />
                <Overlay />
            </main>
        </MediaStreamProvider>
    );
}

export default App;
