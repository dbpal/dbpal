import {
  Alert,
  Button,
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
} from "@mui/material";
import {
  handleCopyToClipboard,
  extractFieldAndIDFromMouseEvent,
  dataGridStyle,
  QueryRespData,
  AppColumnMenuCtx,
  columnHeaderStyle,
  clickDataGridColumn,
  allGridHeaderTitle,
  gridColumnHeaderSelectClassName,
  UuidContext,
} from "./helper.ts";

import React, { useCallback, useContext, useEffect, useMemo, useState } from "react";
import { keyNotInTable } from "./Common";
import ArrowUpwardIcon from "@mui/icons-material/ArrowUpward";
import ArrowDownwardIcon from "@mui/icons-material/ArrowDownward";

import { AppColumnMenu, ResetColumnWidth, ViewRowInModal } from "./CommonComp.tsx";
import VisibilityIcon from "@mui/icons-material/Visibility";
import {
  AppDataGridPagination,
  CopyAsXXX,
  ExportQueryResultToFile,
  StackRowCenter,
} from "./CommonComp2.tsx";
import {
  useColumnWidthStore,
  useContextMenu,
  useNotification,
  usePageSize,
} from "./store.tsx";
import { Field, MuiDataGridData } from "./bindings.ts";
import {
  DataGrid,
  GridSlotsComponent,
  GridSlotsComponentsProps,
  GridSortModel,
  useGridApiRef,
} from "./datagridExport.ts";
import { CustomToolbar } from "./CustomToolBar.tsx";

function RespContextMenu(props: {
  content: string;
  field: string;
  setSort: (model: GridSortModel) => void;
  viewInModal: () => void;
  dbName: string;
  fieldType: string;
  getRow: () => object;
}): React.ReactElement {
  const { content, field, setSort, viewInModal, dbName, getRow, fieldType } = props;
  return (
    <MenuList>
      <CopyAsXXX
        content={content}
        field={field}
        dbName={dbName}
        getRow={getRow}
        fieldType={fieldType}
      />
      <MenuItem onClick={viewInModal}>
        <ListItemIcon>
          <VisibilityIcon style={{ fontSize: "1.4rem" }} />
        </ListItemIcon>
        <ListItemText> View in Modal </ListItemText>
      </MenuItem>
      <Divider />
      <MenuItem
        onClick={() => {
          setSort([{ field, sort: "asc" }]);
        }}>
        <ListItemIcon>
          <ArrowUpwardIcon />
        </ListItemIcon>
        <ListItemText> {field} ASC </ListItemText>
      </MenuItem>
      <MenuItem
        onClick={() => {
          setSort([{ field, sort: "desc" }]);
        }}>
        <ListItemIcon>
          <ArrowDownwardIcon />
        </ListItemIcon>
        <ListItemText> {field} DESC </ListItemText>
      </MenuItem>
    </MenuList>
  );
}

