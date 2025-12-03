import { useCallback, useContext, useEffect, useMemo, useRef, useState } from "react";
import CancelIcon from "@mui/icons-material/Cancel";
import types from "./ApiStruct";
import {
  Autocomplete,
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
import VisibilityIcon from "@mui/icons-material/Visibility";
import EditIcon from "@mui/icons-material/Edit";
import { NotificationContext } from "./Notification";
import SaveIcon from "@mui/icons-material/Save";
import {
  DataGrid,
  GridCallbackDetails,
  GridPaginationModel,
  GridSlotsComponent,
  GridSlotsComponentsProps,
  GridSortModel,
} from "@mui/x-data-grid";
import ClearIcon from "@mui/icons-material/Clear";
import {
  defaultShowTableParams,
  EscDownEvent,
  FieldAndGridField,
  keyNotInTable,
  makeFormatFn,
  modalHandleClose,
  modalStyle,
  NameAndQueryId,
  reloadOK,
  SetAutoCompletionName,
  TableFilter,
  TableFilterHandler,
} from "./Common";
import DeleteForeverIcon from "@mui/icons-material/DeleteForever";
import KeyIcon from "@mui/icons-material/Key";
import ArrowUpwardIcon from "@mui/icons-material/ArrowUpward";
import ArrowDownwardIcon from "@mui/icons-material/ArrowDownward";
import FilterAltIcon from "@mui/icons-material/FilterAlt";
import { FaArrowsAltV } from "react-icons/fa";
import {
  ColumnWidth,
  ContextMenuContext,
  DataSourceKind,
  handleCopyToClipboard,
  isTypeLargeText,
  ErrAndLoc,
  resetMuiDataRespWidth,
  saveAndSetSize,
  secondConfirm,
  SettingContext,
  validJson,
  SavePageSize,
  textEllipsisStyle,
  quoteDatabaseIdentifier,
  MuiDataGridData,
  extractFieldAndIDFromMouseEvent,
  editorPositionToEnd,
  dataGridStyle,
  pageSizeOptions,
  getLanguageFromDataSourceKind,
  dataGridheaderFontSize,
  shrinkAble,
} from "./helper";
import { FaCopy } from "react-icons/fa6";
import FileCopyIcon from "@mui/icons-material/FileCopy";
import {
  AppColumnMenu,
  ExecutingTime,
  ExportMarkDownTable,
  MenuExportMarkDownTable,
  MenuResetColumnWidths,
  ModalHeader,
  ViewRowInModal,
} from "./CommonComp";
import { editor, Position } from "monaco-editor";
import wcmatch from "wildcard-match";
import { MdEditNote } from "react-icons/md";
import { SimpleFlexEditor } from "./CommonComp2";
import {
  makePutValueInSql,
  makeTableDelete,
  makeTableInsertFn,
  makeTableUpdate,
  showTable,
} from "./handlers";
import { invoke } from "@tauri-apps/api/core";

const confirmAboutPrimary = async (
  notWarnPri: boolean,
  fields: types.Field[],
): Promise<boolean> => {
  if (notWarnPri) {
    return true;
  }
  const hasPri = !!fields.find(x => x.is_primary);
  if (hasPri) {
    return true;
  }
  return secondConfirm({
    title: "No Primary key found.",
    content: "No Primary key found,maybe multi row affected.",
    kind: "error",
  });
};

const getRowId = e => e[keyNotInTable];

function SetFilterSort(props: {
  open: boolean;
  handleClose: () => void;
  value: string;
  setValue;
  label: "filter" | "sort";
  tableName: string;
  kind: DataSourceKind;
  uuid: string;
}) {
  let { open, handleClose, value, setValue, label, tableName, kind, uuid } = props;
  const isFilter = label == "filter";
  const extractFn = isFilter
    ? makeExtractTableSelectWhere(uuid)
    : makeExtractTableSelectSort(uuid);
  tableName = quoteDatabaseIdentifier(kind)(tableName);
  const [editor, setEditor] = useState<editor.IStandaloneCodeEditor>(null);
  const [modalBox, setModalBox] = useState(null);
  const { showNotification } = useContext(NotificationContext);
  const setting = useContext(SettingContext);
  useEffect(() => {
    if (modalBox) {
      const clean = saveAndSetSize(modalBox, "where-sort");
      return clean;
    }
  }, [modalBox]);

  const [selectFilter, setSelectFilter] = useState("");
  const [filters, setFileters] = useState<TableFilter[]>([]);
  const loadFilter = useCallback(async () => {
    const x = await TableFilterHandler.get(uuid, tableName);
    if (x) {
      setFileters(x);
    }
  }, [tableName, uuid]);
  useEffect(() => {
    if (!isFilter) {
      return;
    }
    loadFilter();
  }, [isFilter, loadFilter]);

  const formatarg = {
    tabSize: setting.editor.tabSize,
    insertSpaces: true,
  };

  const handleSet = async () => {
    try {
      let value = await extractFn(editor.getValue());
      if (value == null) {
        value = "";
      }
      setValue(value);
      handleClose();
    } catch (e) {
      showNotification("error", "extract sql error.", e);
      return;
    }
  };
  return (
    <Modal
      open={open}
      onClose={modalHandleClose((e: EscDownEvent) => {
        e.preventDefault();
        handleClose();
      })}
    >
      <Box
        sx={{ resize: "both", ...modalStyle }}
        width="75%"
        height="30vh"
        ref={setModalBox}
      >
        <ModalHeader title={`set ${label}`} handleClose={handleClose} />
        <SimpleFlexEditor
          value={
            makeFormatFn(kind)(
              isFilter
                ? `SELECT * FROM ${tableName} WHERE ${value}`
                : `SELECT * FROM ${tableName} ORDER BY ${value}`,
              formatarg,
            ) + " "
          }
          handleEditor={e => {
            setEditor(e);
            e.focus();
            editorPositionToEnd(e);
            SetAutoCompletionName.set(e.getModel().id, new NameAndQueryId(uuid, kind));
          }}
          language={getLanguageFromDataSourceKind(kind)}
        />

        <Stack direction="row" style={{ alignItems: "center" }}>
          <Button onClick={handleSet}>set {label}</Button>
          <Button
            onClick={() => {
              editor.setValue(`select * from ${tableName}`);
            }}
            variant="outlined"
          >
            clear
          </Button>
          {isFilter && (
            <Tooltip title="save current value to filter bookmark">
              <IconButton
                onClick={async () => {
                  const value = editor.getValue();
                  if (!value) {
                    showNotification("warning", "this editor is empty");
                    return;
                  }
                  try {
                    const filter = await extractFn(value);
                    await TableFilterHandler.add(uuid, tableName, filter);
                    loadFilter();
                  } catch (e) {
                    showNotification("error", "save filter failed", e);
                  }
                }}
              >
                <SaveIcon />
              </IconButton>
            </Tooltip>
          )}
          {isFilter && (
            <Autocomplete
              value={selectFilter}
              style={{ flex: 1 }}
              options={filters.map(e => e.filter)}
              freeSolo={false}
              onChange={async (_, value) => {
                editor.setValue(
                  makeFormatFn(kind)(
                    `SELECT * FROM ${tableName} WHERE ${value}`,
                    formatarg,
                  ),
                );
                setSelectFilter(value);
              }}
              renderInput={params => <TextField {...params} label="filters" />}
            />
          )}
        </Stack>
      </Box>
    </Modal>
  );
}

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
    validJson(value).then(e => {
      setJsonErr(e);
    });
  }, [value, language]);

  return (
    <Stack flex={1}>
      <Stack direction="row">
        {editInModalLanguages.map(e => (
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
        handleValue={v => {
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
  constructor(public row: object, public field: string, public content: string) {}
}

function makeExtractTableSelectWhere(uuid: string) {
  return (sql: string) => {
    return invoke<string | null>("extract_table_select_selection", {
      name: uuid,
      sql,
    });
  };
}

function makeExtractTableSelectSort(uuid: string) {
  return (sql: string) => {
    return invoke<string | null>("extract_table_select_sort", {
      name: uuid,
      sql,
    });
  };
}
function SqlTableView(props: {
  notWarnPri?: boolean;
  kind: DataSourceKind;
  tableName: string;
  uuid;
  isTable: boolean;
}) {
  const { notWarnPri, kind, tableName, uuid, isTable } = props;
  const updateOP = makeTableUpdate(uuid, tableName);
  const deleteOP = makeTableDelete(uuid, tableName);
  const put_value_in_sql = makePutValueInSql(uuid);
  const addOP = makeTableInsertFn(uuid, tableName);

  const [loading, setLoading] = useState(false);
  const [where, setWhere] = useState("");
  const [showSetWhere, setShowSetWhere] = useState(false);
  const [sort, setSort] = useState("");
  const [showSetSort, setShowSetSort] = useState(false);
  const [page, setPage] = useState(0);
  const setting = useContext(SettingContext);

  const [pageSize, setPageSize] = useState(SavePageSize.instance.get());
  const [resp, setResp] = useState<MuiDataGridData>({
    sql: "",
    fields: [],
    data: [],
    total: 0,
    cache_time: null,
    has_more: false,
  });
  const { showNotification } = useContext(NotificationContext);

  const [allowFiltering, setAllowFiltering] = useState(false);
  const [editCellInModal, setEditCellInModal] = useState<EditInModalCellData>(null);
  const [editInModalRow, setEditInModalRow] = useState<object>(null);
  const [viewInModal, setViewInModal] = useState<object>(null);
  const [showExportMarkDown, setShowExportMarkDown] = useState(false);
  const handleResp = (resp: MuiDataGridData) => {
    resp.data.forEach((data, index) => {
      data[keyNotInTable] = index;
    });
    resp.fields.forEach(field => {
      const width = ColumnWidth.instance.get(field.field);
      if (width) {
        field["width"] = width;
      }
    });
    for (let i = 0; i < resp.fields.length; i++) {
      const field = resp.fields[i] as FieldAndGridField;
      field.renderHeader = e => {
        const def = e.colDef as unknown as FieldAndGridField;
        const makeTitle = (): string => {
          const f = def as unknown as FieldAndGridField;
          let ret = f.field + " ";
          ret += f.data_type;
          if (f.nullable) {
            ret += " null";
          } else {
            ret += " not null";
          }
          if (f.is_auto_increment) {
            ret += " auto increment";
          }
          if (f.default_value) {
            ret += " " + f.default_value;
          }
          return ret;
        };
        return (
          <Stack direction="row" style={{ alignItems: "center" }}>
            <Tooltip title={makeTitle()}>
              <Typography fontSize={dataGridheaderFontSize}>{def.field}</Typography>
            </Tooltip>
            {def.is_primary && <KeyIcon style={{ fontSize: "16px" }} />}
          </Stack>
        );
      };
      if (!field.is_auto_increment) {
        field.editable = true;
      }
      setResp(resp);
    }
  };

  const handleReload = useCallback(
    (showOK: boolean) => {
      switch (kind) {
        case DataSourceKind.Cassandra:
          break;
      }
      showTable({
        para: {
          where_str: where.trim(),
          order_by: sort.trim(),
          page: page + 1,
          page_size: pageSize,
          allow_filtering: allowFiltering,
          ...defaultShowTableParams(),
        },
        name: tableName,
        isView: !isTable,
        uuid,
        setLoading,
        okCallBack: showOK
          ? () => {
              if (showOK) {
                reloadOK(showNotification);
              }
            }
          : undefined,
        showNotification,
        handleResult: resp => {
          handleResp(resp);
        },
      });
    },
    [
      allowFiltering,
      isTable,
      kind,
      page,
      pageSize,
      showNotification,
      sort,
      tableName,
      uuid,
      where,
    ],
  );

  const handleProcessRowUpdate = useCallback(
    async (newRow: object, oldRow: object) => {
      if (!updateOP) {
        return newRow;
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
      old.delete(keyNotInTable);
      if (updateRow.size == 0) {
        return newRow;
      }
      if (!(await confirmAboutPrimary(notWarnPri, resp.fields))) {
        return oldRow;
      }
      try {
        const ret = await updateOP(old, updateRow);
        showNotification(
          "success",
          "Update OK",
          <Stack>
            <Typography> sql:{ret.sql} </Typography>
            <Typography> row affected :{ret.changes} </Typography>
          </Stack>,
        );
        handleReload(false);
      } catch (e) {
        showNotification("error", "update failed", e);
        throw e;
      }
      return newRow;
    },
    [handleReload, notWarnPri, resp.fields, showNotification, updateOP],
  );

  const handleDelete = useCallback(
    async id => {
      if (!(await confirmAboutPrimary(notWarnPri, resp.fields))) {
        return;
      }
      let old;
      for (const e of resp.data) {
        if (e[keyNotInTable] == id) {
          old = new Map(Object.entries(e));
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
      try {
        old.delete(keyNotInTable);
        const ret = await deleteOP(old);
        showNotification(
          "success",
          "delete OK",
          <Stack>
            <Typography> sql:{ret.sql} </Typography>
            <Typography> row affected :{ret.changes} </Typography>
          </Stack>,
        );
      } catch (e) {
        showNotification("error", "delete failed", e);
      }
      handleReload(false);
    },
    [deleteOP, handleReload, notWarnPri, resp.data, resp.fields, showNotification],
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
  const { setUpMenu } = useContext(ContextMenuContext);
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
    e => {
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
        onContextMenu: e => {
          const { id, field } = extractFieldAndIDFromMouseEvent(e);
          if (!field) {
            return;
          }
          const row = { ...resp.data.find(e => e[keyNotInTable] == id) };
          const content = row[field];
          delete row[keyNotInTable];
          const nullable = resp.fields.find(e => e.field == field).nullable;
          setUpMenu({
            content: (
              <Menu
                updateOP={updateOP}
                kind={kind}
                showExportMarkDown={() => {
                  setShowExportMarkDown(true);
                }}
                editCellInModal={() => {
                  setEditCellInModal(new EditInModalCellData(row, field, row[field]));
                }}
                editRowInModal={() => {
                  setEditInModalRow(row);
                }}
                nullable={nullable}
                fields={resp.fields}
                field={field}
                content={content}
                handleDelete={
                  deleteOP
                    ? () => {
                        handleDelete(id);
                      }
                    : undefined
                }
                row={row}
                addOP={
                  addOP
                    ? async row => {
                        await addOP(row);
                        handleReload(false);
                      }
                    : undefined
                }
                setSort={setSort}
                setWhere={setWhere}
                put_value_in_sql={value => {
                  return put_value_in_sql(
                    resp.fields.find(e => e.field == field).data_type,
                    value,
                  );
                }}
                setNull={
                  updateOP
                    ? async () => {
                        const row = resp.data.find(e => e[keyNotInTable] == id);
                        try {
                          const values = {};
                          values[field] = null;
                          delete row[keyNotInTable];
                          await updateOP(row, values);
                        } catch (e) {
                          showNotification("error", "update failed", e);
                          return;
                        }
                        showNotification("success", "set null OK");
                        handleReload(false);
                        return;
                      }
                    : undefined
                }
                resetColumnWidthCallBack={() => {
                  setResp(old => {
                    return resetMuiDataRespWidth(old);
                  });
                }}
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
    addOP,
    deleteOP,
    handleDelete,
    handleReload,
    kind,
    put_value_in_sql,
    resp.data,
    resp.fields,
    setUpMenu,
    setting.export.csvDelimiter,
    setting.export.includeHeader,
    showNotification,
    updateOP,
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
  const slots = useMemo(() => {
    return { columnMenu: AppColumnMenu } as Partial<GridSlotsComponent>;
  }, []);
  return (
    <Stack style={{ height: "100%" }}>
      <Stack direction="row" style={{ alignItems: "center" }}>
        <Button
          loading={loading}
          color="primary"
          onClick={() => {
            handleReload(true);
          }}
        >
          reload
        </Button>
        <Tooltip title="filter something like 'age=18'">
          <Chip
            label={where}
            onClick={() => {
              setShowSetWhere(true);
            }}
            style={shrinkAble}
            size="medium"
            color="primary"
            icon={<FilterAltIcon />}
          />
        </Tooltip>
        {/* <Typography color="info"> order by </Typography> */}
        <Tooltip title="sort something like 'age asc'">
          <Chip
            label={sort}
            onClick={() => {
              setShowSetSort(true);
            }}
            style={shrinkAble}
            size="medium"
            color="secondary"
            variant="outlined"
            icon={<FaArrowsAltV />}
          />
        </Tooltip>
        {kind == DataSourceKind.Cassandra && (
          <>
            <FormControlLabel
              control={
                <Checkbox
                  checked={allowFiltering}
                  onChange={e => setAllowFiltering(e.target.checked)}
                />
              }
              label="allow filtering"
            />
          </>
        )}
      </Stack>

      <Stack direction="row" sx={{ alignItems: "center" }}>
        <Typography
          sx={{
            cursor: "pointer",
            fontSize: "0.9rem",
            fontStyle: "italic",
            color: "success.main",
            ...textEllipsisStyle,
          }}
          variant="body1"
          onClick={() => {
            handleCopyToClipboard(resp.sql, showNotification);
          }}
        >
          {resp.sql}
        </Typography>

        <Stack
          direction="row"
          style={{
            justifyContent: "flex-end",
            alignItems: "center",
          }}
        >
          <ExecutingTime loading={loading} title="loading time" />
        </Stack>
      </Stack>

      <DataGrid
        showToolbar
        slots={slots}
        showCellVerticalBorder
        showColumnVerticalBorder
        slotProps={slotProps}
        style={dataGridStyle}
        rowHeight={setting.appearance.datagridRowHeight()}
        columnHeaderHeight={setting.appearance.datagridRowHeight()}
        getRowId={getRowId}
        paginationModel={paginationModel}
        pageSizeOptions={pageSizeOptions}
        paginationMode="server"
        onPaginationModelChange={onPaginationModelChange}
        checkboxSelection={false}
        rowCount={resp.total}
        sortingMode="server"
        onSortModelChange={onSortModelChange}
        columns={resp.fields}
        rows={resp.data}
        onColumnResize={onColumnResize}
        processRowUpdate={handleProcessRowUpdate}
        onProcessRowUpdateError={onProcessRowUpdateError}
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
              handleModify={async value => {
                const newRow = { ...editCellInModal.row };
                newRow[editCellInModal.field] = value;
                await handleProcessRowUpdate(newRow, editCellInModal.row);
                handleReload(false);
                setEditCellInModal(null);
              }}
            />
          </Box>
        </Modal>
      )}
      {editInModalRow && (
        <Modal
          open={!!editInModalRow}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setEditInModalRow(null);
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
                setEditInModalRow(null);
              }}
            />
            <SqlTableViewData
              initValues={editInModalRow}
              buttonName="modify"
              fields={resp.fields}
              isAdd={false}
              handleData={async args => {
                await handleProcessRowUpdate(args, editInModalRow);
                handleReload(false);
              }}
              handleClose={() => setEditInModalRow(null)}
            />
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
            <ExportMarkDownTable fields={resp.fields} data={resp.data} />
          </Box>
        </Modal>
      )}

      {showSetWhere && (
        <SetFilterSort
          tableName={tableName}
          open={showSetWhere}
          value={where}
          setValue={v => {
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
          setValue={v => {
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
  fields.forEach(e => {
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
  const { showNotification } = useContext(NotificationContext);
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
        console.info("insert done");
        showNotification("success", `${buttonName} OK`);
        if (handleClose) {
          handleClose();
        }
      })
      .catch(e => {
        console.error("insert failed");
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
          f => filterFn(f.field) || (args[f.field] ? filterFn(args[f.field]) : !filter),
        )
        .map(f => {
          return (
            <Stack
              key={f.field}
              direction="row"
              sx={{ alignContent: "center", justifyContent: "center" }}
            >
              <TextField
                inputRef={x => {
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
                onChange={e => {
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
          onChange={e => {
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
  handleDelete?;
  setSort;
  setWhere;
  put_value_in_sql: (value?: string) => Promise<string>;
  setNull?: () => void;
  addOP?: (x: object) => Promise<void>;
  updateOP?;
  row: object;
  fields: types.Field[];
  nullable: boolean;
  editCellInModal: () => void;
  editRowInModal: () => void;
  resetColumnWidthCallBack: () => void;
  kind: DataSourceKind;
  viewInModal: () => void;
  showExportMarkDown: () => void;
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
    updateOP,
    row,
    fields,
    nullable,
    editCellInModal,
    editRowInModal,
    resetColumnWidthCallBack,
    kind,
    viewInModal,
    showExportMarkDown,
  } = props;
  const { showNotification } = useContext(NotificationContext);
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
        {updateOP && (
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

        {updateOP && (
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

        {nullable && setNull && (
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
        <Divider />

        {addOP && (
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
        {handleDelete && (
          <MenuItem onClick={handleDelete}>
            <ListItemIcon>
              <DeleteForeverIcon />
            </ListItemIcon>
            <ListItemText> Delete Row</ListItemText>
          </MenuItem>
        )}
        <MenuResetColumnWidths reset={resetColumnWidthCallBack} />

        <MenuExportMarkDownTable showExportMarkDown={showExportMarkDown} />
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
              put_value_in_sql={value => {
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
  const { showNotification } = useContext(NotificationContext);
  const [formatValue, setFormatValue] = useState("");
  useEffect(() => {
    lastCompare = compare;
  }, [compare]);
  useEffect(() => {
    put_value_in_sql(content)
      .then(value => {
        setFormatValue(value);
      })
      .catch(e => {
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
        {compares.map(x => (
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
        {["null", "not null"].map(e => (
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
          {["starts", "ends", "contains"].map(e => (
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
      <Stack direction="row" style={{ alignItems: "center" }}>
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
      </Stack>
    </Stack>
  );
}

export { SqlTableView, SqlTableViewData };
