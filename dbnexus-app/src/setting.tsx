import ReactDOM from "react-dom/client";
import React from "react";
import { SeparateWindow } from "./comp/MainPage.tsx";
import { ModifySetting } from "./comp/Settings.tsx";
ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <SeparateWindow>
      <ModifySetting />
    </SeparateWindow>
  </React.StrictMode>,
);
