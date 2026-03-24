import { useCallback, useContext, useEffect, useMemo, useRef, useState } from "react";
import CancelIcon from "@mui/icons-material/Cancel";
import types from "./ApiStruct";
import {
  Box,
  Button,
  Checkbox,
  Chip,
  Divider,
  FormControlLabel,
  IconButton,
  InputAdornment,
  ListItemIcon,
  ListItemText,
  MenuItem,
  MenuList,
  Modal,
  Stack,
  TextField,
  Tooltip,
  Typography,
} from "@mui/material";
import ContentCopyIcon from "@mui/icons-material/ContentCopy";
import CloseIcon from "@mui/icons-material/Close";
import CheckIcon from "@mui/icons-material/Check";
import VisibilityIcon from "@mui/icons-material/Visibility";
import EditIcon from "@mui/icons-material/Edit";
import ArrowCircleDownIcon from "@mui/icons-material/ArrowCircleDown";
import { IoLocation } from "react-icons/io5";
import {
  DataGrid,
  GridCallbackDetails,
  GridPaginationModel,
  GridSlotsComponent,
  GridSlotsComponentsProps,
  GridSortModel,
  useGridApiRef,
} from "@mui/x-data-grid";
import ClearIcon from "@mui/icons-material/Clear";
import {
  EscDownEvent,
  FieldAndGridField,
  keyNotInTable,
  modalHandleClose,
  modalStyle,
  reloadOK,
} from "./Common";
import DeleteForeverIcon from "@mui/icons-material/DeleteForever";
import KeyIcon from "@mui/icons-material/Key";
import ArrowUpwardIcon from "@mui/icons-material/ArrowUpward";
import ArrowDownwardIcon from "@mui/icons-material/ArrowDownward";
import FilterAltIcon from "@mui/icons-material/FilterAlt";
import { FaArrowsAltV } from "react-icons/fa";
import {
  ColumnWidth,
  DataSourceKind,
  handleCopyToClipboard,
  isTypeLargeText,
  ErrAndLoc,
  saveAndSetSize,
  secondConfirm,
  SettingContext,
  validJson,
  SavePageSize,
  textEllipsisStyle,
  quoteDatabaseIdentifier,
  MuiDataGridData,
  extractFieldAndIDFromMouseEvent,
  dataGridStyle,
  pageSizeOptions,
  dataGridheaderFontSize,
  shrinkAble,
  executeSqlWithoutConn,
  TabContext,
  getSupportExportKind,
} from "./helper";
import { FaCopy } from "react-icons/fa6";
import FileCopyIcon from "@mui/icons-material/FileCopy";
import {
  AppColumnMenu,
  ModalHeader,
  ResetColumnWidth,
  TimeDuration,
  ViewRowInModal,
} from "./CommonComp";

import { Position } from "monaco-editor";
import wcmatch from "wildcard-match";
import { MdEditNote } from "react-icons/md";
import {
  AppDataGridPagination,
  AppDataGridToolBar,
  LastFetchTime,
  QueryStatus,
  SetFilterSort,
  SimpleFlexEditor,
  SliderSize,
  StackRowCenter,
  TableAddButton,
} from "./CommonComp2";
import {
  makePutValueInSql,
  makeTableDelete,
  makeTableInsertFn,
  makeTableUpdate,
  showTable,
} from "./handlers";
import { AddTableView } from "./LeftTable";
import { ExportTableOrView } from "./Export";
import { useConfirmMsg, useContextMenu, useNotification } from "./store";

const confirmAboutPrimary = async (
  notWarnPri: boolean,
  fields: types.Field[],
): Promise<boolean> => {
  if (notWarnPri) {
    return true;
  }
  const hasPri = !!fields.find((x) => x.is_primary);
  if (hasPri) {
    return true;
  }
  return secondConfirm({
    title: "No Primary key found.",
    content: "No Primary key found,maybe multi row affected.",
    kind: "error",
  });
};

const getRowId = (e) => e[keyNotInTable];

let EditInModalLanguage = "text";
const editInModalLanguages = [
  { language: "text" },
  { language: "html" },
  { language: "json" },
  { language: "xml" },
] as {
  language: string;
}[];

function EditInModal(props: { content: string; handleModify }) {
  const { content, handleModify } = props;
  const [jsonErr, setJsonErr] = useState<ErrAndLoc>(null);
  const [jsonErrPos, setJsonErrPos] = useState<Position>(null);
  const [language, setLanguage] = useState(EditInModalLanguage);
  const [value, setValue] = useState(content);
  useEffect(() => {
    if (language != "json") {
      setJsonErr(null);
      return;
    }
    validJson(value).then((e) => {
      setJsonErr(e);
    });
  }, [value, language]);

  return (
    <Stack flex={1}>
      <Stack direction="row">
        {editInModalLanguages.map((e) => (
          <Chip
            key={e.language}
            label={e.language}
            color={language == e.language ? "success" : "default"}
            onClick={() => {
              setLanguage(e.language);
              EditInModalLanguage = e.language;
            }}
          />
        ))}
      </Stack>
      <SimpleFlexEditor
        value={content}
        language={language}
        handleValue={(v) => {
          setValue(v);
        }}
        errPos={jsonErrPos}
        barLeft={
          jsonErr ? (
            <Typography
              color="error"
              variant="body1"
              style={{ cursor: "pointer" }}
              onClick={() => {
                if (!jsonErr) {
                  return;
                }
                setJsonErrPos(new Position(jsonErr.line, jsonErr.column));
              }}
            >
              {jsonErr.msg}
            </Typography>
          ) : undefined
        }
      />
      <Divider />
      <Stack direction="row">
        <Button
          onClick={() => {
            handleModify(value);
          }}
        >
          modify
        </Button>
      </Stack>
    </Stack>
  );
}

