import ReactDOM from 'react-dom/client';

import apis from './ApiStruct.ts';
import { Alert, Box, Tab, Tabs } from '@mui/material';

import { testExampleQueryResp } from './exampleTest.ts';
import { generateRandomString } from './helper.ts';
import { DataGrid } from '@mui/x-data-grid';
import React from 'react';

function QueryResp(props: {
  data: apis.MuiDataGridData;
  order: string;
  orderBy: string;
  reloadSort: (order: string, orderBy: string) => void;
}) {
  let { data } = props;
  var rowid = 0;
  data.fields.forEach((e) => {
    // e.filterable = true;
  });
  const getRowId = () => {
    return rowid++;
  };
  return (
    <DataGrid
      columns={data.fields}
      getRowId={getRowId}
      rows={data.data}
      filterMode='client' // 使用客户端过滤
    ></DataGrid>
  );
}

/*
  multi resp have this rust structure
[
  {
    "Err": "this a error"
  },
  {
    "Ok": {
      "fields": [],
      "data": []
    }
  }
]
*/
function testExampleMultiQueryResp(length: number = 3) {
  let ret = [];
  for (let i = 0; i < length; i++) {
    if (Math.random() > 0.66) {
      ret.push({ Err: `This is random error.` });
    } else {
      ret.push({ Ok: testExampleQueryResp() });
    }
  }
  return ret;
}

interface TabPanelProps {
  children?: React.ReactNode;
  index: number;
  value: number;
  idPrefix: string;
}

function CustomTabPanel(props: TabPanelProps) {
  const { children, value, index, idPrefix, ...other } = props;

  return (
    <div
      role='tabpanel'
      hidden={value !== index}
      id={`${idPrefix}-tabpanel-${index}`}
      aria-labelledby={`${idPrefix}-tab-${index}`}
      {...other}
    >
      {value === index && <Box sx={{ p: 3 }}>{children}</Box>}
    </div>
  );
}

function a11yProps(idPrefix: string, index: number) {
  return {
    id: `${idPrefix}-tab-${index}`,
    'aria-controls': `${idPrefix}-tabpanel-${index}`,
  };
}

function MultiQueryResp(props: { data: any[] }) {
  let { data } = props;
  const [value, setValue] = React.useState(0);
  const handleChange = (_event: React.SyntheticEvent, newValue: number) => {
    setValue(newValue);
  };
  let idPrefix = generateRandomString(10);

  return (
    <Box sx={{ width: '100%' }}>
      <Box sx={{ borderBottom: 1, borderColor: 'divider' }}>
        <Tabs
          value={value}
          onChange={handleChange}
          aria-label='basic tabs example'
        >
          {data.map((_, index) => (
            <Tab
              label={`Result ${index + 1}`}
              key={index}
              {...a11yProps(idPrefix, index)}
            />
          ))}
        </Tabs>
      </Box>

      {data.map((e, index) => {
        return (
          <CustomTabPanel
            idPrefix={idPrefix}
            key={`panel${index}`}
            value={value}
            index={index}
          >
            {e.Err ? (
              <Alert severity='error'>{e.Err}</Alert>
            ) : (
              <QueryResp
                data={e.Ok}
                order={''}
                orderBy={''}
                reloadSort={function (order: string, orderBy: string): void {
                  throw new Error('Function not implemented.');
                }}
              ></QueryResp>
            )}
          </CustomTabPanel>
        );
      })}
    </Box>
  );
}

const root = ReactDOM.createRoot(document.getElementById('root'));
export { MultiQueryResp };
root.render(
  <React.StrictMode>
    {/* <QueryResp data={testExampleQueryResp()}></QueryResp> */}
    <MultiQueryResp data={testExampleMultiQueryResp()}></MultiQueryResp>
  </React.StrictMode>
);
