import React from 'react';
import ReactDOM from 'react-dom/client';

const root = ReactDOM.createRoot(document.getElementById('root'));

import { testQueryLogs } from './exampleTest.ts';
import { QueryLog } from './QueryLog.tsx';
import { NotificationContext } from './Notification.tsx';
import { AlertColor } from '@mui/material';

root.render(
  <NotificationContext.Provider
    value={{
      showNotification: (
        severity: AlertColor,
        title: string,
        message: string
      ) => {
        console.log(`${severity} ${title} ${message}`);
        alert(`${severity} ${title} ${message}`);
      },
    }}
  >
    <React.StrictMode>
      <QueryLog logs={testQueryLogs(10)} language='xxx'></QueryLog>
    </React.StrictMode>
  </NotificationContext.Provider>
);
