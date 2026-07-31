import {
  Dispatch,
  SetStateAction,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import ArrowOutwardIcon from "@mui/icons-material/ArrowOutward";
import { FaUpload } from "react-icons/fa";
import CancelIcon from "@mui/icons-material/Cancel";
import {
  Alert,
  Box,
  Button,
  Checkbox,
  Chip,
  Dialog,
  DialogTitle,
  Divider,
  FormControlLabel,
  IconButton,
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

import { MdOutlineReply } from "react-icons/md";
import VisibilityIcon from "@mui/icons-material/Visibility";
import EditIcon from "@mui/icons-material/Edit";
import ArrowCircleDownIcon from "@mui/icons-material/ArrowCircleDown";

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
  DataSourceKind,
  handleCopyToClipboard,
  isTypeLargeText,
  secondConfirm,
  textEllipsisStyle,
  quoteDatabaseIdentifier,
  extractFieldAndIDFromMouseEvent,
  dataGridStyle,
  pageSizeOptions,
  dataGridheaderFontSize,
  shrinkAble,
  TabContext,
  clickDataGridColumn,
  AppColumnMenuCtx,
  DatabaseOperation,
  DatabaseOpKind,
  gridColumnHeaderSelectClassName,
  allGridHeaderTitle,
  columnHeaderStyle,
  SqlOpContext,
  AppColumnMenuKind,
} from "./helper";
import FileCopyIcon from "@mui/icons-material/FileCopy";
import {
  AppColumnMenu,
  ModalHeader,
  ResetColumnWidth,
  ViewRowInModal,
} from "./CommonComp";

import { editor } from "monaco-editor";
import wcmatch from "wildcard-match";
import { MdEditNote } from "react-icons/md";
import {
  AppDataGridPagination,
  AppDataGridToolBar,
  ClearableTextField,
  CopyAsXXX,
  CopySql,
  ExportQueryResultToFile,
  FetchTimeDataGrid,
  ListenElementSize,
  QueryStatus,
  RefreshButton,
  SetFilterSort,
  SimpleFlexEditor,
  SliderSize,
  StackRowCenter,
} from "./CommonComp2";
import { showTable } from "./handlers";
import { AddTableModal, openNewTableStructure } from "./LeftTable";
import {
  useColumnWidthStore,
  useConfirmMsg,
  useContextMenu,
  useEditInModalLanguageStore,
  useNotification,
  useOneShotDialogUI,
  usePageSize,
  UseSizeConfig,
  useSizeStore,
  useSqlDataFilter,
  useSqlTableFilterOp,
  useTableColumnVisibleStore,
  useTabsManage,
} from "./store";
import { commands, Field, MuiDataGridData, TableImportPara } from "./bindings";
import { ExportTableOrView } from "./Export";
import UploadIcon from "@mui/icons-material/Upload";
import {
  DataGrid,
  GridCallbackDetails,
  GridPaginationModel,
  GridRenderCellParams,
  gridRowsLoadingSelector,
  GridSlotProps,
  GridSlotsComponent,
  GridSlotsComponentsProps,
  GridSortModel,
  useGridApiContext,
  useGridApiRef,
  useGridSelector,
} from "./datagridExport";
import { open } from "@tauri-apps/plugin-dialog";

const confirmAboutPrimary = async (fields: Field[]): Promise<boolean> => {
  const hasPri = !!fields.find((x) => x.is_primary);
  if (hasPri) {
    return true;
  }
  return secondConfirm({
    title: "No Primary key found.",
    content: "No Primary key found,maybe multi row affected.",
    kind: "warning",
  });
};

const getRowId = (e) => e[keyNotInTable];

function EditInModal(props: {
  content: string;
  handleModify: (_: string) => void;
  handleClose: () => void;
}) {
  const { content, handleModify, handleClose } = props;

  const langs = useMemo(() => {
    return ["text", "html", "json", "xml"];
  }, []);
  const { editingRowLanguage, setEditingRowLanguage } = useEditInModalLanguageStore();
  const [editor, setEditor] = useState<editor.IStandaloneCodeEditor>(null);
  const [editInModalBox, setEditInModalBox] = useState(null);

  useEffect(() => {
    if (!editingRowLanguage) {
      setEditingRowLanguage(langs[0]);
    }
  }, [editingRowLanguage, langs, setEditingRowLanguage]);

  return (
    <Modal
      open={true}
      onClose={modalHandleClose((e: EscDownEvent) => {
        e.preventDefault();
        handleClose();
      })}>
      <Box
        sx={{ resize: "both", ...modalStyle, width: "80%", height: "30vh" }}
        ref={setEditInModalBox}>
        <ModalHeader
          title="edit cell"
          handleClose={() => {
            handleClose();
          }}
        />
        <ListenElementSize dom={editInModalBox} set="both" name="table-edit-in-modal" />
        <Stack style={{ flex: 1 }}>
          <Stack direction="row">
            {langs.map((lang) => (
              <Chip
                key={lang}
                label={lang}
                color={lang == editingRowLanguage ? "success" : "default"}
                onClick={() => {
                  setEditingRowLanguage(lang);
                }}
              />
            ))}
          </Stack>
          <SimpleFlexEditor
            value={content}
            language={editingRowLanguage}
            handleEditor={(ed) => {
              setEditor(ed);
            }}
          />
          <Divider />
          <Stack direction="row">
            <Button
              onClick={() => {
                handleModify(editor.getValue());
              }}>
              modify
            </Button>
          </Stack>
        </Stack>
      </Box>
    </Modal>
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

function tryGetId(data: object, fields: Field[]) {
  let pf = fields.filter((e) => e.is_primary);
  if (pf.length == 0) {
    pf = fields;
  }
  return pf.map((x) => data[x.field]).join("-");
}

interface SqlTableCustomToolBarLeftProps {
  handleReload: (showOK: boolean) => void;
  where: string;
  setWhere: (val: string) => void;
  sort: string;
  setSort: (val: string) => void;
  kind: DataSourceKind; // 假设你已经定义了这个 Enum 或 Type
  allowFiltering: boolean;
  setAllowFiltering: Dispatch<SetStateAction<boolean>>;
  uuid: string;
  tableName: string;
}
// 建议放在文件顶部，或者单独的文件中
function SqlTableCustomToolBarLeft({
  handleReload,
  where,
  setWhere,
  sort,
  setSort,
  kind,
  allowFiltering,
  setAllowFiltering,
  uuid,
  tableName,
}: SqlTableCustomToolBarLeftProps) {
  // 获取 DataGrid 内部的 api 实例

  const { ops, setOps } = useContext(SqlOpContext);
  const apiRef = useGridApiContext();
  const [showSetWhere, setShowSetWhere] = useState(false);
  const [showSetSort, setShowSetSort] = useState(false);

  const { showNotification } = useNotification();

  // 使用选择器获取当前的 loading 状态
  const isLoading: boolean = useGridSelector(apiRef, gridRowsLoadingSelector);

  return (
    <Stack direction="row" style={{ alignItems: "center" }}>
      <RefreshButton
        loading={isLoading}
        color="primary"
        onClick={() => handleReload(true)}
      />

      <Tooltip title={where ? where : "filter something like 'age=18'"}>
        <Chip
          label={where ? where : "..."}
          onClick={() => setShowSetWhere(true)}
          style={{ ...shrinkAble, maxWidth: "14vw", textOverflow: "clip" }}
          size="medium"
          color="success"
          icon={<FilterAltIcon />}
          onDelete={where ? () => setWhere("") : undefined}
        />
      </Tooltip>

      <Tooltip title={sort ? sort : "sort something like 'age asc'"}>
        <Chip
          label={sort ? sort : "..."}
          onClick={() => setShowSetSort(true)}
          style={{ ...shrinkAble, maxWidth: "12vw", textOverflow: "clip" }}
          size="medium"
          color="secondary"
          variant="outlined"
          onDelete={sort ? () => setSort("") : undefined}
          icon={<FaArrowsAltV />}
        />
      </Tooltip>

      {kind === DataSourceKind.Cassandra && (
        <FormControlLabel
          control={
            <Checkbox
              checked={allowFiltering}
              onChange={(e) => setAllowFiltering(e.target.checked)}
            />
          }
          label="allow filtering"
        />
      )}

      {ops.length > 0 && (
        <>
          <Tooltip
            slotProps={{
              tooltip: {
                style: {
                  maxWidth: "33vw",
                },
              },
            }}
            title=<>
              {ops.map((x, index) => (
                <CopySql sql={x.sql} key={index} />
              ))}
              <Typography style={{ fontWeight: "bold" }}> apply all </Typography>
            </>>
            <IconButton
              onClick={async () => {
                for (const op of ops) {
                  try {
                    await commands.simpleExecuteSql(uuid, op.sql);
                    showNotification({
                      severity: "success",
                      message: <CopySql sql={op.sql} />,
                      title: "apply success",
                    });
                  } catch (e) {
                    showNotification({
                      severity: "error",
                      message: e,
                      title: "apply failed",
                    });
                  }
                }
                setOps([]);
                handleReload(false);
              }}>
              <FaUpload />
            </IconButton>
          </Tooltip>
          <IconButton
            color="warning"
            onClick={async () => {
              if (
                !(await secondConfirm({
                  title: `discard ${ops.length} changes?`,
                  confirmText: "discard",
                }))
              ) {
                return;
              }
              setOps([]);
            }}>
            <MdOutlineReply style={{ fontSize: "1.4rem" }} />
          </IconButton>
        </>
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

type CustomPaginationProps = {
  resp: MuiDataGridData;
  clientModel: boolean;
  uuid: string;
  kind: DataSourceKind;
  table: string;
  isTable: boolean;
  handleReload: () => void;
};
function ImportDataFromFile(props: {
  uuid: string;
  table: string;
  handleReload: () => void;
}) {
  const { uuid, table, handleReload } = props;
  const { showNotification } = useNotification();

  const [loading, setLoading] = useState(false);
  return (
    <Tooltip title="insert data from a file,current support csv,json,jsonline">
      <IconButton
        loading={loading}
        color="success"
        onClick={async () => {
          try {
            const path = await open({
              title: "import file",
              filters: [
                { extensions: ["csv"], name: "csv" },
                { extensions: ["json"], name: "json" },
                {
                  extensions: ["jsonl"],
                  name: "jsonl",
                },
              ],
            });
            if (!path) {
              return;
            }
            let para: TableImportPara = null;
            const pathLower = path.toLowerCase();
            if (pathLower.endsWith(".csv")) {
              para = { import_type: "csv", path };
            } else if (pathLower.endsWith(".json")) {
              para = { import_type: "json", path };
            } else if (pathLower.endsWith(".jsonl")) {
              para = { import_type: "jsonline", path };
            } else {
              showNotification({
                severity: "error",
                title: "unsupported file type",
                message: (
                  <Alert> file extension must end with csv,json or jsonl </Alert>
                ),
              });
              return;
            }
            setLoading(true);
            await commands.tableImportFromFile(uuid, table, para);
            showNotification({ severity: "success", title: "import data ok" });
            handleReload();
          } catch (e) {
            showNotification({
              severity: "error",
              message: e,
              title: "import data from file failed",
            });
          } finally {
            setLoading(false);
          }
        }}>
        <UploadIcon />
      </IconButton>
    </Tooltip>
  );
}

export function SqlTablePagination({
  dbpal: { resp, clientModel, uuid, kind, table, isTable, handleReload },
}: {
  dbpal: CustomPaginationProps;
}) {
  const { showNotification } = useNotification();
  const { appendTab } = useTabsManage();
  return (
    <Stack
      direction="row"
      sx={{
        alignItems: "center",
        justifyContent: "space-between",
        flex: 1,
        overflow: "auto",
      }}>
      <Stack direction="row" sx={{ alignItems: "center", flex: 1 }}>
        {isTable && (
          <Button
            onClick={() => {
              openNewTableStructure(appendTab, uuid, kind, table);
            }}
            variant="outlined">
            structure
            <ArrowOutwardIcon />
          </Button>
        )}
        <FetchTimeDataGrid />
        <ResetColumnWidth />
        {clientModel && <SliderSize config={UseSizeConfig.queryLimit} />}
        {isTable && (
          <ImportDataFromFile uuid={uuid} table={table} handleReload={handleReload} />
        )}

        <ExportQueryResultToFile />

        {isTable && <AddTableModal kind={kind} tableName={table} uuid={uuid} />}
        {resp.sql && (
          <Tooltip title={resp.sql}>
            <Typography
              variant="body2"
              sx={{ cursor: "pointer", color: "info.main" }}
              onClick={() => handleCopyToClipboard(resp.sql!, showNotification)}>
              SQL
            </Typography>
          </Tooltip>
        )}
      </Stack>

      <Stack direction="row" sx={{ alignItems: "center" }}>
        {!clientModel && (
          <Typography variant="body2" color="info.main" sx={textEllipsisStyle}>
            result {resp.data.length} of {resp.total}
          </Typography>
        )}

        <AppDataGridPagination showPageSelect />
      </Stack>
    </Stack>
  );
}

function EditRowInModal(props: {
  handleClose: () => void;
  fields: Field[];
  handleProcessRowUpdate: (value: object, newValue: object) => void;
  value: object;
}) {
  const { handleClose, value, fields, handleProcessRowUpdate } = props;
  const [editRowInModalBox, setEditRowInModalBox] = useState(null);
  return (
    <Modal
      open={true}
      onClose={modalHandleClose((e: EscDownEvent) => {
        e.preventDefault();
        handleClose();
      })}>
      <Box
        sx={{ resize: "horizontal", ...modalStyle, width: "80%" }}
        ref={setEditRowInModalBox}>
        <ModalHeader
          title="edit row"
          handleClose={() => {
            handleClose();
          }}
        />
        <ListenElementSize
          dom={editRowInModalBox}
          set="both"
          name="table-edit-row-in-modal"
        />
        <SqlTableViewData
          initValues={value}
          buttonName="modify"
          fields={fields}
          isAdd={false}
          handleData={async (args) => {
            handleProcessRowUpdate(args, value);
          }}
          handleClose={() => handleClose()}
        />
      </Box>
    </Modal>
  );
}

function CellButtons(props: {
  op: DatabaseOperation;
  setOps: Dispatch<SetStateAction<DatabaseOperation[]>>;
  uuid: string;
  firstField: boolean;
  handleReload: () => void;
}) {
  const { op, setOps, uuid, firstField, handleReload } = props;
  const { showNotification } = useNotification();
  return (
    firstField && (
      <>
        <Tooltip
          title=<>
            <CopySql sql={op.sql} />
          </>>
          <IconButton
            onClick={async () => {
              try {
                await commands.simpleExecuteSql(uuid, op.sql);
                setOps((ops) => ops.filter((x) => x != op));
                showNotification({
                  severity: "success",
                  title: "execute sql success",
                  message: <CopySql sql={op.sql} />,
                });
                handleReload();
              } catch (e) {
                showNotification({
                  severity: "error",
                  title: "try execute sql failed",
                  message: e,
                });
              }
            }}>
            <FaUpload />
          </IconButton>
        </Tooltip>
        <IconButton
          onClick={() => {
            setOps((ops) => ops.filter((x) => x != op));
          }}>
          <MdOutlineReply style={{ fontSize: "1.2rem" }} />
        </IconButton>
      </>
    )
  );
}

function CustomCell(props: GridRenderCellParams) {
  const { row, field } = props;
  const { ops, setOps, fields, uuid, handleReload } = useContext(SqlOpContext);
  let dbId = tryGetId(row, fields);
  const modified = ops.find((x) => x.tableId == dbId && x.op == DatabaseOpKind.Update);
  const deleted = ops.find((x) => x.tableId == dbId && x.op == DatabaseOpKind.Delete);
  const firstField = fields[0].field == field;
  let dataField = { "data-field": field };
  if (deleted) {
    return (
      <Stack
        direction="row"
        spacing={0}
        {...dataField}
        style={{ alignItems: "center" }}>
        <CellButtons
          uuid={uuid}
          setOps={setOps}
          op={deleted}
          handleReload={handleReload}
          firstField={firstField}
        />
        <Typography style={{ textDecoration: "line-through", color: "red" }}>
          {row[field]}
        </Typography>
      </Stack>
    );
  }

  if (modified) {
    return (
      <Stack
        spacing={0}
        direction="row"
        style={{ alignItems: "center" }}
        {...dataField}>
        <CellButtons
          uuid={uuid}
          setOps={setOps}
          op={modified}
          handleReload={handleReload}
          firstField={firstField}
        />
        {modified.newValue[field] != row[field] ? (
          <Tooltip
            title=<Stack>
              <CopySql sql={modified.sql} />
              <Typography> old value:{row[field]} </Typography>
            </Stack>>
            <Typography
              sx={{
                color: "red",
                fontWeight: "bolder",
              }}>
              {modified.newValue[field]}
            </Typography>
          </Tooltip>
        ) : (
          <Typography>{row[field]}</Typography>
        )}
      </Stack>
    );
  }
  return <Typography {...dataField}>{row[field]}</Typography>;
}
function SqlTableView(props: {
  kind: DataSourceKind;
  tableName: string;
  uuid: string;
  isTable: boolean;
}) {
  const { kind, tableName, uuid, isTable } = props;
  const getUpdateSql = useCallback(
    (x, y) => {
      return commands.tableUpdateSql(uuid, tableName, x, y);
    },
    [tableName, uuid],
  );
  const getDeleteSql = useCallback(
    (x) => {
      return commands.tableDeleteSql(uuid, tableName, x);
    },
    [tableName, uuid],
  );
  const putValueInSql = useCallback(
    (typ, x) => {
      return commands.putValueInSql(uuid, typ, x);
    },
    [uuid],
  );
  const addOP = useCallback(
    async (x) => {
      await commands.tableInsert(uuid, tableName, x);
    },
    [uuid, tableName],
  );

  const [loading, setLoading] = useState(false);
  const [where, setWhere] = useState("");

  const [sort, setSort] = useState("");

  const [page, setPage] = useState(0);
  const { sql: pageSize, setPageSize } = usePageSize();
  const { setUi } = useOneShotDialogUI();
  const [resp, setResp] = useState<MuiDataGridData>({
    sql: "",
    fields: [],
    data: [],
    total: 0,
  });
  const apiRef = useGridApiRef();
  const [ops, setOps] = useState<DatabaseOperation[]>([]);

  const tabCtx = useContext(TabContext);
  const { removeConfirmMsg, addConfirmMsg } = useConfirmMsg();

  useEffect(() => {
    if (ops.length == 0) {
      removeConfirmMsg(tabCtx.id);
    } else {
      addConfirmMsg(tabCtx.id, `sure to discard ${ops.length} changes?`);
    }
    return () => {
      removeConfirmMsg(tabCtx.id);
    };
  }, [ops, tabCtx.id, removeConfirmMsg, addConfirmMsg]);

  const { showNotification } = useNotification();
  const [allowFiltering, setAllowFiltering] = useState(false);
  const [editCellInModal, setEditCellInModal] = useState<EditInModalCellData>(null);
  const [editRowInModal, setRowEditInModal] = useState<object>(null);
  const clientModel = [DataSourceKind.Cassandra].includes(kind);
  const [viewInModal, setViewInModal] = useState<object>(null);
  const [queryStatus, setQueryStatus] = useState("");
  const { getColumnWidth, setColumnWidth } = useColumnWidthStore();
  const [showClone, setShowClone] = useState<object>(null);
  const getSize = useSizeStore((s) => s.getSize);
  const [selectedColumns, setSelectColumns] = useState<Set<string>>(new Set());

  const [showFilter, setShowFilter] = useState<{
    field: string;
    content: string;
    typ: string;
  }>(null);
  const handleResp = useCallback(
    (resp: MuiDataGridData) => {
      resp.data.forEach((data, index) => {
        data[keyNotInTable] = index + "";
      });
      resp.fields.forEach((field) => {
        const width = getColumnWidth(field.field);
        if (width) {
          field["width"] = width;
        }
      });
      for (let i = 0; i < resp.fields.length; i++) {
        const field = resp.fields[i] as FieldAndGridField;
        field.renderCell = (e) => <CustomCell {...e} />;
        field.renderHeader = (e) => {
          const field = e.colDef as unknown as FieldAndGridField;
          const makeTitle = (): string => {
            let ret = field.field + " ";
            ret += field.data_type;
            if (!isTable) {
              return ret;
            }
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
              style={{
                alignItems: "center",
                ...columnHeaderStyle,
              }}
              direction="row"
              onClick={(event) => {
                event.stopPropagation();
                clickDataGridColumn(apiRef, event, field.field, setSelectColumns);
              }}>
              {field.is_primary && <KeyIcon style={{ fontSize: "16px" }} />}
              <Tooltip title={makeTitle()}>
                <Typography style={{ fontSize: dataGridheaderFontSize }}>
                  {field.field}
                </Typography>
              </Tooltip>
            </Stack>
          );
        };
        field.editable = !field.is_auto_increment;
      }
      setResp(resp);
    },
    [apiRef, getColumnWidth, isTable],
  );

  const makePara = useCallback(() => {
    const where_str = where.trim();
    let order_by = sort.trim();
    return {
      where_str: where_str ? where_str : null,
      order_by: order_by ? order_by : null,
      page: page + 1,
      page_size: pageSize,
      allow_filtering: allowFiltering,
      limit: getSize(UseSizeConfig.queryLimit),
    };
  }, [allowFiltering, getSize, page, pageSize, sort, where]);
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
    [handleResp, isTable, kind, makePara, showNotification, tableName, uuid],
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
        showNotification({ severity: "warning", title: "row is modifying" });
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
      if (!(await confirmAboutPrimary(resp.fields))) {
        return oldRow;
      }
      const data = await getUpdateSql(updateRow, old);
      setOps((e) => [
        ...e,
        new DatabaseOperation(
          DatabaseOpKind.Update,
          data,
          tryGetId(oldRow, resp.fields),
          newRow,
        ),
      ]);
      return oldRow;
    },
    [rowHasOP, resp, showNotification, getUpdateSql],
  );

  const handleDelete = useCallback(
    async (id) => {
      if (!(await confirmAboutPrimary(resp.fields))) {
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
        showNotification({
          severity: "error",
          title: "not found the record to delete this is a internal bug",
        });
        return;
      }
      if (rowHasOP(old)) {
        showNotification({ severity: "warning", title: "row is modifying" });
        return;
      }
      const data = await getDeleteSql(old);
      setOps((e) => [
        ...e,
        new DatabaseOperation(
          DatabaseOpKind.Delete,
          data,
          tryGetId(old, resp.fields),
          null,
        ),
      ]);
    },
    [getDeleteSql, resp.data, resp.fields, rowHasOP, showNotification],
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
      if (e.pageSize > 100) {
        console.error("should not have page size greater than 100");
      }
      if (reason.reason == "setPaginationModel" && e.pageSize <= 100) {
        setPageSize("sql", e.pageSize);
      }
    },
    [setPageSize],
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
        `${quoteDatabaseIdentifier(sort.field, kind)} ${sort.sort.toUpperCase()}`,
      );
    },
    [kind],
  );

  const onProcessRowUpdateError = useCallback(
    (e) => {
      showNotification({ severity: "error", title: "update failed", message: e });
    },
    [showNotification],
  );

  const onColumnResize = useCallback(
    ({ colDef, width }) => {
      setColumnWidth(colDef.field, width);
    },
    [setColumnWidth],
  );

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
          const nullable =
            resp.fields.find((e) => e.field == field).nullable && !!content;
          const fieldDef = resp.fields.find((x) => x.field == field);

          setUpMenu({
            content: (
              <CellMenu
                getRow={() => {
                  const ret = {};
                  for (const f of apiRef.current.getAllColumns()) {
                    ret[f.field] = apiRef.current.getCellValue(id, f.field);
                  }
                  return ret;
                }}
                kind={kind}
                editCellInModal={() => {
                  setEditCellInModal(
                    new EditInModalCellData(row, field, row[field], parseInt(id)),
                  );
                }}
                uuid={uuid}
                fieldType={fieldDef.data_type}
                editRowInModal={() => {
                  setRowEditInModal(row);
                }}
                isTable={isTable}
                nullable={nullable}
                field={field}
                content={content}
                handleDelete={
                  isTable
                    ? () => {
                        handleDelete(id);
                      }
                    : undefined
                }
                setShowClone={() => {
                  setShowClone(row);
                }}
                setSort={setSort}
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
                setShowFilter={() => {
                  setShowFilter({
                    field,
                    content,
                    typ: resp.fields.find((e) => e.field == field).data_type,
                  });
                }}
              />
            ),
            event: e,
          });
        },
        style: { cursor: "context-menu" },
      },
      pagination: {
        dbpal: (function (): CustomPaginationProps {
          return {
            resp,
            clientModel,
            uuid,
            kind,
            table: tableName,
            isTable,
            handleReload: () => {
              handleReload(false);
            },
          };
        })(),
      },
      toolbar: {
        dbpal: (function (): SqlTableCustomToolBarLeftProps {
          return {
            handleReload,
            where,
            setWhere,
            sort,
            setSort,
            kind,
            allowFiltering,
            setAllowFiltering,

            tableName,
            uuid,
          };
        })(),
      },
    } as GridSlotsComponentsProps;
  }, [
    resp,
    setUpMenu,
    kind,
    uuid,
    isTable,
    apiRef,
    handleDelete,
    handleProcessRowUpdate,
    clientModel,
    tableName,
    handleReload,
    where,
    sort,
    allowFiltering,
  ]);

  const slots = useMemo((): Partial<GridSlotsComponent> => {
    return {
      columnMenu: AppColumnMenu,
      toolbar: ({
        dbpal,
      }: GridSlotProps["toolbar"] & { dbpal: SqlTableCustomToolBarLeftProps }) => {
        return (
          <AppDataGridToolBar
            titleName={tableName}
            right=<>
              <IconButton
                onClick={() => {
                  setUi(
                    <ExportTableOrView
                      uuid={uuid}
                      name={tableName}
                      kind={kind}
                      isTable={isTable}
                      para={makePara()}
                    />,
                  );
                }}>
                <ArrowCircleDownIcon />
              </IconButton>
            </>
            left=<SqlTableCustomToolBarLeft {...dbpal} />
          />
        );
      },
      pagination: SqlTablePagination,
    };
  }, [isTable, kind, makePara, setUi, tableName, uuid]);
  const sortModel = clientModel ? "client" : "server";
  const { getColumnVisibility, setColumnVisibility } = useTableColumnVisibleStore(
    (s) => s,
  );
  return (
    <Stack style={{ height: "100%" }}>
      <QueryStatus status={queryStatus} />

      <SqlOpContext
        value={{
          ops,
          setOps,
          uuid,
          handleReload: () => {
            handleReload(false);
          },
          fields: resp.fields,
        }}>
        <AppColumnMenuCtx
          value={{
            uuid,
            selectColumns: (function () {
              return resp.fields
                .filter((f) => selectedColumns.has(f.field))
                .map((f) => f.field);
            })(),
            kind: AppColumnMenuKind.SqlTable,
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
            slots={slots}
            apiRef={apiRef}
            columnVisibilityModel={getColumnVisibility(uuid, tableName)}
            onColumnVisibilityModelChange={(e) => {
              setColumnVisibility(uuid, tableName, e);
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
        </AppColumnMenuCtx>
      </SqlOpContext>

      <ViewRowInModal
        data={viewInModal}
        handleClose={() => {
          setViewInModal(null);
        }}
        isTable={isTable}
        fields={resp.fields}
      />

      {editCellInModal && (
        <EditInModal
          content={editCellInModal.content}
          handleModify={async (value) => {
            const newRow = { ...editCellInModal.row };
            newRow[editCellInModal.field] = value;
            await handleProcessRowUpdate(newRow, editCellInModal.row);
            setEditCellInModal(null);
          }}
          handleClose={() => {
            setEditCellInModal(null);
          }}
        />
      )}
      {showFilter && (
        <Dialog
          open={!!showFilter}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setShowFilter(null);
          })}
          fullWidth
          maxWidth="sm">
          <DialogTitle> Quick filter </DialogTitle>
          <FilterOnValue
            content={showFilter.content}
            setWhere={setWhere}
            field={showFilter.field}
            kind={kind}
            handleClose={() => {
              setShowFilter(null);
            }}
            putValueInSql={(value) => {
              return putValueInSql(showFilter.typ, value);
            }}
          />
        </Dialog>
      )}

      {editRowInModal && (
        <EditRowInModal
          handleProcessRowUpdate={handleProcessRowUpdate}
          value={editRowInModal}
          handleClose={() => {
            setRowEditInModal(null);
          }}
          fields={resp.fields}
        />
      )}

      {showClone && (
        <Modal
          open={!!showClone}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setShowClone(null);
          })}>
          <Box sx={{ resize: "horizontal", ...modalStyle, width: "50%" }}>
            <ModalHeader
              title="Clone Data"
              handleClose={() => {
                setShowClone(null);
              }}
            />
            <SqlTableViewData
              buttonName="add"
              handleData={addOP}
              fields={resp.fields}
              handleClose={() => setShowClone(null)}
              initValues={showClone}
              isAdd
            />
          </Box>
        </Modal>
      )}
    </Stack>
  );
}

