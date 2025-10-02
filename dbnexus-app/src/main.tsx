'use strict';
import ReactDOM from 'react-dom/client';
import { WebviewWindow } from '@tauri-apps/api/webviewWindow';
import './app.css';
import { MainPage } from './comp/MainPage';
import { invoke } from '@tauri-apps/api/core';
import React from 'react';
import { secondConfirm, windowInit } from './comp/helper';

// console.log('before error');
// xxxx + 1;
// console.log('after error');

await windowInit(true, true);

const currentWebview = await WebviewWindow.getCurrent();
currentWebview.onCloseRequested(async (event) => {
  let confirm = await secondConfirm({
    title: 'close the window',
    content: 'are u sure to exit the app',
    confirmText: 'close',
  });
  if (!confirm) {
    event.preventDefault();
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
    <MainPage></MainPage>
  </React.StrictMode>
);
