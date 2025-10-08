'use strict';
import ReactDOM from 'react-dom/client';

import { SomeTempStore, webviewWIndowInit } from './comp/helper.ts';
import { SeparateWindow } from './comp/MainPage.tsx';
import React from 'react';
import { ConnConfigInfo } from './comp/CommonComp2.tsx';

webviewWIndowInit(false);
ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>
    <SeparateWindow>
      <ConnConfigInfo uuid={SomeTempStore.get<string>()}></ConnConfigInfo>
    </SeparateWindow>
  </React.StrictMode>
);
