import { types } from "./ApiStruct.ts";
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
} from "@mui/material";
import {
  handleCopyToClipboard,
  ContextMenuContext,
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
  ExportMarkDownTable,
  MenuExportMarkDownTable,
  ModalHeader,
  ViewRowInModal,
} from "./CommonComp.tsx";
import VisibilityIcon from "@mui/icons-material/Visibility";

function RespContextMenu(props: {
  content;
  field;
  setSort;
  viewInModal;
  showExportMarkDown;
}) {
  const { content, field, setSort, viewInModal, showExportMarkDown } = props;
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
      <MenuExportMarkDownTable showExportMarkDown={showExportMarkDown} />
    </MenuList>
  );
}

function smallDataSet(fields: types.Field[]) {
  return fields.length <= 4;
}

function queryRespFix(data: MuiDataGridData) {
  data.data.forEach((e, index) => {
    e[keyNotInTable] = index;
  });
  const considerSmallDataSet = smallDataSet(data.fields);
  data.fields.forEach(field => {
    if (considerSmallDataSet) {
      field["flex"] = 1;
    } else {
      const width = ColumnWidth.instance.get(field.field);
      if (width) {
        field["width"] = width;
      }
    }
    field["renderHeader"] = e => {
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
  const setting = useContext(SettingContext);
  const [showExportMarkDown, setShowExportMarkDown] = useState(false);

  const [viewInModal, setViewInModal] = useState<object>(null);
  const onColumnResize = useCallback(
    ({ colDef, width }) => {
      if (!data) {
        return;
      }
      if (smallDataSet(data.fields)) {
        return;
      }
      ColumnWidth.instance.save(colDef.field, width);
    },
    [data],
  );

  const [sortModel, setSortModel] = useState<GridSortModel>([]);
  const { setUpMenu } = useContext(ContextMenuContext);
  const slotProps = useMemo(() => {
    if (!data) {
      return undefined;
    }
    return {
      row: {
        onContextMenu: e => {
          const { id, field } = extractFieldAndIDFromMouseEvent(e);
          if (!field) {
            return;
          }
          const row = {
            ...data.data.find(e => e[keyNotInTable] == id),
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
                showExportMarkDown={() => {
                  setShowExportMarkDown(true);
                }}
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

  const onSortModelChange = useCallback(e => {
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
    return { columnMenu: AppColumnMenu } as Partial<GridSlotsComponent>;
  }, []);
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
            showToolbar
            getRowId={getRowId}
            slots={slots}
            sortModel={sortModel}
            onSortModelChange={onSortModelChange}
            showCellVerticalBorder
            showColumnVerticalBorder
            slotProps={slotProps}
            style={dataGridStyle}
            rowHeight={setting.appearance.datagridRowHeight()}
            columnHeaderHeight={setting.appearance.datagridRowHeight()}
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
      {showExportMarkDown && (
        <Modal
          open={showExportMarkDown}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setShowExportMarkDown(false);
          })}
        >
          <Box sx={modalStyle} width="60%">
            <ModalHeader
              title="export markdown table"
              handleClose={() => {
                setShowExportMarkDown(false);
              }}
            />
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

const styleForTrans: CSSProperties = {
  transition: "all 0.5s ease",
  position: "relative",
  overflow: "hidden",
};

function MultiQueryResp(props: { data: QueryRespData[] }) {
  const { data } = props;
  const [value, setValue] = React.useState(0);
  if (data.length == 0) {
    return <></>;
  }
  if (data.length == 1) {
    return data[0].Err ? (
      <Alert severity="error">{data[0].Err}</Alert>
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

export { MultiQueryResp, QueryResp, queryRespFix };
export type { QueryRespData };
