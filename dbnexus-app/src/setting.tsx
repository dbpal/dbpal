import ReactDOM from 'react-dom/client';
import './app.css';
import React from 'react';
import { windowInit } from './comp/helper.ts';
import { SeparateWindow } from './comp/MainPage.tsx';
import { ModifySetting } from './comp/Settings.tsx';

windowInit(false);

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>
    <SeparateWindow>
      <ModifySetting />
    </SeparateWindow>
  </React.StrictMode>
);
