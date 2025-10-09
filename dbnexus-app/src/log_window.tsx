'use strict';
import ReactDOM from 'react-dom/client';

import { webviewWIndowInit } from './comp/helper.ts';
import { SeparateWindow } from './comp/MainPage.tsx';
import React from 'react';
import { LogFile } from './comp/LogFile.tsx';
import { WebviewWindow } from '@tauri-apps/api/webviewWindow';
import { invoke } from '@tauri-apps/api/core';

webviewWIndowInit(false);
ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>
    <SeparateWindow>
      <LogFile></LogFile>
    </SeparateWindow>
  </React.StrictMode>
);

const currentWebView = await WebviewWindow.getCurrent();

currentWebView.onCloseRequested(async () => {
  try {
    await invoke('close_log_channel');
  } catch (e) {
    console.log(e);
    return;
  }
  console.log('close log channel success');
});
