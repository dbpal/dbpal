import ReactDOM from 'react-dom/client';
import './app.css';
import { windowInit } from './comp/helper.ts';
import { SeparateWindow } from './comp/MainPage.tsx';
import { WebviewWindow } from '@tauri-apps/api/webviewWindow';
import { AddConnection } from './comp/AddConnection.tsx';
import React from 'react';

windowInit(false, false);
const params = new URLSearchParams(location.search);
let config = params.get('config');
const currentWebview = WebviewWindow.getCurrent();

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>
    <SeparateWindow>
      <AddConnection
        handleClose={() => {
          currentWebview.close();
        }}
        config={config && JSON.parse(config)}
      />
    </SeparateWindow>
  </React.StrictMode>
);
