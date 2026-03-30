import ReactDOM from "react-dom/client";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";

import { App } from "./comp/MainPage";
import { invoke } from "@tauri-apps/api/core";
import { secondConfirm } from "./comp/helper.ts";
import { saveWindowState, StateFlags } from "@tauri-apps/plugin-window-state";

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
});

if (import.meta.env.DEV) {
  invoke("page_init_callback").then(() => {
    console.info("page init callback successfully");
  });
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(<App />);

// if (import.meta.env.DEV) {
//   setTimeout(() => {
//     invoke("command_panic");
//   }, 4000);
// }
