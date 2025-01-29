import { types } from './ApiStruct.ts';
import {
  Alert,
  ListItemIcon,
  ListItemText,
  MenuItem,
  MenuList,
  Paper,
  Tab,
  Tabs,
  Tooltip,
} from '@mui/material';
import { v4 as uuidv4 } from 'uuid';
import {
  calMenuStyle,
  cloumnWidths,
  datagridContainerHeight,
  generateRandomString,
  handleCopyToClipboard,
  saveColumnWidths,
} from './helper.ts';
import { DataGrid, GridSlotsComponentsProps } from '@mui/x-data-grid';
import React, { memo, useCallback, useContext, useMemo } from 'react';
import { ContextMenuContext, VisiableContext } from './Common.tsx';
import { datagridHeaderHeight, datagridRowHeight } from './CommonComp.tsx';
import ContentCopyIcon from '@mui/icons-material/ContentCopy';
import { NotificationContext } from './Notification.tsx';

function RespContextMenu(props: { content }) {
  const { content } = props;
  const { showNotification } = useContext(NotificationContext);
  return (
    <Paper>
      <MenuList>
        <MenuItem
          onClick={() => {
            handleCopyToClipboard(content, showNotification);
          }}
        >
          <ListItemIcon>
            <ContentCopyIcon></ContentCopyIcon>
          </ListItemIcon>
          <ListItemText> copy </ListItemText>
        </MenuItem>
      </MenuList>
    </Paper>
  );
}

function QueryResp_(props: { data: types.MuiDataGridData }) {
  let { data } = props;
  data.data.forEach((e) => {
    if (!('id' in e)) {
      e.id = uuidv4();
    }
  });
  data.fields.forEach((e) => {
    const width = cloumnWidths[e.field];
    if (width) {
      e['width'] = width;
    }
    //@ts-ignore
    e.renderHeader = (e) => {
      let def = e.colDef;
      return (
        <Tooltip title={def.typ}>
          <strong> {def.field}</strong>
        </Tooltip>
      );
    };
  });

  const onColumnResize = useCallback(({ colDef, width }) => {
    saveColumnWidths(colDef.field, width);
  }, []);
  const vis = useContext(VisiableContext);

  const { setUpMenu } = useContext(ContextMenuContext);
  const slotProps = useMemo(() => {
    return {
      row: {
        onContextMenu: (e) => {
          const { target } = e;
          let content = (target as HTMLDivElement).innerHTML;
          setUpMenu({
            content: <RespContextMenu content={content}></RespContextMenu>,
            style: calMenuStyle(e, 200),
          });
        },
        style: { cursor: 'context-menu' },
      },
    } as GridSlotsComponentsProps;
  }, []);

  return (
    vis && (
      <div
        style={{
          display: 'grid',
          height: datagridContainerHeight(data.data.length),
        }}
      >
        <DataGrid
          slotProps={slotProps}
          rowHeight={datagridRowHeight}
          columnHeaderHeight={datagridHeaderHeight}
          columns={data.fields}
          rows={data.data}
          filterMode="client" // 使用客户端过滤
          onColumnResize={onColumnResize}
        ></DataGrid>
      </div>
    )
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

function MultiQueryResp_(props: { data: any[] }) {
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
  const vis = useContext(VisiableContext);
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
          <VisiableContext.Provider
            value={vis && index == value}
            key={`panel${index}`}
          >
            <CustomTabPanel idPrefix={idPrefix} value={value} index={index}>
              {e.Err ? (
                <Alert severity="error">{e.Err}</Alert>
              ) : (
                <QueryResp data={e.Ok}></QueryResp>
              )}
            </CustomTabPanel>
          </VisiableContext.Provider>
        );
      })}
    </div>
  );
}
const MultiQueryResp = memo(MultiQueryResp_);
const QueryResp = memo(QueryResp_);

export { MultiQueryResp, QueryResp };
