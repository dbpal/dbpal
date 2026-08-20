import {
  Alert,
  Button,
  Divider,
  IconButton,
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
  DbnameContext,
  DataSourceKind,
} from "./helper.ts";
import EditIcon from "@mui/icons-material/Edit";

import React, { useCallback, useContext, useEffect, useMemo, useState } from "react";
import { keyNotInTable } from "./Common";
import ArrowUpwardIcon from "@mui/icons-material/ArrowUpward";
import ArrowDownwardIcon from "@mui/icons-material/ArrowDownward";
import PlayArrowIcon from "@mui/icons-material/PlayArrow";
import { AppColumnMenu, ResetColumnWidth, ViewRowInModal } from "./CommonComp.tsx";
import VisibilityIcon from "@mui/icons-material/Visibility";
import {
  AppCode,
  AppDataGridPagination,
  CopyAsXXX,
  ExportQueryResultToFile,
  SqlTableViewData,
  StackRowCenter,
} from "./CommonComp2.tsx";
import {
  useColumnWidthStore,
  useContextMenu,
  useNotification,
  useOneShotDialogUI,
  usePageSize,
} from "./store.tsx";
import { commands, Field, MuiDataGridData } from "./bindings.ts";
import {
  DataGrid,
  GridSlotsComponent,
  GridSlotsComponentsProps,
  GridSortModel,
  useGridApiRef,
} from "./datagridExport.ts";
import { CustomToolbar } from "./CustomToolBar.tsx";

function EditResultSetData(props: {
  dbName: string;
  sql: string;
  oldValues: object;
  fields: Field[];
}) {
  const { dbName, sql, oldValues, fields } = props;
  const { showNotification } = useNotification();
  const [up, setUp] = useState<string[]>(null);
  let makeArray = (x: object) => {
    return fields.map((f) => x[f.field]);
  };
  return (
    <Stack spacing={1}>
      {up &&
        up.map((sql, index) => (
          <AppCode
            key={index}
            codeBlock={(function () {
              return up.join(";\n");
            })()}
            extra={() => {
              return (
                <Tooltip title="try execute the sql,those sql execute not in transaction mode,notice this.">
                  <IconButton
                    color="primary"
                    onClick={async () => {
                      try {
                        const row_aff = await commands.simpleExecuteSql(dbName, sql);
                        showNotification({
                          severity: "success",
                          title: "execute sql success",
                          message: `row affected ${row_aff}`,
                        });
                      } catch (e) {
                        showNotification({
                          severity: "error",
                          message: e,
                          title: "execute sql failed ",
                        });
                      }
                    }}>
                    <PlayArrowIcon />
                  </IconButton>
                </Tooltip>
              );
            }}
            language="sql"
          />
        ))}

      <SqlTableViewData
        initValues={oldValues}
        fields={fields}
        isAdd={false}
        buttonName={"gen modify sql"}
        handleData={async (newvalues) => {
          try {
            let ret = await commands.getUpdateSqlFromResultSet(
              dbName,
              sql,
              makeArray(oldValues),
              makeArray(newvalues),
            );
            setUp(ret);
          } catch (e) {
            showNotification({
              severity: "error",
              title: "trying to get update sql failed",
              message: e,
            });
          }
        }}
      />
    </Stack>
  );
}

function RespContextMenu(props: {
  content: string;
  field: string;
  fields: Field[];
  setSort: (model: GridSortModel) => void;
  viewInModal: () => void;
  dbName: string;
  kind: DataSourceKind;
  fieldType: string;
  getRow: () => object;
  sql: string;
}): React.ReactElement {
  const {
    content,
    field,
    setSort,
    viewInModal,
    dbName,
    getRow,
    fieldType,
    kind,
    sql,
    fields,
  } = props;
  const supportModify = ![DataSourceKind.Redis].includes(kind);
  const { setOneShotUI } = useOneShotDialogUI();

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
      {supportModify && (
        <MenuItem
          onClick={() => {
            setOneShotUI({
              ui: (
                <EditResultSetData
                  fields={fields}
                  sql={sql}
                  dbName={dbName}
                  oldValues={getRow()}
                />
              ),
              op: { maxWidth: "xl", fullWidth: true },
            });
          }}>
          <ListItemIcon>
            <EditIcon />
          </ListItemIcon>
          <ListItemText> try edit </ListItemText>
        </MenuItem>
      )}
    </MenuList>
  );
}

function QueryResp(props: { data: MuiDataGridData }): React.ReactElement {
  const { data } = props;
  const { dbName, kind } = useContext(DbnameContext);
  const { getColumnWidth } = useColumnWidthStore();
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
                fields={data.fields}
                fieldType={data.fields.find((e) => e.field == field).data_type}
                dbName={dbName}
                kind={kind}
                content={content}
                field={field}
                setSort={setSortModel}
                sql={data.sql}
              />
            ),
            event: e,
          });
        },
        style: { cursor: "context-menu" },
      },
    } as GridSlotsComponentsProps;
  }, [data, setUpMenu, dbName, kind]);

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
