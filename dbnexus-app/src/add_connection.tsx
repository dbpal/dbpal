'use strict';
import ReactDOM from 'react-dom/client';
import './app.css';
import { windowInit } from './comp/helper.ts';
import { SeparateWindow } from './comp/MainPage.tsx';
import { WebviewWindow } from '@tauri-apps/api/webviewWindow';
import { AddConnection } from './comp/AddConnection.tsx';
import React from 'react';
import { Stack, Typography } from '@mui/material';

windowInit(false, false);
const params = new URLSearchParams(location.search);
let c = params.get('config');
const currentWebview = WebviewWindow.getCurrent();
let config = null;
let err = '';
if (c) {
  try {
    let x = JSON.parse(decodeURIComponent(c));
    config = x;
  } catch (e) {
    err = 'parse config to object error :' + c + ' :this is a internal error';
  }
}

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>
    <SeparateWindow>
      <Stack spacing={1}>
        {err && <Typography color="error">{err}</Typography>}
        <AddConnection
          handleClose={() => {
            currentWebview.close();
          }}
          config={config}
        />
      </Stack>
    </SeparateWindow>
  </React.StrictMode>
);
