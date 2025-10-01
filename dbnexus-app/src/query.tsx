import ReactDOM from 'react-dom/client';
import './app.css';
import React, { useContext, useEffect, useState } from 'react';
import {
  connTypeToDataSourceKind,
  DataSourceKind,
  secondConfirm,
  SetCloseConfirmTextContext,
  windowInit,
} from './comp/helper.ts';
import { initMonacoLanguage } from './comp/Common.ts';
import { SeparateWindow } from './comp/MainPage.tsx';
import { makeQueryComponet } from './comp/Left.tsx';
import { NotificationContext } from './comp/Notification.tsx';
import { WebviewWindow } from '@tauri-apps/api/webviewWindow';

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
  const { showNotification } = useContext(NotificationContext);
  const [closeConfirm, setCloseConfirm] = useState('');
  useEffect(() => {
    cc = closeConfirm;
  }, [closeConfirm]);
  let x = makeQueryComponet(kind, uuid, showNotification);
  return (
    <SetCloseConfirmTextContext
      value={(msg) => {
        setCloseConfirm(msg);
      }}
    >
      {x}
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
