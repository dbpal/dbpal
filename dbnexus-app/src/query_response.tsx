'use strict';
import ReactDOM from 'react-dom/client';

import {
  DetachQueyRespViewPara,
  SomeTempStore,
  webviewWIndowInit,
} from './comp/helper.ts';
import { SeparateWindow } from './comp/MainPage.tsx';
import React, { useEffect, useState } from 'react';
import { MultiQueryResp } from './comp/QueryResp.tsx';
import { emit, listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import { Typography } from '@mui/material';
import { listenOK, newQueryIDName, reloadData } from './comp/SqlQuery.tsx';

webviewWIndowInit(false);
let { uuid, queryId, eventName } = SomeTempStore.get<DetachQueyRespViewPara>();

function QueryResponse() {
  const [resp, setResp] = useState([]);
  const handleReload = () => {
    invoke<any>('get_query_cache', {
      name: uuid,
      queryId,
    }).then((data) => {
      setResp(data);
    });
  };
  useEffect(() => {
    let unlisten = listen<'listen_ok' | object | string>(eventName, (e) => {
      if (e.payload == listenOK) {
        // I send this message nothing to do here.
        return;
      }
      if (e.payload === reloadData) {
        handleReload();
        return;
      }
      if (typeof e.payload == 'object' && e.payload[newQueryIDName]) {
        queryId = e.payload[newQueryIDName];
        console.log('send new query id:', queryId);
        return;
      }
      console.error('unhandled event:', e.payload);
    });
    emit(eventName, listenOK);
    return () => {
      unlisten.then((e) => {
        e();
      });
    };
  }, []);

  return (
    <div style={{ height: '100%' }}>
      {resp.length == 0 && <Typography color="info"> No Data </Typography>}
      <MultiQueryResp data={resp}></MultiQueryResp>
    </div>
  );
}

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>
    <SeparateWindow>
      <QueryResponse />
    </SeparateWindow>
  </React.StrictMode>
);
