import { render } from "preact";
import "./Globals.css";
import App from "./App";
import WindowBar from "./components/WindowBar/WindowBar";
import { FullscreenProvider } from "./context/FullscreenContext";
import { native } from "./core/native/api";
import { initLanguages, LanguageProvider } from "./i18n";

async function start() {
    // Fora do app (navegador comum) não há Rust: segue o idioma do navegador.
    const [system, config] = await Promise.all([native.systemLanguages().catch(() => [...navigator.languages]), native.getConfig().catch(() => null)]);
    const language = initLanguages(system, config?.language ?? null);
    render(
        <LanguageProvider initial={language}>
            <FullscreenProvider>
                <>
                    <WindowBar />
                    <App />
                </>
            </FullscreenProvider>
        </LanguageProvider>,
        document.getElementById("root")!
    );
}

start();
