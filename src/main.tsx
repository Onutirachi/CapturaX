import { render } from "preact";
import "./globals.css";
import App from "./App";
import WindowBar from "./components/WindowBar/WindowBar";
import { FullscreenProvider } from "./context/FullscreenContext";

render(
    <>
        <FullscreenProvider>
            <>
                <WindowBar />
                <App />
            </>
        </FullscreenProvider>
    </>,
    document.getElementById("root")!
);
