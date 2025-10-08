'use strict';
import { types } from './ApiStruct.ts';
import {
  Alert,
  Box,
  Divider,
  ListItemIcon,
  ListItemText,
  MenuItem,
  MenuList,
  Modal,
  Stack,
  Tab,
  Tabs,
  Tooltip,
  Typography,
} from '@mui/material';
import {
  handleCopyToClipboard,
  ContextMenuContext,
  resetMuiDataRespWidth,
  SettingContext,
  saveAndSetSize,
  ColumnWidth,
  MuiDataGridData,
  extractFieldAndIDFromMouseEvent,
} from './helper.ts';
import {
  DataGrid,
  GridSlotsComponent,
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
import {
  EscDownEvent,
  keyNotInTable,
  modalHandleClose,
  modalStyle,
} from './Common';
import ArrowUpwardIcon from '@mui/icons-material/ArrowUpward';
import ArrowDownwardIcon from '@mui/icons-material/ArrowDownward';

import ContentCopyIcon from '@mui/icons-material/ContentCopy';
import { NotificationContext } from './Notification.tsx';
import {
  AppColumnMenu,
  ExportMarkDownTable,
  MenuExportMarkDownTable,
  MenuResetColumnWidths,
  ModalHeader,
  ViewRowInModal,
} from './CommonComp.tsx';
import VisibilityIcon from '@mui/icons-material/Visibility';

function RespContextMenu(props: {
  content;
  field;
  setSort;
  resetColumnWidthCallBack;
  viewInModal;
  showExportMarkDown;
}) {
  const {
    content,
    field,
    setSort,
    resetColumnWidthCallBack,
    viewInModal,
    showExportMarkDown,
  } = props;
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

      <MenuItem onClick={viewInModal}>
        <ListItemIcon>
          <VisibilityIcon style={{ fontSize: '1.4rem' }}></VisibilityIcon>
        </ListItemIcon>
        <ListItemText> View in Modal </ListItemText>
      </MenuItem>
      <Divider />
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
      <MenuExportMarkDownTable
        showExportMarkDown={showExportMarkDown}
      ></MenuExportMarkDownTable>
      <MenuResetColumnWidths
        reset={resetColumnWidthCallBack}
      ></MenuResetColumnWidths>
    </MenuList>
  );
}

