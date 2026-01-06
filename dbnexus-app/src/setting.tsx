import ReactDOM from "react-dom/client";
import { SeparateWindow } from "./comp/MainPage.tsx";
import { ModifySetting } from "./comp/Settings.tsx";
ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <SeparateWindow>
    <ModifySetting />
  </SeparateWindow>,
);
