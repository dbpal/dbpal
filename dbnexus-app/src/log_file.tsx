'use strict';
import ReactDOM from 'react-dom/client';

import { webviewWIndowInit } from './comp/helper.ts';
import { SeparateWindow } from './comp/MainPage.tsx';
import React from 'react';
import { LogFile } from './comp/LogFile.tsx';

webviewWIndowInit(false);
ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>
    <SeparateWindow>
      <LogFile></LogFile>
    </SeparateWindow>
  </React.StrictMode>
);
