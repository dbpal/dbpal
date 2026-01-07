import ReactDOM from "react-dom/client";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";

import { App } from "./comp/MainPage";
import { invoke } from "@tauri-apps/api/core";
import { appStore, LoggerLevel, secondConfirm } from "./comp/helper.ts";

const currentWebview = WebviewWindow.getCurrent();

currentWebview.onCloseRequested(async event => {
  const confirm = await secondConfirm({
    title: "close the window",
    content: "exit the app",
    confirmText: "close",
  });
  if (!confirm) {
    event.preventDefault();
    return;
  }
  const views = await WebviewWindow.getAll();
  for (const view of views) {
    if (view.label != currentWebview.label) {
      // close all webview
      await view.close();
    }
    // no need close current view just not calling `event.preventDefault()`
    //  await currentWebview.close();
  }
});

if (import.meta.env.DEV) {
  invoke("page_init_callback").then(() => {
    console.info("page init callback successfully");
  });
}

function setLogLevel(level: string) {
  invoke("set_log_level", { level: level });
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(<App />);
appStore.onKeyChange<string>(LoggerLevel.key, level => {
  setLogLevel(level);
});

appStore.get<string>(LoggerLevel.key).then(level => {
  setLogLevel(level);
});

// if (import.meta.env.DEV) {
//   setTimeout(() => {
//     invoke("command_panic");
//   }, 4000);
// }
