'use strict';
import ReactDOM from 'react-dom/client';
import React, { useEffect, useState } from 'react';
import {
  DataSourceKind,
  FavoriteSql,
  secondConfirm,
  SetCloseConfirmTextContext,
  SomeTempStore,
  webviewWIndowInit,
} from './comp/helper.ts';
import { SeparateWindow } from './comp/MainPage.tsx';
import { WebviewWindow } from '@tauri-apps/api/webviewWindow';
import { AllKindQuery } from './comp/Left.tsx';

webviewWIndowInit(false);

const sqlData = SomeTempStore.get<FavoriteSql>();

// close confirm message
// when in transaction prevent user to close the window
let exitConfirmMessage: string;

function Query(props: {
  uuid: string;
  kind: DataSourceKind;
  sql?: string;
  favoriteName?: string;
}) {
  const { uuid, kind, sql, favoriteName } = props;
  const [closeConfirm, setCloseConfirm] = useState('');
  useEffect(() => {
    exitConfirmMessage = closeConfirm;
  }, [closeConfirm]);
  return (
    <SetCloseConfirmTextContext
      value={(msg) => {
        setCloseConfirm(msg);
      }}
    >
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
  if (!exitConfirmMessage) {
    return;
  }
  const confirm = await secondConfirm({
    title: 'close the window',
    content: exitConfirmMessage,
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
        uuid={sqlData.uuid}
        kind={sqlData.kind}
        sql={sqlData.sql}
        favoriteName={sqlData.name}
      ></Query>
    </SeparateWindow>
  </React.StrictMode>
);