function QueryResp_(props: { data: MuiDataGridData }) {
  let { data: oldData } = props;
  const [data, setData] = useState<MuiDataGridData>(null);
  const setting = useContext(SettingContext);
  const [showExportMarkDown, setShowExportMarkDown] = useState(false);
  useEffect(() => {
    oldData.data.forEach((e, index) => {
      e[keyNotInTable] = index;
    });
    oldData.fields.forEach((field) => {
      const width = ColumnWidth.instance.get(field.field);
      if (width) {
        field['width'] = width;
      }
      field['renderHeader'] = (e) => {
        let def = e.colDef as types.Field;
        return (
          <Tooltip title={`${def.field} ${def.data_type}`}>
            <span> {def.field}</span>
          </Tooltip>
        );
      };
    });
    setData(oldData);
  }, [oldData]);
  const [viewInModal, setViewInModal] = useState<object>(null);

  const onColumnResize = useCallback(({ colDef, width }) => {
    ColumnWidth.instance.save(colDef.field, width);
  }, []);

  const [sortModel, setSortModel] = useState<GridSortModel>([]);
  const { setUpMenu } = useContext(ContextMenuContext);
  const slotProps = useMemo(() => {
    return {
      row: {
        onContextMenu: (e) => {
          const { id, field } = extractFieldAndIDFromMouseEvent(e);
          if (!field) {
            return;
          }
          let row = { ...data.data.find((e) => e[keyNotInTable] == id) };
          delete row[keyNotInTable];
          let content = row[field];
          setUpMenu({
            content: (
              <RespContextMenu
                viewInModal={() => {
                  setViewInModal(row);
                }}
                content={content}
                field={field}
                setSort={setSortModel}
                resetColumnWidthCallBack={() => {
                  setData((e) => {
                    return resetMuiDataRespWidth(e);
                  });
                }}
                showExportMarkDown={() => {
                  setShowExportMarkDown(true);
                }}
              ></RespContextMenu>
            ),
            event: e,
          });
        },
        style: { cursor: 'context-menu' },
      },
      toolbar: {
        csvOptions: {
          delimiter: setting.csvDelimiter,
          includeHeaders: setting.includeHeader,
        },
      },
    } as GridSlotsComponentsProps;
  }, [data]);

  const onSortModelChange = useCallback((e) => {
    setSortModel(e);
  }, []);
  const [viewInModalBox, setViewInModalBox] = useState(null);
  useEffect(() => {
    if (viewInModalBox) {
      let x1 = saveAndSetSize(viewInModalBox, 'view-row-in-modal');
      return () => {
        x1();
      };
    }
  }, [viewInModalBox]);

  const slots = useMemo(() => {
    return { columnMenu: AppColumnMenu } as Partial<GridSlotsComponent>;
  }, []);
  const gridStyle = useMemo(() => {
    return {} as CSSProperties;
  }, []);
  return (
    <div
      style={{
        height: '100%',
        display: 'flex',
        flexDirection: 'column',
      }}
    >
      {data && (
        <Stack spacing={1} flex={1}>
          {data.sql && (
            <Typography
              color="info"
              style={{
                whiteSpace: 'nowrap',
                textOverflow: 'ellipsis',
                overflow: 'hidden',
              }}
            >
              sql: {data.sql}
            </Typography>
          )}
          <div
            style={{
              display: 'grid',
              height: '100px',
              flexGrow: 1,
              flexShrink: 1, // 写flex 1 会有问题 好像不能缩小原因不清楚
            }}
          >
            <DataGrid
              showToolbar
              getRowId={getRowId}
              slots={slots}
              sortModel={sortModel}
              style={gridStyle}
              onSortModelChange={onSortModelChange}
              showCellVerticalBorder
              showColumnVerticalBorder
              slotProps={slotProps}
              rowHeight={setting.datagridRowHeightXXX()}
              columnHeaderHeight={setting.datagridRowHeightXXX()}
              columns={data.fields}
              rows={data.data}
              filterMode="client" // 使用客户端过滤
              onColumnResize={onColumnResize}
            ></DataGrid>
          </div>
        </Stack>
      )}

      {viewInModal && (
        <Modal
          open={!!viewInModal}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setViewInModal(null);
          })}
        >
          <Box
            sx={{ resize: 'both', ...modalStyle }}
            ref={setViewInModalBox}
            width="80%"
            height="30vh"
          >
            <ModalHeader title="View Row"></ModalHeader>
            <ViewRowInModal
              fields={data.fields}
              x={viewInModal}
            ></ViewRowInModal>
          </Box>
        </Modal>
      )}
      {showExportMarkDown && (
        <Modal
          open={showExportMarkDown}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setShowExportMarkDown(false);
          })}
        >
          <Box sx={modalStyle} width="60%">
            <ModalHeader title="export markdown table"></ModalHeader>
            <ExportMarkDownTable fields={data.fields} data={data.data} />
          </Box>
        </Modal>
      )}
    </div>
  );
}

function getRowId(row) {
  return row[keyNotInTable];
}

let styleForTrans: CSSProperties = {
  transition: 'all 0.5s ease',
  position: 'relative',
  overflow: 'hidden',
};

function MultiQueryResp_(props: { data: any[] }) {
  let { data: oldData } = props;
  const [data, setData] = useState([]);
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
    }, 600);
  }, [oldData]);
  useEffect(() => {
    setTimeout(() => {
      setData(oldData);
    }, 500);
  }, [oldData]);

  if (data.length == 0) {
    return <></>;
  }
  if (data.length == 1) {
    return data[0].Err ? (
      <Alert
        style={{ top, opacity, left: top, ...styleForTrans }}
        severity="error"
      >
        {data[0].Err}
      </Alert>
    ) : (
      <div
        style={{
          top,
          opacity,
          left: top,
          ...styleForTrans,
          height: '100%',
        }}
      >
        <QueryResp data={data[0].Ok}></QueryResp>
      </div>
    );
  }

  const handleChange = (_event: React.SyntheticEvent, newValue: number) => {
    setValue(newValue);
  };

  return (
    <div
      style={{
        top,
        opacity,
        left: top,
        height: '100%',
        display: 'flex',
        flexDirection: 'column',
        ...styleForTrans,
      }}
    >
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

      <div style={{ height: '100px', flex: 1 }}>
        {data.map((e, index) => {
          return (
            <div hidden={value != index} key={index} style={{ height: '100%' }}>
              {e.Err ? (
                <Alert severity="error">{e.Err}</Alert>
              ) : (
                <QueryResp data={e.Ok}></QueryResp>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}
const MultiQueryResp = memo(MultiQueryResp_);
const QueryResp = memo(QueryResp_);

export { MultiQueryResp, QueryResp };
