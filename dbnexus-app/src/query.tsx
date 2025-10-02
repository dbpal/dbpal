import ReactDOM from 'react-dom/client';
import './app.css';
import React, { useEffect, useState } from 'react';
import {
  connTypeToDataSourceKind,
  DataSourceKind,
  secondConfirm,
  SetCloseConfirmTextContext,
  windowInit,
} from './comp/helper.ts';
import { initMonacoLanguage } from './comp/Common.ts';
import { SeparateWindow } from './comp/MainPage.tsx';
import { WebviewWindow } from '@tauri-apps/api/webviewWindow';
import { AllKindQuery } from './comp/Left.tsx';

windowInit(false, false);
const params = new URLSearchParams(location.search);

const uuid = params.get('uuid');
const kind = connTypeToDataSourceKind(params.get('kind'));

await initMonacoLanguage(uuid, kind, (msg) => {
  alert(msg);
});

// close confirm message
// when in transaction prevent user to close the window
let cc;

function Query(props: { uuid: string; kind: DataSourceKind }) {
  const { uuid, kind } = props;
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
      <AllKindQuery uuid={uuid} kind={kind} />
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
      <Query uuid={uuid} kind={kind}></Query>
    </SeparateWindow>
  </React.StrictMode>
);
