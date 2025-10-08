'use strict';
import ReactDOM from 'react-dom/client';
import React, { useEffect, useState } from 'react';
import {
  connTypeToDataSourceKind,
  DataSourceKind,
  secondConfirm,
  SetCloseConfirmTextContext,
  SomeTempStore,
  webviewWIndowInit,
} from './comp/helper.ts';
import { initMonacoLanguage } from './comp/Common.ts';
import { SeparateWindow } from './comp/MainPage.tsx';
import { WebviewWindow } from '@tauri-apps/api/webviewWindow';
import { AllKindQuery } from './comp/Left.tsx';
import { Typography } from '@mui/material';

webviewWIndowInit(false);
const params = new URLSearchParams(location.search);

const uuid = params.get('uuid');
const kind = connTypeToDataSourceKind(params.get('kind'));
let err;
await initMonacoLanguage(uuid, kind, (msg) => {
  err = msg;
});

// close confirm message
// when in transaction prevent user to close the window
let cc;

let sqlData = SomeTempStore.get();

function Query(props: {
  uuid: string;
  kind: DataSourceKind;
  sql?: string;
  favoriteName?: string;
}) {
  const { uuid, kind, sql, favoriteName } = props;
  const [closeConfirm, setCloseConfirm] = useState('');
  useEffect(() => {
    cc = closeConfirm;
  }, [closeConfirm]);
  return (
    <SetCloseConfirmTextContext
      value={(msg) => {
        setCloseConfirm(msg);
      }}
    >
      {err && <Typography color="error">{err}</Typography>}
      <AllKindQuery
        uuid={uuid}
        kind={kind}
        value={sql}
        favoriteName={favoriteName}
      />
    </SetCloseConfirmTextContext>
  );
}

const currentWebview = await WebviewWindow.getCurrent();
currentWebview.onCloseRequested(async (event) => {
  if (!cc) {
    return;
  }
  let confirm = await secondConfirm({
    title: 'close the window',
    content: cc,
    confirmText: 'close',
  });
  if (!confirm) {
    event.preventDefault();
  }
});

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>
    <SeparateWindow>
      <Query
        uuid={uuid}
        kind={kind}
        sql={sqlData && sqlData.sql}
        favoriteName={sqlData && sqlData.name}
      ></Query>
    </SeparateWindow>
  </React.StrictMode>
);
