import { render } from "preact";
import "./globals.css";
import App from "./App";
import WindowBar from "./components/WindowBar/WindowBar";

render(
    <>
        <WindowBar />
        <App />
    </>,
    document.getElementById("root")!
);