function SqlTableViewData(props: {
  handleData: (args: any) => Promise<void>;
  fields: Field[];
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
  const { filter, setFilter } = useSqlDataFilter();

  const filterFn = useMemo(() => {
    return filter
      ? wcmatch("*" + filter + "*", { flags: "i" })
      : () => {
          return true;
        };
  }, [filter]);

  const [loading, setLoading] = useState(false);
  const [args, setArgs] = useState(initValues);
  const { showNotification } = useNotification();
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
        showNotification({
          severity: "error",
          title: `filed '${f.field}' need be filled`,
        });
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
        showNotification({ severity: "success", title: `${buttonName} OK` });
        if (handleClose) {
          handleClose();
        }
      })
      .catch((e) => {
        console.error("handle value failure", e);
        showNotification({
          severity: "error",
          title: `${buttonName} failure`,
          message: e,
        });
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
              sx={{ alignContent: "center", justifyContent: "center" }}>
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
        <ClearableTextField
          value={filter}
          onChange={(e) => {
            setFilter(e.target.value);
          }}
          label="filter"
          fullWidth
          helperText="can match field name and content"
          onClear={() => {
            setFilter("");
          }}
        />
      </Stack>
    </Stack>
  );
}

function CellMenu(props: {
  field: string;
  fieldType: string;
  content: string;
  handleDelete?: () => void;
  setSort: Dispatch<SetStateAction<string>>;
  setNull?: () => void;
  nullable: boolean;
  editCellInModal: () => void;
  editRowInModal: () => void;
  kind: DataSourceKind;
  viewInModal: () => void;
  isTable: boolean;
  setShowFilter: () => void;
  setShowClone: () => void;
  uuid: string;
  getRow: () => object;
}) {
  const {
    field,
    content,
    handleDelete,
    setSort,
    setNull,
    nullable,
    editCellInModal,
    editRowInModal,
    kind,
    viewInModal,
    isTable,
    setShowFilter,
    setShowClone,
    uuid,
    fieldType,
    getRow,
  } = props;

  const quoteField = quoteDatabaseIdentifier(field, kind);

  return (
    <MenuList>
      {isTable && (
        <MenuItem
          onClick={() => {
            editCellInModal();
          }}>
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
          }}>
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
          }}>
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
      <CopyAsXXX
        uuid={uuid}
        field={field}
        fieldType={fieldType}
        getRow={getRow}
        content={content}
      />
      <Divider />
      <MenuItem
        onClick={() => {
          setSort(`${quoteField} ASC`);
        }}>
        <ListItemIcon>
          <ArrowUpwardIcon />
        </ListItemIcon>
        <ListItemText> {field} ASC </ListItemText>
      </MenuItem>
      <MenuItem
        onClick={() => {
          setSort(`${quoteField} DESC`);
        }}>
        <ListItemIcon>
          <ArrowDownwardIcon />
        </ListItemIcon>
        <ListItemText> {field} DESC </ListItemText>
      </MenuItem>

      {content && (
        <MenuItem
          onClick={() => {
            setShowFilter();
          }}>
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
            setShowClone();
          }}>
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
  );
}

