import { types } from './ApiStruct.ts';
import { Alert, Tab, Tabs, Tooltip } from '@mui/material';
import { v4 as uuidv4 } from 'uuid';
import { generateRandomString } from './helper.ts';
import { DataGrid } from '@mui/x-data-grid';
import React from 'react';
import { datagridHeaderHeight, datagridRowHeight } from './Common.tsx';

function QueryResp(props: { data: types.MuiDataGridData }) {
  let { data } = props;
  data.data.forEach((e) => {
    if (!('id' in e)) {
      e.id = uuidv4();
    }
  });
  data.fields.forEach((e) => {
    //@ts-ignore
    e.renderHeader = (e) => {
      let def = e.colDef;
      return (
        <Tooltip title={def.typ}>
          <strong> {def.field}</strong>
        </Tooltip>
      );
    };
    //@ts-ignore
    e.flex = 1;
  });
  return (
    <div style={{ display: 'grid' }}>
      <DataGrid
        rowHeight={datagridRowHeight}
        columnHeaderHeight={datagridHeaderHeight}
        columns={data.fields}
        rows={data.data}
        filterMode="client" // 使用客户端过滤
      ></DataGrid>
    </div>
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
      role="tabpanel"
      hidden={value !== index}
      id={`${idPrefix}-tabpanel-${index}`}
      aria-labelledby={`${idPrefix}-tab-${index}`}
      {...other}
    >
      {value === index && <div>{children}</div>}
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
  if (data.length == 0) {
    return <></>;
  }
  if (data.length == 1) {
    return data[0].Err ? (
      <Alert severity="error">{data[0].Err}</Alert>
    ) : (
      <QueryResp data={data[0].Ok}></QueryResp>
    );
  }
  const [value, setValue] = React.useState(0);
  const handleChange = (_event: React.SyntheticEvent, newValue: number) => {
    setValue(newValue);
  };
  let idPrefix = generateRandomString(10);
  return (
    <div>
      <div>
        <Tabs value={value} onChange={handleChange} aria-label="query results.">
          {data.map((_, index) => (
            <Tab
              label={`Result ${index + 1}`}
              key={index}
              {...a11yProps(idPrefix, index)}
            />
          ))}
        </Tabs>
      </div>

      {data.map((e, index) => {
        return (
          <CustomTabPanel
            idPrefix={idPrefix}
            key={`panel${index}`}
            value={value}
            index={index}
          >
            {e.Err ? (
              <Alert severity="error">{e.Err}</Alert>
            ) : (
              <QueryResp data={e.Ok}></QueryResp>
            )}
          </CustomTabPanel>
        );
      })}
    </div>
  );
}

export { MultiQueryResp, QueryResp };
