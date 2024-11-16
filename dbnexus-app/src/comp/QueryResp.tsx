import apis from './ApiStruct.ts';
import { Alert, Box, Tab, Tabs } from '@mui/material';
import { v4 as uuidv4 } from 'uuid';
import { generateRandomString } from './helper.ts';
import { DataGrid } from '@mui/x-data-grid';
import React from 'react';

function QueryResp(props: { data: apis.MuiDataGridData }) {
  let { data } = props;
  data.data.forEach((e) => {
    if (!e['id']) {
      e.id = uuidv4();
    }
  });
  return (
    <DataGrid
      columns={data.fields}
      rows={data.data}
      filterMode='client' // 使用客户端过滤
    ></DataGrid>
  );
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
              <QueryResp data={e.Ok}></QueryResp>
            )}
          </CustomTabPanel>
        );
      })}
    </Box>
  );
}

export { MultiQueryResp, QueryResp };
