'use strict';
import ReactDOM from 'react-dom/client';

import { SomeTempStore } from './comp/helper.ts';
import { SeparateWindow } from './comp/MainPage.tsx';
import React from 'react';
import { ConnConfigInfo } from './comp/CommonComp2.tsx';

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>
    <SeparateWindow>
      <div style={{ height: '100%' }}>
        <ConnConfigInfo uuid={SomeTempStore.get<string>()}></ConnConfigInfo>
      </div>
    </SeparateWindow>
  </React.StrictMode>
);
