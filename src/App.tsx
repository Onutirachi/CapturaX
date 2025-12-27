import Overlay from "./components/Overlay/Overlay";
import VideoStream from "./components/VideoStream/VideoStream";
import { useState } from "preact/hooks";
import { MediaStreamContext } from "./context/MediaStreamContext";

function App() {
    const [mediaStream, setMediaStream] = useState<MediaStream | null>(null);
    return (
        <MediaStreamContext.Provider value={{ mediaStream, setMediaStream }}>
            <main className="w-screen h-screen bg-gradient relative overflow-hidden font-family-segoe">
                <VideoStream />
                <Overlay />
            </main>
        </MediaStreamContext.Provider>
    );
}

export default App;
