'use strict';
import ReactDOM from 'react-dom/client';

import { windowInit } from './comp/helper.ts';
import { SeparateWindow } from './comp/MainPage.tsx';
import React, { useEffect, useState } from 'react';

import { MultiQueryResp } from './comp/QueryResp.tsx';
import { emit, listen } from '@tauri-apps/api/event';
import { Typography } from '@mui/material';

windowInit(false, false);
const params = new URLSearchParams(location.search);

const event_name = params.get('event_name');

function QueryResponse() {
  const [resp, setResp] = useState([]);
  useEffect(() => {
    let unlisten = listen<any[]>(event_name, (e) => {
      if (typeof e.payload == 'string') {
        return;
      }
      setResp(e.payload);
    });
    //
    emit(event_name, 'listen_ok');
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