function FilterOnValue(props: {
  content: string;
  setWhere;
  field: string;
  handleClose: () => void;
  kind: DataSourceKind;
  putValueInSql: (x: string) => Promise<string>;
}) {
  const { content, setWhere, field, handleClose, putValueInSql, kind } = props;
  const compares = ["=", "<>", ">", ">=", "<", "<="];
  const { showNotification } = useNotification();
  const [formatValue, setFormatValue] = useState("");

  const { filter, setFilter } = useSqlTableFilterOp();
  useEffect(() => {
    putValueInSql(content)
      .then((value) => {
        setFormatValue(value);
      })
      .catch((e) => {
        showNotification({
          severity: "error",
          title: "put value in sql failed",
          message: e,
        });
      });
  }, [content, putValueInSql, showNotification]);

  const where = () => {
    const quoteField = quoteDatabaseIdentifier(field, kind);
    if (filter == "null") {
      return `${quoteField} IS NULL`;
    }
    if (filter == "not null") {
      return `${quoteField} IS NOT NULL`;
    }
    if (filter == "starts") {
      return `${quoteField} LIKE '${content}%'`;
    }
    if (filter == "ends") {
      return `${quoteField} LIKE '%${content}'`;
    }
    if (filter == "contains") {
      return `${quoteField} LIKE '%${content}%'`;
    }
    return `${quoteField} ${filter} ${formatValue}`;
  };
  return (
    <Stack spacing={1}>
      <Stack direction="row">
        {compares.map((x) => (
          <Chip
            key={x}
            onClick={() => {
              setFilter(x);
            }}
            label={x}
            color={filter == x ? "success" : "default"}
            style={{ minWidth: "3rem" }}
          />
        ))}
      </Stack>

      {/* cassandra not support this syntax  */}
      {kind != DataSourceKind.Cassandra && (
        <Stack direction="row">
          {["null", "not null"].map((e) => (
            <Chip
              label={e}
              color={filter == e ? "success" : "default"}
              onClick={() => {
                setFilter(e);
              }}
              key={e}
            />
          ))}
        </Stack>
      )}

      {formatValue.endsWith("'") && (
        <Stack direction="row">
          {["starts", "ends", "contains"].map((e) => (
            <Chip
              label={e}
              color={filter == e ? "success" : "default"}
              onClick={() => {
                setFilter(e);
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
          }}>
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
