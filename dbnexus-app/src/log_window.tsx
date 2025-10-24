'use strict';
import ReactDOM from 'react-dom/client';

import { SeparateWindow } from './comp/MainPage.tsx';
import React from 'react';
import { LogStream } from './comp/LogStream.tsx';
import { WebviewWindow } from '@tauri-apps/api/webviewWindow';
import { invoke } from '@tauri-apps/api/core';

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>
    <SeparateWindow>
      <LogStream></LogStream>
    </SeparateWindow>
  </React.StrictMode>
);

const currentWebView = await WebviewWindow.getCurrent();

currentWebView.onCloseRequested(async () => {
  try {
    await invoke('close_log_channel');
  } catch (e) {
    console.error(e);
    return;
  }
  console.info('close log channel success');
});