class EditInModalCellData {
  constructor(
    public row: object,
    public field: string,
    public content: string,
    public rowId: number,
  ) {}
}

enum DatabaseOpKind {
  Update,
  Delete,
}

class DatabaseOperation {
  public constructor(
    public op: DatabaseOpKind,
    public sql: string,
    public tableId: any,
  ) {}
}

function DatabaseOperationView(props: {
  op: DatabaseOperation;
  reload: () => void;
  handleCancel: () => void;
  uuid: string;
  handleLocate: () => void;
}) {
  const { reload, uuid, handleCancel, handleLocate } = props;
  const { sql } = props.op;
  const [loading, setLoading] = useState(false);
  const handleApply = () => {
    setLoading(true);
    executeSqlWithoutConn(uuid, sql)
      .then((row_aff) => {
        showNotification("success", "database operation ok", `row affected:${row_aff}`);
        handleCancel();
        reload();
      })
      .catch((e) => {
        showNotification("error", "update failed", e);
      })
      .finally(() => {
        setLoading(false);
      });
  };

  const { showNotification } = useNotification((s) => s);

  return (
    <Stack direction="row" style={{ alignItems: "center" }}>
      <Tooltip title="cancel this not apply anything">
        <IconButton
          onClick={() => {
            handleCancel();
          }}
        >
          <CloseIcon color="warning" />
        </IconButton>
      </Tooltip>

      <Tooltip title="apply">
        <IconButton loading={loading} onClick={handleApply}>
          <CheckIcon color="primary" />
        </IconButton>
      </Tooltip>

      <Tooltip title="copy sql">
        <IconButton
          onClick={() => {
            handleCopyToClipboard(sql, showNotification);
          }}
        >
          <ContentCopyIcon />
        </IconButton>
      </Tooltip>
      <Tooltip title="locate the row we are editing">
        <IconButton onClick={handleLocate}>
          <IoLocation fontSize="1.4em" />
        </IconButton>
      </Tooltip>
      <Typography color="info" style={textEllipsisStyle}>
        {sql}
      </Typography>
    </Stack>
  );
}

