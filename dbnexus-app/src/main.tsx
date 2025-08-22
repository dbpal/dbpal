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
import { emit } from '@tauri-apps/api/event';

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

appWindow.theme().then(async (theme) => {
  if (!theme) {
    return;
  }
  let set = await UserSettings.get();
  set.theme = theme;
  appStore.set(AppStoreKey.UserSetttings, set);
});
appWindow.onThemeChanged(async (msg) => {
  let set = await UserSettings.get();
  set.theme = msg.payload;
  appStore.set(AppStoreKey.UserSetttings, set);
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
