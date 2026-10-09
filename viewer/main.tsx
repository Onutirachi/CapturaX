import { render } from "preact";
import "./viewer.css";
import Viewer from "./Viewer";
import { initLanguages, setLanguage } from "../src/i18n";

setLanguage(initLanguages(navigator.languages, null));
render(<Viewer />, document.getElementById("root")!);