function tryGetId(data: object, fields: types.Field[]) {
  let pf = fields.filter((e) => e.is_primary);
  if (pf.length == 0) {
    pf = fields;
  }
  return pf.map((x) => data[x.field]).join("-");
}
function SqlTableView(props: {
  notWarnPri?: boolean;
  kind: DataSourceKind;
  tableName: string;
  uuid;
  isTable: boolean;
}) {
  const { notWarnPri, kind, tableName, uuid, isTable } = props;
  const getUpdateSql = makeTableUpdate(uuid, tableName);
  const getDeleteSql = makeTableDelete(uuid, tableName);
  const put_value_in_sql = makePutValueInSql(uuid);
  const addOP = makeTableInsertFn(uuid, tableName);
  const [loading, setLoading] = useState(false);
  const [where, setWhere] = useState("");
  const [showSetWhere, setShowSetWhere] = useState(false);
  const [sort, setSort] = useState("");
  const [showSetSort, setShowSetSort] = useState(false);
  const setting = useContext(SettingContext);
  const [page, setPage] = useState(0);
  const [pageSize, setPageSize] = useState(SavePageSize.instance.get());
  const [resp, setResp] = useState<MuiDataGridData>({
    sql: "",
    fields: [],
    data: [],
    total: 0,
  });
  const apiRef = useGridApiRef();
  const [lastFetchTime, setLastFetchTime] = useState(new Date());
  useEffect(() => {
    setLastFetchTime(new Date());
  }, [resp]);
  const [ops, setOps] = useState<DatabaseOperation[]>([]);

  const tabCtx = useContext(TabContext);
  const { removeConfirmMsg, addConfirmMsg } = useConfirmMsg((state) => state);

  useEffect(() => {
    if (ops.length == 0) {
      removeConfirmMsg(tabCtx.id);
    } else {
      addConfirmMsg(tabCtx.id, `sure to discard ${ops.length} changes?`);
    }
  }, [ops, tabCtx.id, removeConfirmMsg, addConfirmMsg]);
  useEffect(() => {
    removeConfirmMsg(tabCtx.id);
  }, [removeConfirmMsg, tabCtx.id]);

  const [fetchDur, setFetchDur] = useState(0);
  const clientModelLimit = useRef<number>(1000);
  const { showNotification } = useNotification((s) => s);
  const [allowFiltering, setAllowFiltering] = useState(false);
  const [editCellInModal, setEditCellInModal] = useState<EditInModalCellData>(null);
  const [editRowInModal, setRowEditInModal] = useState<object>(null);
  const clientModel = [DataSourceKind.Cassandra].includes(kind);
  const [viewInModal, setViewInModal] = useState<object>(null);
  const [showExport, setShowExport] = useState(false);
  const [showAdd, setShowAdd] = useState(false);
  const [queryStatus, setQueryStatus] = useState("");
  const [clickedField, setClickedField] = useState("");
  const [rowBeenLocate, setRowBeenLocate] = useState("");

  const handleResp = (resp: MuiDataGridData) => {
    resp.data.forEach((data, index) => {
      data[keyNotInTable] = index + "";
    });
    resp.fields.forEach((field) => {
      const width = ColumnWidth.instance.get(field.field);
      if (width) {
        field["width"] = width;
      }
    });
    for (let i = 0; i < resp.fields.length; i++) {
      const field = resp.fields[i] as FieldAndGridField;
      field.renderHeader = (e) => {
        const field = e.colDef as unknown as FieldAndGridField;
        const makeTitle = (): string => {
          let ret = field.field + " ";
          ret += field.data_type;
          if (field.nullable) {
            ret += " null";
          } else {
            ret += " not null";
          }
          if (field.is_auto_increment) {
            ret += " auto increment";
          }
          if (field.default_value) {
            ret += " " + field.default_value;
          }
          return ret;
        };
        return (
          <Stack
            onClick={(e) => {
              e.stopPropagation();
              setClickedField(field.field);
            }}
            style={{ alignItems: "center", cursor: "default" }}
            direction="row"
          >
            <Tooltip title={makeTitle()}>
              <Typography fontSize={dataGridheaderFontSize}>{field.field}</Typography>
            </Tooltip>
            {field.is_primary && <KeyIcon style={{ fontSize: "16px" }} />}
          </Stack>
        );
      };
      if (!field.is_auto_increment) {
        field.editable = true;
      }
    }
    setResp(resp);
  };

  const makePara = useCallback(() => {
    const where_str = where.trim();
    let order_by = sort.trim();
    return {
      where_str: where_str ? where_str : null,
      order_by: order_by ? order_by : null,
      page: page + 1,
      page_size: pageSize,
      allow_filtering: allowFiltering,
      limit: clientModelLimit.current,
    } as types.ShowTableParameter;
  }, [allowFiltering, page, pageSize, sort, where]);
  const handleReload = useCallback(
    (showOK: boolean) => {
      switch (kind) {
        case DataSourceKind.Cassandra:
          break;
      }
      showTable({
        para: makePara(),
        setStatus: setQueryStatus,
        name: tableName,
        isView: !isTable,
        uuid,
        setLoading,
        handleExecuteTime: (dur) => {
          setFetchDur(dur);
        },
        okCallBack: showOK
          ? () => {
              if (showOK) {
                reloadOK(showNotification);
              }
            }
          : undefined,
        showNotification,
        handleResult: (resp) => {
          handleResp(resp);
        },
      });
    },
    [isTable, kind, makePara, showNotification, tableName, uuid],
  );

  const rowHasOP = useCallback(
    (row: object) => {
      let id = tryGetId(row, resp.fields);

      return ops.find((e) => e.tableId == id);
    },
    [ops, resp.fields],
  );

  const handleProcessRowUpdate = useCallback(
    async (newRow: object, oldRow: object) => {
      if (rowHasOP(oldRow)) {
        showNotification("warning", "row is modifying");
        return oldRow;
      }
      for (const key in newRow) {
        if (!newRow[key]) {
          newRow[key] = null;
        }
      }
      const findDiff = (): Map<string, string | null> => {
        const m = new Map();
        for (const key in newRow) {
          if (newRow[key] != oldRow[key]) {
            m.set(key, newRow[key]);
          }
        }
        return m;
      };
      const updateRow = findDiff();
      const old = new Map(Object.entries(oldRow));
      if (updateRow.size == 0) {
        return newRow;
      }
      if (!(await confirmAboutPrimary(notWarnPri, resp.fields))) {
        return oldRow;
      }
      try {
        const sql = await getUpdateSql(old, updateRow);
        setOps((e) => [
          ...e,
          new DatabaseOperation(
            DatabaseOpKind.Update,
            sql,
            tryGetId(oldRow, resp.fields),
          ),
        ]);
      } catch (e) {
        showNotification("error", "generate update sql failed", e);
        throw e;
      }
      return oldRow;
    },
    [rowHasOP, notWarnPri, resp, showNotification, getUpdateSql],
  );

  const handleDelete = useCallback(
    async (id) => {
      if (!(await confirmAboutPrimary(notWarnPri, resp.fields))) {
        return;
      }
      let old;
      for (const e of resp.data) {
        if (e[keyNotInTable] == id) {
          old = { ...e };
          break;
        }
      }
      if (!old) {
        showNotification(
          "error",
          "not found the record to delete this is a internal bug",
        );
        return;
      }
      if (rowHasOP(old)) {
        showNotification("warning", "row is modifying");
        return;
      }
      try {
        let old2 = { ...old };
        const sql = await getDeleteSql(old2);
        setOps((e) => [
          ...e,
          new DatabaseOperation(DatabaseOpKind.Delete, sql, tryGetId(old, resp.fields)),
        ]);
      } catch (e) {
        showNotification("error", "generate delete sql failed", e);
      }
    },
    [getDeleteSql, notWarnPri, resp.data, resp.fields, rowHasOP, showNotification],
  );
  useEffect(() => {
    handleReload(false);
  }, [page, pageSize, where, sort, allowFiltering, handleReload]);

  const paginationModel = useMemo(() => {
    return { page, pageSize };
  }, [page, pageSize]);
  const onPaginationModelChange = useCallback(
    (e: GridPaginationModel, reason: GridCallbackDetails<"pagination">) => {
      setPage(e.page);
      setPageSize(e.pageSize);
      if (reason.reason == "setPaginationModel") {
        SavePageSize.instance.save(e.pageSize);
      }
    },
    [],
  );
  const setUpMenu = useContextMenu((state) => state.setUpMenu);
  const onSortModelChange = useCallback(
    (sorts: GridSortModel) => {
      if (sorts.length == 0) {
        setSort("");
        return;
      }
      const sort = sorts[0];
      if (!sort.sort) {
        setSort("");
        return;
      }
      setSort(
        `${quoteDatabaseIdentifier(kind)(sort.field)} ${sort.sort.toUpperCase()}`,
      );
    },
    [kind],
  );

  const onProcessRowUpdateError = useCallback(
    (e) => {
      showNotification("error", "update failed", e);
    },
    [showNotification],
  );

  const onColumnResize = useCallback(({ colDef, width }) => {
    ColumnWidth.instance.save(colDef.field, width);
  }, []);
  const slotProps = useMemo(() => {
    return {
      row: {
        onContextMenu: (e) => {
          const { id, field } = extractFieldAndIDFromMouseEvent(e);
          if (!field) {
            return;
          }
          const row = { ...resp.data.find((e) => e[keyNotInTable] == id) };
          const content = row[field];
          const nullable = resp.fields.find((e) => e.field == field).nullable;
          setUpMenu({
            content: (
              <Menu
                kind={kind}
                editCellInModal={() => {
                  setEditCellInModal(
                    new EditInModalCellData(row, field, row[field], parseInt(id)),
                  );
                }}
                editRowInModal={() => {
                  setRowEditInModal(row);
                }}
                isTable={isTable}
                nullable={nullable}
                fields={resp.fields}
                field={field}
                content={content}
                handleDelete={
                  isTable
                    ? () => {
                        handleDelete(id);
                      }
                    : undefined
                }
                row={row}
                addOP={isTable && addOP}
                setSort={setSort}
                setWhere={setWhere}
                put_value_in_sql={(value) => {
                  return put_value_in_sql(
                    resp.fields.find((e) => e.field == field).data_type,
                    value,
                  );
                }}
                setNull={
                  isTable
                    ? async () => {
                        const row = resp.data.find((e) => e[keyNotInTable] == id);
                        let newRow = { ...row };
                        newRow[field] = null;
                        await handleProcessRowUpdate(newRow, row);
                      }
                    : undefined
                }
                viewInModal={() => {
                  setViewInModal(row);
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
  }, [
    setting.export.csvDelimiter,
    setting.export.includeHeader,
    resp.data,
    resp.fields,
    setUpMenu,
    kind,
    isTable,
    addOP,
    handleDelete,
    put_value_in_sql,
    handleProcessRowUpdate,
  ]);

  const [editInModalBox, setEditInModalBox] = useState(null);
  useEffect(() => {
    if (editInModalBox) {
      const x1 = saveAndSetSize(editInModalBox, "edit-cell");
      return () => {
        x1();
      };
    }
  }, [editInModalBox]);
  const [editRowInModalBox, setEditRowInModalBox] = useState(null);
  useEffect(() => {
    if (editRowInModalBox) {
      const x1 = saveAndSetSize(editRowInModalBox, "edit-row", "width");
      return () => {
        x1();
      };
    }
  }, [editRowInModalBox]);
  const [viewInModalBox, setViewInModalBox] = useState(null);
  useEffect(() => {
    if (viewInModalBox) {
      const x1 = saveAndSetSize(viewInModalBox, "view-row-in-modal");
      return () => {
        x1();
      };
    }
  }, [viewInModalBox]);

  const [addModalBox, setAddModalBox] = useState(null);
  useEffect(() => {
    if (addModalBox) {
      const x1 = saveAndSetSize(addModalBox, "table-insert");
      return () => {
        x1();
      };
    }
  }, [addModalBox]);
  const slots = useMemo(() => {
    return {
      columnMenu: AppColumnMenu,
      toolbar: () => {
        return (
          <AppDataGridToolBar
            title={tableName}
            right=<>
              <IconButton
                onClick={() => {
                  setShowExport(true);
                }}
              >
                <ArrowCircleDownIcon />
              </IconButton>
            </>
            left=<StackRowCenter>
              <Button
                loading={loading}
                color="primary"
                onClick={() => {
                  handleReload(true);
                }}
              >
                reload
              </Button>
              <Tooltip title={where ? where : "filter something like 'age=18'"}>
                <Chip
                  label={where}
                  onClick={() => {
                    setShowSetWhere(true);
                  }}
                  style={{ ...shrinkAble, maxWidth: "14vw", textOverflow: "clip" }}
                  size="medium"
                  color="primary"
                  icon={<FilterAltIcon />}
                  onDelete={
                    where
                      ? () => {
                          setWhere("");
                        }
                      : undefined
                  }
                />
              </Tooltip>
              <Tooltip title={sort ? sort : "sort something like 'age asc'"}>
                <Chip
                  label={sort}
                  onClick={() => {
                    setShowSetSort(true);
                  }}
                  style={{ ...shrinkAble, maxWidth: "12vw", textOverflow: "clip" }}
                  size="medium"
                  color="secondary"
                  variant="outlined"
                  onDelete={
                    sort
                      ? () => {
                          setSort("");
                        }
                      : undefined
                  }
                  icon={<FaArrowsAltV />}
                />
              </Tooltip>
              {kind == DataSourceKind.Cassandra && (
                <>
                  <FormControlLabel
                    control={
                      <Checkbox
                        checked={allowFiltering}
                        onChange={(e) => setAllowFiltering(e.target.checked)}
                      />
                    }
                    label="allow filtering"
                  />
                </>
              )}
              {clientModel && (
                <SliderSize
                  notice=<Typography>
                    some backend not support pagination like sql,cassandra etc,we use
                    client model.
                  </Typography>
                  min={1000}
                  max={100000}
                  storeKey="sqltable-limit"
                  step={1000}
                  handleValue={(v) => {
                    clientModelLimit.current = v;
                  }}
                />
              )}
              {isTable && (
                <TableAddButton
                  handleAdd={() => {
                    setShowAdd(true);
                  }}
                />
              )}
            </StackRowCenter>
          />
        );
      },
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
            <Stack direction="row" flex={1} style={{ alignItems: "center" }}>
              <LastFetchTime date={lastFetchTime} />
              <TimeDuration dur={fetchDur} />
              <ResetColumnWidth />
              {resp.sql && (
                <Tooltip title={resp.sql}>
                  <Typography
                    style={{ cursor: "pointer" }}
                    onClick={() => {
                      handleCopyToClipboard(resp.sql, showNotification);
                    }}
                    color="info"
                  >
                    sql
                  </Typography>
                </Tooltip>
              )}
            </Stack>
            <Stack direction="row" alignItems="center">
              {!clientModel && (
                <Typography color="info">
                  result {resp.data.length} of {resp.total}
                </Typography>
              )}
              <AppDataGridPagination showPageSelect />
            </Stack>
          </Stack>
        );
      },
    } as Partial<GridSlotsComponent>;
  }, [
    allowFiltering,
    clientModel,
    fetchDur,
    handleReload,
    isTable,
    kind,
    lastFetchTime,
    loading,
    resp.data.length,
    resp.sql,
    resp.total,
    showNotification,
    sort,
    tableName,
    where,
  ]);

  const rowBeenModify = (id, kind: DatabaseOpKind) => {
    if (!id) {
      return false; // this id belong to row data
    }
    return ops.findIndex((e) => e.tableId == id && e.op == kind) != -1;
  };

  const sortModel = clientModel ? "client" : "server";
  return (
    <Stack style={{ height: "100%" }}>
      <QueryStatus status={queryStatus} />
      {ops.map((operation, idx) => (
        <DatabaseOperationView
          op={operation}
          handleCancel={() => {
            setOps((ops) => ops.filter((_, index) => idx != index));
          }}
          uuid={uuid}
          key={idx}
          reload={() => {
            handleReload(false);
          }}
          handleLocate={() => {
            if (!operation.tableId) {
              showNotification(
                "warning",
                "the table don't contains valid primary key to locate",
              );
              return;
            }
            let index = resp.data.findIndex(
              (e) => tryGetId(e, resp.fields) == operation.tableId,
            );
            if (index != -1) {
              apiRef.current.scrollToIndexes({ rowIndex: index });
              setRowBeenLocate(operation.tableId);
              setTimeout(() => {
                setRowBeenLocate("");
              }, 10_000);
            } else {
              showNotification(
                "warning",
                "the data we are looking for may not present at this page",
                operation.tableId,
              );
            }
          }}
        />
      ))}
      <DataGrid
        slots={slots}
        getCellClassName={(e) => {
          let id = tryGetId(e.row, resp.fields);
          if (id == rowBeenLocate) {
            return "datagrid-field-row-highlight";
          }
          if (rowBeenModify(id, DatabaseOpKind.Update)) {
            return "datagrid-field-been-modify";
          }
          if (rowBeenModify(id, DatabaseOpKind.Delete)) {
            return "datagrid-field-been-delete";
          }
          if (e.field == clickedField) {
            return "datagrid-field-clicked";
          } else {
            return "";
          }
        }}
        apiRef={apiRef}
        onCellClick={() => {
          setClickedField("");
        }}
        slotProps={slotProps}
        style={dataGridStyle}
        getRowId={getRowId}
        paginationModel={clientModel ? undefined : paginationModel}
        pageSizeOptions={pageSizeOptions}
        paginationMode={sortModel}
        onPaginationModelChange={clientModel ? undefined : onPaginationModelChange}
        checkboxSelection={false}
        rowCount={clientModel ? resp.data.length : resp.total}
        sortingMode={clientModel ? "client" : "server"}
        onSortModelChange={clientModel ? undefined : onSortModelChange}
        columns={resp.fields}
        rows={resp.data}
        onColumnResize={onColumnResize}
        processRowUpdate={isTable ? handleProcessRowUpdate : undefined}
        onProcessRowUpdateError={onProcessRowUpdateError}
        loading={loading}
      />
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
            <ViewRowInModal fields={resp.fields} data={viewInModal} isTable={isTable} />
          </Box>
        </Modal>
      )}
      {showAdd && (
        <Modal
          open={showAdd}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setShowAdd(false);
          })}
        >
          <Box
            sx={{ resize: "both", ...modalStyle }}
            ref={setAddModalBox}
            width="80%"
            height="30vh"
          >
            <ModalHeader
              title="insert data"
              handleClose={() => {
                setShowAdd(false);
              }}
            />
            <AddTableView uuid={uuid} table={tableName} kind={kind} />
          </Box>
        </Modal>
      )}
      {editCellInModal && (
        <Modal
          open={!!editCellInModal}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setEditCellInModal(null);
          })}
        >
          <Box
            sx={{ resize: "both", ...modalStyle }}
            ref={setEditInModalBox}
            width="80%"
            height="30vh"
          >
            <ModalHeader
              title="edit cell"
              handleClose={() => {
                setEditCellInModal(null);
              }}
            />
            <EditInModal
              content={editCellInModal.content}
              handleModify={async (value) => {
                const newRow = { ...editCellInModal.row };
                newRow[editCellInModal.field] = value;
                await handleProcessRowUpdate(newRow, editCellInModal.row);
                setEditCellInModal(null);
              }}
            />
          </Box>
        </Modal>
      )}
      {editRowInModal && (
        <Modal
          open={!!editRowInModal}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setRowEditInModal(null);
          })}
        >
          <Box
            sx={{ resize: "horizontal", ...modalStyle }}
            ref={setEditRowInModalBox}
            width="80%"
          >
            <ModalHeader
              title="edit row"
              handleClose={() => {
                setRowEditInModal(null);
              }}
            />
            <SqlTableViewData
              initValues={editRowInModal}
              buttonName="modify"
              fields={resp.fields}
              isAdd={false}
              handleData={async (args) => {
                await handleProcessRowUpdate(args, editRowInModal);
              }}
              handleClose={() => setRowEditInModal(null)}
            />
          </Box>
        </Modal>
      )}
      {showExport && (
        <Modal
          open={showExport}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setShowExport(false);
          })}
        >
          <Box sx={modalStyle} width="60%">
            <ExportTableOrView
              uuid={uuid}
              name={tableName}
              supportKind={getSupportExportKind(kind, isTable)}
              handleClose={() => {
                setShowExport(false);
              }}
              para={makePara()}
            />
          </Box>
        </Modal>
      )}
      {showSetWhere && (
        <SetFilterSort
          tableName={tableName}
          open={showSetWhere}
          value={where}
          setValue={(v) => {
            setWhere(v);
          }}
          handleClose={() => {
            setShowSetWhere(false);
          }}
          label="filter"
          kind={kind}
          uuid={uuid}
        />
      )}
      {showSetSort && (
        <SetFilterSort
          tableName={tableName}
          open={showSetSort}
          value={sort}
          setValue={(v) => {
            setSort(v);
          }}
          kind={kind}
          handleClose={() => {
            setShowSetSort(false);
          }}
          label="sort"
          uuid={uuid}
        />
      )}
    </Stack>
  );
}

