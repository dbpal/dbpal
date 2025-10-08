'use strict';
import ReactDOM from 'react-dom/client';
import React from 'react';
import { webviewWIndowInit } from './comp/helper.ts';
import { SeparateWindow } from './comp/MainPage.tsx';
import { BackendCrash } from './comp/BackendCrash.tsx';
import { invoke } from '@tauri-apps/api/core';

webviewWIndowInit(false);
const params = new URLSearchParams(location.search);
const id = parseInt(params.get('id'));
let msg;

try {
  msg = await invoke<string>('get_backend_crash', { id });
} catch (e) {
  console.log('error', e);
}

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>
    <SeparateWindow>
      <BackendCrash msg={msg}></BackendCrash>
    </SeparateWindow>
  </React.StrictMode>
);
