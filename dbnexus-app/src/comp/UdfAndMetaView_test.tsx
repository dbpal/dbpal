import { AlertColor } from '@mui/material';
import React from 'react';
import ReactDOM from 'react-dom/client';
import { NotificationContext } from './Notification.tsx';

import { testProcedure } from './exampleTest.ts';

import { DefAndMetasView } from './UdfAndMetaView.tsx';

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
      <DefAndMetasView
        reloadOP={async () => {
          return testProcedure();
        }}
        uuid="xxx"
      ></DefAndMetasView>

      {/* <UdfView
        reloadOP={async () => {
          return testUDF();
        }}
        language='xxx'
      ></UdfView> */}
    </React.StrictMode>
  </NotificationContext.Provider>
);