let SqlTableViewDataMatch = "";

function SqlTableViewData(props: {
  handleData: (args: any) => Promise<void>;
  fields: types.Field[];
  initValues?: object;
  buttonName;
  isAdd: boolean;
  handleClose?: () => void;
}) {
  let { handleData, fields, initValues, buttonName, isAdd, handleClose } = props;
  if (!initValues) {
    initValues = {};
  }
  fields.forEach((e) => {
    if (e.is_auto_increment && isAdd) {
      initValues[e.field] = "";
    }
  });
  const [filter, setFilter] = useState(SqlTableViewDataMatch);

  const filterFn = useMemo(() => {
    SqlTableViewDataMatch = filter;
    return filter
      ? wcmatch("*" + filter + "*", { flags: "i" })
      : () => {
          return true;
        };
  }, [filter]);

  const [loading, setLoading] = useState(false);
  const [args, setArgs] = useState(initValues);
  const { showNotification } = useNotification((s) => s);
  const inputDoms = useRef({});

  const handleModify = () => {
    const argsclone = { ...args };
    for (let i = 0; i < fields.length; i++) {
      const f = fields[i];
      if (f.is_auto_increment) {
        continue;
      }
      if (!argsclone[f.field] && !f.nullable && !f.default_value) {
        inputDoms.current[f.field].focus();
        showNotification("error", `filed '${f.field}' need be filled`);
        return;
      }
    }
    for (const key in argsclone) {
      if (!argsclone[key]) {
        // make sure '' convert to null
        argsclone[key] = null;
      }
    }
    setLoading(true);
    handleData(argsclone)
      .then(() => {
        console.log("handle value success");
        showNotification("success", `${buttonName} OK`);
        if (handleClose) {
          handleClose();
        }
      })
      .catch((e) => {
        console.error("handle value failure", e);
        showNotification("error", `${buttonName} failure`, e);
      })
      .finally(() => {
        setLoading(false);
      });
  };

  return (
    <Stack>
      {fields
        .filter(
          (f) =>
            filterFn(f.field) || (args[f.field] ? filterFn(args[f.field]) : !filter),
        )
        .map((f) => {
          return (
            <Stack
              key={f.field}
              direction="row"
              sx={{ alignContent: "center", justifyContent: "center" }}
            >
              <TextField
                inputRef={(x) => {
                  inputDoms.current[f.field] = x;
                }}
                disabled={f.is_auto_increment}
                value={args[f.field]}
                helperText={
                  f.data_type +
                  " " +
                  (f.nullable ? "null" : "not null") +
                  " " +
                  (f.is_auto_increment ? "auto increment" : "") +
                  " " +
                  (f.default_value ? `default ${f.default_value}` : "")
                }
                required={!f.nullable}
                label={f.field}
                multiline={isTypeLargeText(f.data_type)}
                rows={4}
                fullWidth
                onChange={(e) => {
                  const value = e.target.value;
                  const newArgs = { ...args };
                  newArgs[f.field] = value;
                  setArgs(newArgs);
                }}
              />
            </Stack>
          );
        })}
      <Divider />
      <Stack direction="row" style={{ alignItems: "flex-start" }}>
        <Button loading={loading} onClick={handleModify}>
          {buttonName}
        </Button>
        <TextField
          value={filter}
          onChange={(e) => {
            setFilter(e.target.value);
          }}
          label="filter"
          fullWidth
          helperText="can match field name and content"
          slotProps={{
            input: {
              endAdornment: filter && (
                <InputAdornment position="end">
                  <IconButton
                    onClick={() => {
                      setFilter("");
                    }}
                    edge="end"
                  >
                    <ClearIcon fontSize="small" />
                  </IconButton>
                </InputAdornment>
              ),
            },
          }}
        />
      </Stack>
    </Stack>
  );
}

