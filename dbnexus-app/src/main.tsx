import "@fontsource/pt-mono/400.css";

import ReactDOM from "react-dom/client";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";

import { App } from "./comp/MainPage";
import { secondConfirm } from "./comp/helper.ts";
import { saveWindowState, StateFlags } from "@tauri-apps/plugin-window-state";
import { commands } from "./comp/bindings.ts";

saveWindowState(StateFlags.ALL);

const currentWebview = WebviewWindow.getCurrent();

currentWebview.onCloseRequested(async (event) => {
  const confirm = await secondConfirm({
    title: "close the window",
    content: "exit the app",
    confirmText: "close",
  });
  if (!confirm) {
    event.preventDefault();
    return;
  }
  for (const view of await WebviewWindow.getAll()) {
    if (view.label != currentWebview.label) {
      await view.close();
    }
  }
});

if (import.meta.env.DEV) {
  commands.pageInitCallback();
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(<App />);