function QueryResp(props: { data: MuiDataGridData }): React.ReactElement {
  const { data } = props;
  const { dbName } = useContext(UuidContext);
  const { setColumnWidth, getColumnWidth } = useColumnWidthStore();
  const [selectedColumns, setSelectColumns] = useState<Set<string>>(new Set());
  const apiRef = useGridApiRef();
  const [page, setPage] = useState(0);
  const { setPageSize, sql: pageSize } = usePageSize();
  const [viewInModal, setViewInModal] = useState<object>(null);

  data.data.forEach((e, index) => {
    e[keyNotInTable] = index + "";
  });
  data.fields.forEach((field) => {
    const width = getColumnWidth(field.field);
    if (width) {
      field["width"] = width;
    }
    field["renderHeader"] = (e) => {
      const def = e.colDef as Field;
      return (
        <Stack
          style={{ ...columnHeaderStyle }}
          onClick={(event) => {
            event.stopPropagation();
            clickDataGridColumn(apiRef, event, field.field, setSelectColumns);
          }}>
          <Tooltip title={`${def.field} ${def.data_type}`}>
            <span> {def.field}</span>
          </Tooltip>
        </Stack>
      );
    };
  });

  const onColumnResize = useCallback(
    ({ colDef, width }) => {
      if (!data) {
        return;
      }
      setColumnWidth(colDef.field, width);
    },
    [data, setColumnWidth],
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
                getRow={() => {
                  return row;
                }}
                fieldType={data.fields.find((e) => e.field == field).data_type}
                dbName={dbName}
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
    } as GridSlotsComponentsProps;
  }, [data, setUpMenu, dbName]);

  const onSortModelChange = useCallback((e) => {
    setSortModel(e);
  }, []);

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
            }}>
            <Stack style={{ flex: 1, alignItems: "center" }} direction="row">
              <CustomToolbar />
              <ResetColumnWidth />
              <ExportQueryResultToFile />
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
      }}>
      {data && (
        <Stack style={{ flex: 1 }}>
          {data.sql && (
            <Typography
              color="info"
              style={{
                whiteSpace: "nowrap",
                textOverflow: "ellipsis",
                overflow: "hidden",
              }}>
              sql: {data.sql}
            </Typography>
          )}
          <AppColumnMenuCtx
            value={{
              selectColumns: (function () {
                return data.fields
                  .filter((f) => selectedColumns.has(f.field))
                  .map((f) => f.field);
              })(),
            }}>
            <DataGrid
              getCellClassName={({ field }) => {
                if (selectedColumns.has(field)) {
                  return gridColumnHeaderSelectClassName;
                }
              }}
              onCellClick={() => {
                setSelectColumns(new Set());
                allGridHeaderTitle(apiRef, (x) => {
                  x.classList.remove(gridColumnHeaderSelectClassName);
                });
              }}
              apiRef={apiRef}
              getRowId={getRowId}
              slots={slots}
              sortModel={sortModel}
              onSortModelChange={onSortModelChange}
              slotProps={slotProps}
              style={dataGridStyle}
              columns={data.fields}
              rows={data.data}
              showToolbar={false}
              filterMode="client" // 使用客户端过滤
              onColumnResize={onColumnResize}
              paginationModel={{ page, pageSize }}
              onPaginationModelChange={(e) => {
                setPage(e.page);
                setPageSize("sql", e.pageSize);
              }}
            />
          </AppColumnMenuCtx>
        </Stack>
      )}

      <ViewRowInModal
        fields={data.fields}
        data={viewInModal}
        handleClose={() => {
          setViewInModal(null);
        }}
      />
    </div>
  );
}

function getRowId(row: Record<string, any>): string {
  return row[keyNotInTable];
}

function MultiQueryResp(props: { data: QueryRespData[] }): React.ReactElement {
  const { data } = props;
  const [value, setValue] = React.useState(0);

  useEffect(() => {
    if (data.length > 0) {
      setValue(data.length - 1);
    }
  }, [data]);

  const { showNotification } = useNotification();
  if (data.length == 0) {
    return <></>;
  }
  if (data.length == 1) {
    return data[0].Err ? (
      <StackRowCenter>
        <Button
          onClick={() => {
            handleCopyToClipboard(data[0].Err, showNotification);
          }}>
          copy
        </Button>
        <Alert severity="error">{data[0].Err}</Alert>
      </StackRowCenter>
    ) : (
      <QueryResp data={data[0].Ok} />
    );
  }
  const handleChange = (_event: React.SyntheticEvent, newValue: number): void => {
    setValue(newValue);
  };
  return (
    <Stack
      style={{
        height: "100%",
      }}
      direction="row">
      <Tabs
        orientation="vertical"
        value={value}
        onChange={handleChange}
        variant="scrollable"
        scrollButtons="auto">
        {data.map((_, index) => (
          <Tab
            label={(function () {
              return `Result ${index + 1}`;
            })()}
            key={index}
          />
        ))}
        st
      </Tabs>

      <div style={{ height: "100%", width: "100px", flex: 1 }}>
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
    </Stack>
  );
}

export { MultiQueryResp, QueryResp };
