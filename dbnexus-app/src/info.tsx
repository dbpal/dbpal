'use strict';
import ReactDOM from 'react-dom/client';

import { webviewWIndowInit } from './comp/helper.ts';
import { SeparateWindow } from './comp/MainPage.tsx';
import React from 'react';
import { ConnConfigInfo } from './comp/CommonComp2.tsx';
const params = new URLSearchParams(location.search);
const uuid = params.get('uuid');

webviewWIndowInit(false);
ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>
    <SeparateWindow>
      <ConnConfigInfo uuid={uuid}></ConnConfigInfo>
    </SeparateWindow>
  </React.StrictMode>
);
