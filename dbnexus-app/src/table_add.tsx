'use strict';
import ReactDOM from 'react-dom/client';

import {
  NewWindowTableParameter,
  SomeTempStore,
  webviewWIndowInit,
} from './comp/helper.ts';
import { SeparateWindow } from './comp/MainPage.tsx';
import React from 'react';
import { AddTableView } from './comp/LeftTable.tsx';

const { uuid, table, kind, redisTableType } =
  SomeTempStore.get<NewWindowTableParameter>();

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
