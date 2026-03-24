import { types } from "./ApiStruct.ts";
import {
  Alert,
  Box,
  Button,
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
} from "@mui/material";
import {
  handleCopyToClipboard,
  SettingContext,
  saveAndSetSize,
  ColumnWidth,
  MuiDataGridData,
  extractFieldAndIDFromMouseEvent,
  dataGridStyle,
  QueryRespData,
} from "./helper.ts";
import {
  DataGrid,
  GridSlotsComponent,
  GridSlotsComponentsProps,
  GridSortModel,
} from "@mui/x-data-grid";
import React, {
  CSSProperties,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
} from "react";
import { EscDownEvent, keyNotInTable, modalHandleClose, modalStyle } from "./Common";
import ArrowUpwardIcon from "@mui/icons-material/ArrowUpward";
import ArrowDownwardIcon from "@mui/icons-material/ArrowDownward";

import ContentCopyIcon from "@mui/icons-material/ContentCopy";
import { NotificationContext } from "./Notification.tsx";
import {
  AppColumnMenu,
  ModalHeader,
  ResetColumnWidth,
  ViewRowInModal,
} from "./CommonComp.tsx";
import VisibilityIcon from "@mui/icons-material/Visibility";
import { AppDataGridPagination, StackRowCenter } from "./CommonComp2.tsx";
import { useContextMenu } from "./store.ts";

function RespContextMenu(props: { content; field; setSort; viewInModal }) {
  const { content, field, setSort, viewInModal } = props;
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
            <ContentCopyIcon />
          </ListItemIcon>
          <ListItemText> Copy </ListItemText>
        </MenuItem>
      )}

      <MenuItem onClick={viewInModal}>
        <ListItemIcon>
          <VisibilityIcon style={{ fontSize: "1.4rem" }} />
        </ListItemIcon>
        <ListItemText> View in Modal </ListItemText>
      </MenuItem>
      <Divider />
      <MenuItem
        onClick={() => {
          setSort([{ field, sort: "asc" }] as GridSortModel);
        }}
      >
        <ListItemIcon>
          <ArrowUpwardIcon />
        </ListItemIcon>
        <ListItemText> {field} ASC </ListItemText>
      </MenuItem>
      <MenuItem
        onClick={() => {
          setSort([{ field, sort: "desc" }] as GridSortModel);
        }}
      >
        <ListItemIcon>
          <ArrowDownwardIcon />
        </ListItemIcon>
        <ListItemText> {field} DESC </ListItemText>
      </MenuItem>
    </MenuList>
  );
}

function queryRespFix(data: MuiDataGridData) {
  data.data.forEach((e, index) => {
    e[keyNotInTable] = index;
  });
  data.fields.forEach((field) => {
    const width = ColumnWidth.instance.get(field.field);
    if (width) {
      field["width"] = width;
    }
    field["renderHeader"] = (e) => {
      const def = e.colDef as types.Field;
      return (
        <Tooltip title={`${def.field} ${def.data_type}`}>
          <span> {def.field}</span>
        </Tooltip>
      );
    };
  });
}

