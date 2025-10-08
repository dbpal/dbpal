'use strict';
import ReactDOM from 'react-dom/client';

import {
  DataSourceKind,
  RedisTableType,
  webviewWIndowInit,
} from './comp/helper.ts';
import { SeparateWindow } from './comp/MainPage.tsx';
import React from 'react';
import { AddTableView } from './comp/LeftTable.tsx';
const params = new URLSearchParams(location.search);
const table = params.get('table');
const uuid = params.get('uuid');
const kind = params.get('kind') as DataSourceKind;
const redisTableType = params.get('redisTableType') as RedisTableType;

webviewWIndowInit(false);

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>
    <SeparateWindow>
      <AddTableView
        uuid={uuid}
        table={table}
        kind={kind}
        redisTableType={redisTableType}
      ></AddTableView>
    </SeparateWindow>
  </React.StrictMode>
);
