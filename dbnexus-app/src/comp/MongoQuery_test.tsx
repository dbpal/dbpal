import React from 'react';
import ReactDOM from 'react-dom/client';

import { MongoQuery } from './MongoQuery';
import { AlertColor } from '@mui/material';
import { NotificationContext } from './Notification';

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
        alert(`${severity} ${title} ${message}`);
      },
    }}
  >
    <React.StrictMode>
      <MongoQuery></MongoQuery>
    </React.StrictMode>
  </NotificationContext.Provider>
);
