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

import { Typography } from '@mui/material';
import { emit, listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';

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
    let unlisten = listen<any[]>(eventName, (e) => {
      if (typeof e.payload == 'string') {
        return;
      }
      handleReload();
    });
    emit(eventName, 'listen_ok');
    return () => {
      unlisten.then((e) => {
        e();
      });
    };
  }, []);

  return (
    <div style={{ height: '100%' }}>
      {resp.length == 0 && <Typography color="info"> no results </Typography>}
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
