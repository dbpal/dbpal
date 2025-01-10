import { AlertColor, Stack, TextField } from '@mui/material';
import React, { useEffect, useState } from 'react';
import ReactDOM from 'react-dom/client';
import { NotificationContext } from './Notification';

import { SqlTableView } from './SqlTableView.tsx';
import { testQueryRespTableList } from './exampleTest.ts';
import { voidTestFunction } from './helper.ts';
import types from './ApiStruct.ts';
import { DataGrid } from '@mui/x-data-grid';
import { v4 as uuidv4 } from 'uuid';
const root = ReactDOM.createRoot(document.getElementById('root'));

function XXX(props: { reloadOP: () => Promise<types.MuiDataTableListResp> }) {
  let { reloadOP } = props;

  const [resp, setResp] = useState<types.MuiDataTableListResp | null>(null);
  useEffect(() => {
    reloadOP().then((e) => {
      e.data.data.forEach((e) => {
        e['id'] = uuidv4();
      });
      setResp(e);
    });
  }, []);
  const [value, setValue] = useState('');

  return (
    <Stack>
      <TextField
        value={value}
        onChange={(e) => {
          setValue(e.target.value);
        }}
        size="small"
      ></TextField>
      {resp && (
        <DataGrid
          getRowId={(e) => e['id']}
          columns={resp.data.fields}
          rows={resp.data.data}
          rowHeight={28}
          columnHeaderHeight={33}
        ></DataGrid>
      )}
    </Stack>
  );
}

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
      <XXX
        reloadOP={async () => {
          return testQueryRespTableList(10, 2000);
        }}
      ></XXX>
    </React.StrictMode>
  </NotificationContext.Provider>
);
