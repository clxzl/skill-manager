import ReactDOM from "react-dom/client";
import App from "./App";
import { applyTheme, getTheme } from "./theme";

applyTheme(getTheme());

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(<App />);
