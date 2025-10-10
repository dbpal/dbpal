'use strict';
import ReactDOM from 'react-dom/client';
import { SomeTempStore, webviewWIndowInit } from './comp/helper.ts';
import { SeparateWindow } from './comp/MainPage.tsx';
import { WebviewWindow } from '@tauri-apps/api/webviewWindow';
import { AddConnection } from './comp/AddConnection.tsx';
import React from 'react';
import { Stack } from '@mui/material';

webviewWIndowInit(false);

const currentWebview = WebviewWindow.getCurrent();
ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>
    <SeparateWindow>
      <Stack spacing={1}>
        <AddConnection
          handleClose={() => {
            currentWebview.close();
          }}
          config={SomeTempStore.get()}
        />
      </Stack>
    </SeparateWindow>
  </React.StrictMode>
);
