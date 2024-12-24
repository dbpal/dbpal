import { AlertColor } from '@mui/material';
import React from 'react';
import ReactDOM from 'react-dom/client';
import { NotificationContext } from './Notification';

import { TriggerDetail } from './TriggerDetail.tsx';

import {
  testProcedure,
  testQueryRespTableList,
  testTriggerDetail,
  testUDF,
} from './exampleTest.ts';
import { voidTestFunction } from './helper.ts';
import { ProcudureView, UdfView } from './UdfAndMetaView.tsx';
const root = ReactDOM.createRoot(document.getElementById('root'));

root.render(
  <NotificationContext.Provider
    value={{
      showNotification: (
        severity: AlertColor,
        title: string,
        message: string
      ) => {
        console.log(`${severity} ${title} ${message}`);
      },
    }}
  >
    <React.StrictMode>
      {/* <TriggerDetail
        reloadOP={async () => {
          return testTriggerDetail();
        }}
        language='xxx'
        disableOP={voidTestFunction}
        enableOP={voidTestFunction}
      ></TriggerDetail> */}

      <ProcudureView
        reloadOP={async () => {
          return testProcedure();
        }}
        language='xxx'
      ></ProcudureView>

      {/* <UdfView
        reloadOP={async () => {
          return testUDF();
        }}
        language='xxx'
      ></UdfView> */}
    </React.StrictMode>
  </NotificationContext.Provider>
);
