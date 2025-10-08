'use strict';
import ReactDOM from 'react-dom/client';
import React from 'react';
import { webviewWIndowInit } from './comp/helper.ts';
import { SeparateWindow } from './comp/MainPage.tsx';
import { ModifySetting } from './comp/Settings.tsx';

webviewWIndowInit(false);

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>
    <SeparateWindow>
      <ModifySetting />
    </SeparateWindow>
  </React.StrictMode>
);