function QueryResp(props: { data: MuiDataGridData }) {
  const { data } = props;

  queryRespFix(data);

  const setting = useContext(SettingContext);

  const [viewInModal, setViewInModal] = useState<object>(null);
  const onColumnResize = useCallback(
    ({ colDef, width }) => {
      if (!data) {
        return;
      }
      ColumnWidth.instance.save(colDef.field, width);
    },
    [data],
  );

  const [sortModel, setSortModel] = useState<GridSortModel>([]);
  const setUpMenu = useContextMenu((state) => state.setUpMenu);
  const slotProps = useMemo(() => {
    if (!data) {
      return undefined;
    }
    return {
      row: {
        onContextMenu: (e) => {
          const { id, field } = extractFieldAndIDFromMouseEvent(e);
          if (!field) {
            return;
          }
          const row = {
            ...data.data.find((e) => e[keyNotInTable] == id),
          };
          delete row[keyNotInTable];
          const content = row[field];
          setUpMenu({
            content: (
              <RespContextMenu
                viewInModal={() => {
                  setViewInModal(row);
                }}
                content={content}
                field={field}
                setSort={setSortModel}
              />
            ),
            event: e,
          });
        },
        style: { cursor: "context-menu" },
      },
      toolbar: {
        csvOptions: {
          delimiter: setting.export.csvDelimiter,
          includeHeaders: setting.export.includeHeader,
        },
      },
    } as GridSlotsComponentsProps;
  }, [data, setUpMenu, setting.export.csvDelimiter, setting.export.includeHeader]);

  const onSortModelChange = useCallback((e) => {
    setSortModel(e);
  }, []);
  const [viewInModalBox, setViewInModalBox] = useState(null);
  useEffect(() => {
    if (viewInModalBox) {
      const x1 = saveAndSetSize(viewInModalBox, "view-row-in-modal");
      return () => {
        x1();
      };
    }
  }, [viewInModalBox]);

  const slots = useMemo(() => {
    return {
      columnMenu: AppColumnMenu,
      pagination: () => {
        return (
          <Stack
            direction="row"
            style={{
              alignItems: "center",
              justifyContent: "space-between",
              flex: 1,
              overflow: "auto",
            }}
          >
            <Stack style={{ flex: 1, alignItems: "center" }} direction="row">
              <ResetColumnWidth />
              <Typography color="info">rows: {data.data.length} </Typography>
            </Stack>
            <AppDataGridPagination showPageSelect />
          </Stack>
        );
      },
    } as Partial<GridSlotsComponent>;
  }, [data.data.length]);
  return (
    <div
      style={{
        height: "100%",
        display: "flex",
        flexDirection: "column",
      }}
    >
      {data && (
        <Stack flex={1}>
          {data.sql && (
            <Typography
              color="info"
              style={{
                whiteSpace: "nowrap",
                textOverflow: "ellipsis",
                overflow: "hidden",
              }}
            >
              sql: {data.sql}
            </Typography>
          )}

          <DataGrid
            getRowId={getRowId}
            slots={slots}
            sortModel={sortModel}
            onSortModelChange={onSortModelChange}
            slotProps={slotProps}
            style={dataGridStyle}
            columns={data.fields}
            rows={data.data}
            filterMode="client" // 使用客户端过滤
            onColumnResize={onColumnResize}
          />
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
            sx={{ resize: "both", ...modalStyle }}
            ref={setViewInModalBox}
            width="80%"
            height="30vh"
          >
            <ModalHeader
              title="View Row"
              handleClose={() => {
                setViewInModal(null);
              }}
            />
            <ViewRowInModal fields={data.fields} data={viewInModal} />
          </Box>
        </Modal>
      )}
    </div>
  );
}

function getRowId(row) {
  return row[keyNotInTable];
}

const styleForTrans: CSSProperties = {
  transition: "all 0.5s ease",
  position: "relative",
  overflow: "hidden",
};

function MultiQueryResp(props: { data: QueryRespData[] }) {
  const { data } = props;
  const [value, setValue] = React.useState(0);

  const { showNotification } = useContext(NotificationContext);
  if (data.length == 0) {
    return <></>;
  }
  if (data.length == 1) {
    return data[0].Err ? (
      <StackRowCenter>
        <Button
          onClick={() => {
            handleCopyToClipboard(data[0].Err, showNotification);
          }}
        >
          copy
        </Button>
        <Alert severity="error">{data[0].Err}</Alert>
      </StackRowCenter>
    ) : (
      <QueryResp data={data[0].Ok} />
    );
  }
  const handleChange = (_event: React.SyntheticEvent, newValue: number) => {
    setValue(newValue);
  };
  return (
    <div
      style={{
        height: "100%",
        display: "flex",
        flexDirection: "column",
        ...styleForTrans,
      }}
    >
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

      <div style={{ height: "100px", flex: 1 }}>
        {data.map((e, index) => {
          return (
            <div hidden={value != index} key={index} style={{ height: "100%" }}>
              {e.Err ? (
                <Alert severity="error">{e.Err}</Alert>
              ) : (
                <QueryResp data={e.Ok} />
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}

export { MultiQueryResp, QueryResp };
export type { QueryRespData };
