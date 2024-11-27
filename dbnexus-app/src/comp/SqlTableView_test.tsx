import { AlertColor } from '@mui/material';
import React from 'react';
import ReactDOM from 'react-dom/client';
import { NotificationContext } from './Notification';

import { SqlTableView } from './SqlTableView.tsx';
import { testQueryRespTableList } from './exampleTest.ts';
import { voidTestFunction } from './helper.ts';
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
      <SqlTableView
        reloadOP={async () => {
          return testQueryRespTableList(10, 20);
        }}
        //@ts-ignore
        updateOP={async () => {
          try {
            voidTestFunction();
          } catch (e) {
            throw e;
          }
        }}
        deleteOP={voidTestFunction}
        addOP={voidTestFunction}
      ></SqlTableView>
    </React.StrictMode>
  </NotificationContext.Provider>
);
