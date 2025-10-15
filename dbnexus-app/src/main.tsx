'use strict';
import ReactDOM from 'react-dom/client';
import { WebviewWindow } from '@tauri-apps/api/webviewWindow';

import { App } from './comp/MainPage';
import { invoke } from '@tauri-apps/api/core';
import React from 'react';
import {
  secondConfirm,
  setUpDatabase,
  webviewWIndowInit,
} from './comp/helper.ts';

await setUpDatabase();
webviewWIndowInit(true);
const currentWebview = WebviewWindow.getCurrent();

currentWebview.onCloseRequested(async (event) => {
  const confirm = await secondConfirm({
    title: 'close the window',
    content: 'exit the app',
    confirmText: 'close',
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
  invoke('page_init_callback')
    .then(() => {
      console.log('page init callback successfully');
    })
    .catch((e) => {
      console.log('call page init failed', e);
    });
}

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>
    <App></App>
  </React.StrictMode>
);
