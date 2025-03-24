import ReactDOM from 'react-dom/client';

import './comp/MainPage';
import MainPage from './comp/MainPage';
import { debugBuild } from './comp/debug';
import { invoke } from '@tauri-apps/api/core';
import React from 'react';
import * as tauriwindow from '@tauri-apps/api/window';
import { platform } from '@tauri-apps/plugin-os';

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

// call every time maybe use refresh this app some what.
window.addEventListener('load', (_event) => {
  invoke('page_init_callback');
});

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>
    <MainPage></MainPage>
  </React.StrictMode>
);
