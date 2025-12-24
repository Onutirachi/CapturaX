import { render } from "preact";
import App from "./App";
import "./globals.css";
import WindowBar from "./components/WindowBar/WindowBar";

render(
    <>
        <WindowBar />
        <App />
    </>,
    document.getElementById("root")!
);
