import { AlertColor } from '@mui/material';
import React from 'react';
import ReactDOM from 'react-dom/client';
import { NotificationContext } from './Notification';

import { SqlQuery } from './SqlQuery.tsx';
import {
  testMultiQueryResp,
  testQueryRespTableList,
  testQueryLogs,
} from './exampleTest.ts';
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
      <SqlQuery
        language="sql"
        queryOP={async (_text) => {
          return testMultiQueryResp(3);
        }}
        loadQueryLog={async () => {
          return testQueryLogs(220);
        }}
      ></SqlQuery>
    </React.StrictMode>
  </NotificationContext.Provider>
);