function Menu(props: {
  field: string;
  content: string;
  handleDelete?: () => void;
  setSort;
  setWhere;
  put_value_in_sql: (value?: string) => Promise<string>;
  setNull?: () => void;
  addOP?: (x: object) => Promise<void>;
  row: object;
  fields: types.Field[];
  nullable: boolean;
  editCellInModal: () => void;
  editRowInModal: () => void;
  kind: DataSourceKind;
  viewInModal: () => void;
  isTable: boolean;
}) {
  const {
    field,
    content,
    handleDelete,
    setSort,
    setWhere,
    put_value_in_sql,
    setNull,
    addOP,
    row,
    fields,
    nullable,
    editCellInModal,
    editRowInModal,
    kind,
    viewInModal,
    isTable,
  } = props;
  const { showNotification } = useNotification((s) => s);
  const [showFilter, setShowFilter] = useState(false);
  const quoteFn = quoteDatabaseIdentifier(kind);
  const [showClone, setShowClone] = useState(false);
  const quoteField = quoteFn(field);
  const [cloneModalBox, setCloneModalBox] = useState(null);
  useEffect(() => {
    if (cloneModalBox) {
      const clean = saveAndSetSize(cloneModalBox, "clone-data", "width");
      return () => {
        clean();
      };
    }
  }, [cloneModalBox]);
  return (
    <div>
      <MenuList>
        {isTable && (
          <MenuItem
            onClick={() => {
              editCellInModal();
            }}
          >
            <ListItemIcon>
              <EditIcon />
            </ListItemIcon>
            <ListItemText> Edit Cell in Modal </ListItemText>
          </MenuItem>
        )}

        {isTable && (
          <MenuItem
            onClick={() => {
              editRowInModal();
            }}
          >
            <ListItemIcon>
              <MdEditNote fontSize="1.7rem" />
            </ListItemIcon>
            <ListItemText> Edit Row in Modal </ListItemText>
          </MenuItem>
        )}

        {nullable && isTable && setNull && (
          <MenuItem
            onClick={() => {
              setNull();
            }}
          >
            <ListItemIcon>
              <CancelIcon />
            </ListItemIcon>
            <ListItemText> Set As Null </ListItemText>
          </MenuItem>
        )}

        <MenuItem onClick={viewInModal}>
          <ListItemIcon>
            <VisibilityIcon style={{ fontSize: "1.4rem" }} />
          </ListItemIcon>
          <ListItemText> View in Modal </ListItemText>
        </MenuItem>

        {content && (
          <MenuItem
            onClick={() => {
              handleCopyToClipboard(content, showNotification);
            }}
          >
            <ListItemIcon>
              <FaCopy fontSize="1.4rem" />
            </ListItemIcon>
            <ListItemText> Copy </ListItemText>
          </MenuItem>
        )}

        <Divider />
        <MenuItem
          onClick={() => {
            setSort(`${quoteField} ASC`);
          }}
        >
          <ListItemIcon>
            <ArrowUpwardIcon />
          </ListItemIcon>
          <ListItemText> {field} ASC </ListItemText>
        </MenuItem>
        <MenuItem
          onClick={() => {
            setSort(`${quoteField} DESC`);
          }}
        >
          <ListItemIcon>
            <ArrowDownwardIcon />
          </ListItemIcon>
          <ListItemText> {field} DESC </ListItemText>
        </MenuItem>

        {content && (
          <MenuItem
            onClick={() => {
              setShowFilter(true);
            }}
          >
            <ListItemIcon>
              <FilterAltIcon />
            </ListItemIcon>
            <ListItemText> Quick Filter </ListItemText>
          </MenuItem>
        )}
        {isTable && <Divider />}
        {isTable && (
          <MenuItem
            onClick={() => {
              setShowClone(true);
            }}
          >
            <ListItemIcon>
              <FileCopyIcon />
            </ListItemIcon>
            <ListItemText> Clone Row </ListItemText>
          </MenuItem>
        )}
        {isTable && (
          <MenuItem onClick={handleDelete}>
            <ListItemIcon>
              <DeleteForeverIcon />
            </ListItemIcon>
            <ListItemText> Delete Row</ListItemText>
          </MenuItem>
        )}
      </MenuList>

      {showClone && (
        <Modal
          open={showClone}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setShowClone(false);
          })}
        >
          <Box
            sx={{ resize: "horizontal", ...modalStyle }}
            width="50%"
            ref={setCloneModalBox}
          >
            <ModalHeader
              title="Clone Data"
              handleClose={() => {
                setShowClone(false);
              }}
            />
            <SqlTableViewData
              buttonName="add"
              handleData={addOP}
              fields={fields}
              handleClose={() => setShowClone(false)}
              initValues={row}
              isAdd
            />
          </Box>
        </Modal>
      )}

      {showFilter && (
        <Modal
          open={showFilter}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setShowFilter(false);
          })}
        >
          <Box sx={modalStyle} width="45vw">
            <ModalHeader
              title="Quick Filter"
              handleClose={() => {
                setShowFilter(false);
              }}
            />
            <FilterOnValue
              content={content}
              setWhere={setWhere}
              field={field}
              kind={kind}
              handleClose={() => {
                setShowFilter(false);
              }}
              put_value_in_sql={(value) => {
                return put_value_in_sql(value);
              }}
            />
          </Box>
        </Modal>
      )}
    </div>
  );
}

