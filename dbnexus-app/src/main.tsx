"use strict";
import ReactDOM from "react-dom/client";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";

import { App } from "./comp/MainPage";
import { invoke } from "@tauri-apps/api/core";
import React from "react";
import { LLMConfig, secondConfirm } from "./comp/helper.ts";
import { createChatAPI } from "./comp/Common.ts";

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

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App></App>
  </React.StrictMode>,
);

const api = createChatAPI({
  baseUrl: "http://localhost:11434/v1",
  // apiKey: "sk-egR0EUYn0EzVybl7D9JkUFx2ksnD62sZPQ8d0J0yJgPEiAWt",
  model: "qwen2.5-coder:1.5b",
} as LLMConfig);

console.time("call stream");
let s = await api.stream("what is sqlite, with a word count of 5,000");
console.timeEnd("call stream");
console.time("read stream");
let wholeText = "";
for await (const msg of s) {
  wholeText += msg.content;
  console.log(msg.content);
}
console.timeEnd("read stream");
console.log("!!!!!!!!!!!!!!", wholeText);
