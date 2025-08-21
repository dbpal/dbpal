'use strict';

import ReactDOM from 'react-dom/client';

import './comp/MainPage';
import MainPage from './comp/MainPage';
import { debugBuild } from './comp/debug';
import { invoke } from '@tauri-apps/api/core';
import React from 'react';
import * as tauriwindow from '@tauri-apps/api/window';
import { platform } from '@tauri-apps/plugin-os';
import { secondConfirm } from './comp/helper';

if (['windows', 'linux'].includes(platform())) {
  document.addEventListener('keydown', function (event) {
    if (event.key === 'F11') {
      const window = tauriwindow.getCurrent();
      window.isFullscreen().then((isFull) => {
        window.setFullscreen(!isFull);
      });
    }
  });
}

let appWindow = tauriwindow.getCurrent();
if (appWindow) {
  appWindow.onCloseRequested(async (event) => {
    let confirm = await secondConfirm({
      title: 'close the window',
      content: 'are u sure to exit the app',
      confirmText: 'close',
    });
    if (!confirm) {
      event.preventDefault();
    }
  });
}

if (!debugBuild) {
  document.addEventListener('contextmenu', function (event) {
    event.preventDefault();
  });

  document.addEventListener('keydown', function (event) {
    // Prevent F5 or Ctrl+R (Windows/Linux) and Command+R (Mac) from refreshing the page
    if (
      event.key === 'F5' ||
      (event.ctrlKey && event.key === 'r') ||
      (event.metaKey && event.key === 'r')
    ) {
      event.preventDefault();
    }
  });
}

if (debugBuild) {
  window.addEventListener('load', (_event) => {
    invoke('page_init_callback');
  });
}

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>
    <MainPage></MainPage>
  </React.StrictMode>
);

window.prompt();