let lastCompare = "=";
function FilterOnValue(props: {
  content: string;
  setWhere;
  field: string;
  handleClose;
  kind: DataSourceKind;
  put_value_in_sql: (x: string) => Promise<string>;
}) {
  const { content, setWhere, field, handleClose, put_value_in_sql, kind } = props;
  const compares = ["=", "<>", ">", ">=", "<", "<="];
  const [compare, setCompare] = useState(lastCompare);
  const { showNotification } = useNotification((s) => s);
  const [formatValue, setFormatValue] = useState("");
  useEffect(() => {
    lastCompare = compare;
  }, [compare]);
  useEffect(() => {
    put_value_in_sql(content)
      .then((value) => {
        setFormatValue(value);
      })
      .catch((e) => {
        showNotification("error", "put value in sql failed", e);
      });
  }, [content, put_value_in_sql, showNotification]);

  const where = () => {
    const quoteField = quoteDatabaseIdentifier(kind)(field);
    if (compare == "null") {
      return `${quoteField} IS NULL`;
    }
    if (compare == "not null") {
      return `${quoteField} IS NOT NULL`;
    }
    if (compare == "starts") {
      return `${quoteField} LIKE '${content}%'`;
    }
    if (compare == "ends") {
      return `${quoteField} LIKE '%${content}'`;
    }
    if (compare == "contains") {
      return `${quoteField} LIKE '%${content}%'`;
    }
    return `${quoteField} ${compare} ${formatValue}`;
  };
  return (
    <Stack spacing={1}>
      <Stack direction="row">
        {compares.map((x) => (
          <Chip
            key={x}
            onClick={() => {
              setCompare(x);
            }}
            label={x}
            color={compare == x ? "success" : "default"}
            style={{ minWidth: "3rem" }}
          />
        ))}
      </Stack>
      <Stack direction="row">
        {["null", "not null"].map((e) => (
          <Chip
            label={e}
            color={compare == e ? "success" : "default"}
            onClick={() => {
              setCompare(e);
            }}
            key={e}
          />
        ))}
      </Stack>
      {formatValue.endsWith("'") && (
        <Stack direction="row">
          {["starts", "ends", "contains"].map((e) => (
            <Chip
              label={e}
              color={compare == e ? "success" : "default"}
              onClick={() => {
                setCompare(e);
              }}
              key={e}
            />
          ))}
        </Stack>
      )}

      <Divider />
      <StackRowCenter>
        <Button
          onClick={() => {
            setWhere(where());
            handleClose();
          }}
        >
          set
        </Button>
        <Typography color="success" style={textEllipsisStyle}>
          <em> {where()} </em>
        </Typography>
      </StackRowCenter>
    </Stack>
  );
}

export { SqlTableView, SqlTableViewData };
