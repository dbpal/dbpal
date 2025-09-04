'use strict';

import ReactDOM from 'react-dom/client';

import './app.css';
import './comp/MainPage';
import MainPage from './comp/MainPage';
import { debugBuild } from './comp/debug';
import { invoke } from '@tauri-apps/api/core';
import React from 'react';
import * as tauriwindow from '@tauri-apps/api/window';
import { platform } from '@tauri-apps/plugin-os';
import {
  appStore,
  AppStoreKey,
  secondConfirm,
  UserSettings,
  windowFocusChangeEvent,
} from './comp/helper';
import { emit, listen } from '@tauri-apps/api/event';

import { WebviewWindow } from '@tauri-apps/api/webviewWindow';

async function setZoom(out: boolean) {
  const webview = WebviewWindow.getCurrent(); // Replace "main" with your webview label
  let zoom = await appStore.get<number>(AppStoreKey.Zoom);
  if (!zoom) {
    zoom = 1;
  }
  const min = 0.5;
  const max = 1.5;
  if (out) {
    zoom += 0.1;
    if (zoom > max) {
      zoom = max;
    }
  } else {
    zoom -= 0.1;
    if (zoom < min) {
      zoom = min;
    }
  }
  webview.setZoom(zoom);
  await appStore.set(AppStoreKey.Zoom, zoom);
}

listen('zoom_in', async (_) => {
  await setZoom(false);
});

listen('zoom_out', async (_) => {
  await setZoom(true);
});
listen('zoom_reset', async (_) => {
  const webview = WebviewWindow.getCurrent();
  webview.setZoom(1);
  appStore.set(AppStoreKey.Zoom, 1);
});

appStore.get<number>(AppStoreKey.Zoom).then(async (zoom) => {
  if (zoom) {
    const webview = WebviewWindow.getCurrent();
    await webview.setZoom(zoom);
  }
});

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

appWindow.onFocusChanged((x) => {
  emit(windowFocusChangeEvent, x.payload);
});

if (!debugBuild) {
  appWindow.theme().then(async (theme) => {
    if (!theme) {
      return;
    }
    let set = await UserSettings.get();
    set.theme = theme;
    appStore.set(AppStoreKey.UserSettings, set);
  });
}

appWindow.onThemeChanged(async (msg) => {
  let set = await UserSettings.get();
  set.theme = msg.payload;
  appStore.set(AppStoreKey.UserSettings, set);
});

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
