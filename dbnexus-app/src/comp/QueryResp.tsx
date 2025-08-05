import { types } from './ApiStruct.ts';
import {
  Alert,
  Divider,
  ListItemIcon,
  ListItemText,
  MenuItem,
  MenuList,
  Stack,
  Tab,
  Tabs,
  Tooltip,
  Typography,
} from '@mui/material';
import { v4 as uuidv4 } from 'uuid';
import {
  calMenuStyle,
  generateRandomString,
  handleCopyToClipboard,
  columnWidths,
  ContextMenuContext,
  VisiableContext,
  resetMuiDataRespWidth,
} from './helper.ts';
import {
  DataGrid,
  GridSlotsComponentsProps,
  GridSortModel,
} from '@mui/x-data-grid';
import React, {
  CSSProperties,
  memo,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
} from 'react';
import { SettingContext } from './Common.tsx';
import ArrowUpwardIcon from '@mui/icons-material/ArrowUpward';
import ArrowDownwardIcon from '@mui/icons-material/ArrowDownward';

import ContentCopyIcon from '@mui/icons-material/ContentCopy';
import { NotificationContext } from './Notification.tsx';
import { MenuResetColumnWidths } from './CommonComp.tsx';

function RespContextMenu(props: {
  content;
  field;
  setSort;
  resetColunmWidthCallBack;
}) {
  const { content, field, setSort, resetColunmWidthCallBack } = props;
  const { showNotification } = useContext(NotificationContext);
  return (
    <MenuList>
      {content && (
        <MenuItem
          onClick={() => {
            handleCopyToClipboard(content, showNotification);
          }}
        >
          <ListItemIcon>
            <ContentCopyIcon></ContentCopyIcon>
          </ListItemIcon>
          <ListItemText> Copy </ListItemText>
        </MenuItem>
      )}

      <MenuItem
        onClick={() => {
          handleCopyToClipboard(field, showNotification);
        }}
      >
        <ListItemIcon>
          <ContentCopyIcon></ContentCopyIcon>
        </ListItemIcon>
        <ListItemText> Copy Field </ListItemText>
      </MenuItem>
      <Divider></Divider>
      <MenuItem
        onClick={() => {
          setSort([{ field, sort: 'asc' }] as GridSortModel);
        }}
      >
        <ListItemIcon>
          <ArrowUpwardIcon></ArrowUpwardIcon>
        </ListItemIcon>
        <ListItemText> {field} ASC </ListItemText>
      </MenuItem>
      <MenuItem
        onClick={() => {
          setSort([{ field, sort: 'desc' }] as GridSortModel);
        }}
      >
        <ListItemIcon>
          <ArrowDownwardIcon></ArrowDownwardIcon>
        </ListItemIcon>
        <ListItemText> {field} DESC </ListItemText>
      </MenuItem>
      <MenuResetColumnWidths
        reset={resetColunmWidthCallBack}
      ></MenuResetColumnWidths>
    </MenuList>
  );
}

function QueryResp_(props: { data: types.MuiDataGridData }) {
  let { data: oldData } = props;
  const [data, setData] = useState<types.MuiDataGridData>(null);
  useEffect(() => {
    oldData.data.forEach((e) => {
      if (!('id' in e)) {
        e.id = uuidv4();
      }
    });
    oldData.fields.forEach((e) => {
      const width = columnWidths.getWidth(e.field);
      if (width) {
        e['width'] = width;
      }
      //@ts-ignore
      e.renderHeader = (e) => {
        let def = e.colDef;
        return (
          <Tooltip title={`${def.field} ${def.typ}`}>
            <strong> {def.field}</strong>
          </Tooltip>
        );
      };
    });

    console.log(oldData);
    setData(oldData);
  }, [oldData]);

  const onColumnResize = useCallback(({ colDef, width }) => {
    columnWidths.saveColumnWidths(colDef.field, width);
  }, []);
  const vis = useContext(VisiableContext);
  const [sortModel, setSortModel] = useState<GridSortModel>([]);
  const { setUpMenu } = useContext(ContextMenuContext);
  const slotProps = useMemo(() => {
    return {
      row: {
        onContextMenu: (e) => {
          const { target } = e;
          let content = (target as HTMLDivElement).innerHTML;
          let field = (target as HTMLDivElement).getAttribute('data-field');
          if (!field) {
            return;
          }
          setUpMenu({
            content: (
              <RespContextMenu
                content={content}
                field={field}
                setSort={setSortModel}
                resetColunmWidthCallBack={() => {
                  setData((e) => {
                    return resetMuiDataRespWidth(e);
                  });
                }}
              ></RespContextMenu>
            ),
            style: calMenuStyle(e, 200),
          });
        },
        style: { cursor: 'context-menu' },
      },
    } as GridSlotsComponentsProps;
  }, []);
  const setting = useContext(SettingContext);
  const onSortModelChange = useCallback((e) => {
    setSortModel(e);
  }, []);

  const gridStyle = useMemo(() => {
    return { height: 'auto', maxHeight: '78vh' } as CSSProperties;
  }, []);
  return (
    data && (
      <Stack spacing={1}>
        {data.sql && <Typography color="info">sql: {data.sql} </Typography>}
        {vis && (
          <div
            style={{
              display: 'grid',
            }}
          >
            <DataGrid
              style={gridStyle}
              sortModel={sortModel}
              onSortModelChange={onSortModelChange}
              showCellVerticalBorder
              showColumnVerticalBorder
              slotProps={slotProps}
              rowHeight={setting.datagridRowHeight}
              columnHeaderHeight={setting.datagridHeaderHeight}
              columns={data.fields}
              rows={data.data}
              filterMode="client" // 使用客户端过滤
              onColumnResize={onColumnResize}
            ></DataGrid>
          </div>
        )}
      </Stack>
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
      {...other}
    >
      {value === index && <div>{children}</div>}
    </div>
  );
}

let styleForTrans: CSSProperties = {
  transition: 'all 0.3s ease',
  position: 'relative',
  overflow: 'hidden',
};

function MultiQueryResp_(props: { data: any[] }) {
  let { data } = props;
  if (data.length == 0) {
    return <></>;
  }
  const [top, setTop] = useState(0);
  const [value, setValue] = React.useState(0);
  const [opacity, setOpacity] = useState(1);
  useEffect(() => {
    if (value >= data.length) {
      setValue(0);
    }
    setTop(50);
    setOpacity(0);
    setTimeout(() => {
      setTop(0);
      setOpacity(1);
    }, 400);
  }, [data]);

  if (data.length == 1) {
    return data[0].Err ? (
      <Alert
        style={{ top, opacity, left: top, ...styleForTrans }}
        severity="error"
      >
        {data[0].Err}
      </Alert>
    ) : (
      <div style={{ top, opacity, left: top, ...styleForTrans }}>
        <QueryResp data={data[0].Ok}></QueryResp>
      </div>
    );
  }

  const handleChange = (_event: React.SyntheticEvent, newValue: number) => {
    setValue(newValue);
  };

  let idPrefix = generateRandomString(10);
  const vis = useContext(VisiableContext);
  return (
    <div style={{ top, opacity, left: top, ...styleForTrans }}>
      <div style={{ display: 'grid' }}>
        <Tabs
          value={value}
          onChange={handleChange}
          variant="scrollable"
          scrollButtons="auto"
        >
          {data.map((_, index) => (
            <Tab label={`Result ${index + 1}`} key={index} />
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
