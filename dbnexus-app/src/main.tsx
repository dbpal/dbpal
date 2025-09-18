'use strict';
console.log('!!!!!!!!!!!!!');
import ReactDOM from 'react-dom/client';

import './app.css';
import './comp/MainPage';
import MainPage from './comp/MainPage';
import { invoke } from '@tauri-apps/api/core';
import React from 'react';
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
import { console } from 'inspector';
const currentWebview = WebviewWindow.getCurrent();
async function setZoom(out: boolean) {
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
  currentWebview.setZoom(zoom);
  await appStore.set(AppStoreKey.Zoom, zoom);
}

listen('zoom_in', async (_) => {
  await setZoom(false);
});

listen('zoom_out', async (_) => {
  await setZoom(true);
});
listen('zoom_reset', async (_) => {
  currentWebview.setZoom(1);
  appStore.set(AppStoreKey.Zoom, 1);
});

appStore.get<number>(AppStoreKey.Zoom).then(async (zoom) => {
  if (zoom) {
    await currentWebview.setZoom(zoom);
  }
});

if (['windows', 'linux'].includes(platform())) {
  document.addEventListener('keydown', function (event) {
    if (event.key === 'F11') {
      currentWebview.isFullscreen().then((isFull) => {
        currentWebview.setFullscreen(!isFull);
      });
    }
  });
}

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

currentWebview.onFocusChanged((x) => {
  emit(windowFocusChangeEvent, x.payload);
});

if (!import.meta.env.DEV) {
  currentWebview.theme().then(async (theme) => {
    if (!theme) {
      return;
    }
    let set = await UserSettings.get();
    set.theme = theme;
    appStore.set(AppStoreKey.UserSettings, set);
  });
}

currentWebview.onThemeChanged(async (msg) => {
  let set = await UserSettings.get();
  set.theme = msg.payload;
  appStore.set(AppStoreKey.UserSettings, set);
});

if (!import.meta.env.DEV) {
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

if (import.meta.env.DEV) {
  window.addEventListener('load', (_event) => {
    invoke('page_init_callback');
  });
}

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>
    <MainPage></MainPage>
  </React.StrictMode>
);
