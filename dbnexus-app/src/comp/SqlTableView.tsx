import {
  createContext,
  Dispatch,
  SetStateAction,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";

import { FaUpload } from "react-icons/fa";
import CancelIcon from "@mui/icons-material/Cancel";
import {
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
  validJson,
  textEllipsisStyle,
  quoteDatabaseIdentifier,
  extractFieldAndIDFromMouseEvent,
  dataGridStyle,
  pageSizeOptions,
  dataGridheaderFontSize,
  shrinkAble,
  TabContext,
} from "./helper";
import { FaCopy } from "react-icons/fa6";
import FileCopyIcon from "@mui/icons-material/FileCopy";
import {
  AppColumnMenu,
  ModalHeader,
  ResetColumnWidth,
  ViewRowInModal,
} from "./CommonComp";

import { editor, Position } from "monaco-editor";
import wcmatch from "wildcard-match";
import { MdEditNote } from "react-icons/md";
import {
  AppDataGridPagination,
  AppDataGridToolBar,
  ClearableTextField,
  CopySql,
  FetchTimeDataGrid,
  ListenElementSize,
  QueryStatus,
  SetFilterSort,
  SimpleFlexEditor,
  SliderSize,
  StackRowCenter,
} from "./CommonComp2";
import { showTable } from "./handlers";
import { AddTableModal } from "./LeftTable";
import {
  useColumnWidthStore,
  useConfirmMsg,
  useContextMenu,
  useNotification,
  useOneShotDialogUI,
  usePageSize,
  UseSizeConfig,
  useSizeStore,
  useTableColumnVisibleStore,
} from "./store";
import {
  commands,
  ErrAndLoc,
  Field,
  MuiDataGridData,
  ShowTableParameter,
} from "./bindings";
import { ExportTableOrView } from "./Export";
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

const confirmAboutPrimary = async (
  notWarnPri: boolean,
  fields: Field[],
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
    kind: "warning",
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

function EditInModal(props: {
  content: string;
  handleModify;
  handleClose: () => void;
}) {
  const { content, handleModify, handleClose } = props;
  const [jsonErr, setJsonErr] = useState<ErrAndLoc>(null);
  const [jsonErrPos, setJsonErrPos] = useState<Position>(null);
  const [language, setLanguage] = useState(EditInModalLanguage);
  const [editor, setEditor] = useState<editor.IStandaloneCodeEditor>(null);
  const [editInModalBox, setEditInModalBox] = useState(null);
  useEffect(() => {
    if (!editor) {
      return;
    }
    let d = editor.getModel().onDidChangeContent(() => {
      if (language != "json") {
        setJsonErr(null);
        return;
      }
      validJson(editor.getValue()).then((e) => {
        setJsonErr(e);
      });
    });
    return () => {
      d.dispose();
    };
  }, [editor, language]);

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
            handleEditor={(ed) => {
              setEditor(ed);
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
                  }}>
                  {jsonErr.msg}
                </Typography>
              ) : undefined
            }
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

enum DatabaseOpKind {
  Update,
  Delete,
}

class DatabaseOperation {
  public constructor(
    public op: DatabaseOpKind,
    public sql: string,
    public tableId: string | null,
    public newValue: object | null,
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
  setShowSetWhere: (show: boolean) => void;
  sort: string;
  setSort: (val: string) => void;
  setShowSetSort: (show: boolean) => void;
  kind: DataSourceKind; // 假设你已经定义了这个 Enum 或 Type
  allowFiltering: boolean;
  setAllowFiltering: (val: boolean) => void;
  clientModel: any; // 根据你实际的 model 类型修改
  isTable: boolean;
  uuid: string;
  tableName: string;
  ops: DatabaseOperation[];
  setOps: Dispatch<SetStateAction<DatabaseOperation[]>>;
}
// 建议放在文件顶部，或者单独的文件中
function SqlTableCustomToolBarLeft({
  handleReload,
  where,
  setWhere,
  setShowSetWhere,
  sort,
  setSort,
  setShowSetSort,
  kind,
  allowFiltering,
  setAllowFiltering,
  clientModel,
  isTable,
  uuid,
  tableName,
  ops,
  setOps,
}: SqlTableCustomToolBarLeftProps) {
  // 获取 DataGrid 内部的 api 实例
  const apiRef = useGridApiContext();

  const { showNotification } = useNotification();

  // 使用选择器获取当前的 loading 状态
  const isLoading: boolean = useGridSelector(apiRef, gridRowsLoadingSelector);

  return (
    <StackRowCenter>
      <Button loading={isLoading} color="primary" onClick={() => handleReload(true)}>
        reload
      </Button>

      <Tooltip title={where ? where : "filter something like 'age=18'"}>
        <Chip
          label={where}
          onClick={() => setShowSetWhere(true)}
          style={{ ...shrinkAble, maxWidth: "14vw", textOverflow: "clip" }}
          size="medium"
          color="primary"
          icon={<FilterAltIcon />}
          onDelete={where ? () => setWhere("") : undefined}
        />
      </Tooltip>

      <Tooltip title={sort ? sort : "sort something like 'age asc'"}>
        <Chip
          label={sort}
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
      {clientModel && <SliderSize config={UseSizeConfig.queryLimit} />}
      {isTable && <AddTableModal kind={kind} tableName={tableName} uuid={uuid} />}
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
              if (!(await secondConfirm({ title: `discard ${ops.length} changes?` }))) {
                return;
              }
              setOps([]);
            }}>
            <MdOutlineReply style={{ fontSize: "1.4rem" }} />
          </IconButton>
        </>
      )}
    </StackRowCenter>
  );
}

type CustomPaginationProps = { resp: MuiDataGridData; clientModel: boolean };
export function CustomPagination({
  dbpal: { resp, clientModel },
}: {
  dbpal: CustomPaginationProps;
}) {
  const { showNotification } = useNotification();
  return (
    <Stack
      direction="row"
      sx={{
        alignItems: "center",
        justifyContent: "space-between",
        flex: 1,
        overflow: "auto",
        px: 1, // 建议加一点间距
      }}>
      <Stack direction="row" spacing={2} sx={{ alignItems: "center", flex: 1 }}>
        <FetchTimeDataGrid />
        <ResetColumnWidth />
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

      <Stack direction="row" spacing={2} sx={{ alignItems: "center" }}>
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
            await handleProcessRowUpdate(args, value);
          }}
          handleClose={() => handleClose()}
        />
      </Box>
    </Modal>
  );
}

const OpsContext = createContext<{
  ops: DatabaseOperation[];
  setOps: Dispatch<SetStateAction<DatabaseOperation[]>>;
  fields: Field[];
  uuid: string;
  handleReload: () => void;
  clickedField: string;
}>(null);

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
  const { ops, setOps, fields, uuid, handleReload, clickedField } =
    useContext(OpsContext);
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
  if (field == clickedField) {
    return (
      <Typography {...dataField} style={{ background: " #c8e6c9", color: "black" }}>
        {row[field]}
      </Typography>
    );
  }
  return <Typography {...dataField}>{row[field]}</Typography>;
}
function SqlTableView(props: {
  notWarnPri?: boolean;
  kind: DataSourceKind;
  tableName: string;
  uuid: string;
  isTable: boolean;
}) {
  const { notWarnPri, kind, tableName, uuid, isTable } = props;
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
  const [showSetWhere, setShowSetWhere] = useState(false);
  const [sort, setSort] = useState("");
  const [showSetSort, setShowSetSort] = useState(false);
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
  }, [ops, tabCtx.id, removeConfirmMsg, addConfirmMsg]);
  useEffect(() => {
    return () => {
      removeConfirmMsg(tabCtx.id);
    };
  }, [removeConfirmMsg, tabCtx.id]);
  const { showNotification } = useNotification();
  const [allowFiltering, setAllowFiltering] = useState(false);
  const [editCellInModal, setEditCellInModal] = useState<EditInModalCellData>(null);
  const [editRowInModal, setRowEditInModal] = useState<object>(null);
  const clientModel = [DataSourceKind.Cassandra].includes(kind);
  const [viewInModal, setViewInModal] = useState<object>(null);
  const [queryStatus, setQueryStatus] = useState("");
  const [clickedField, setClickedField] = useState("");
  const { getColumnWidth, setColumnWidth } = useColumnWidthStore();
  const [showClone, setShowClone] = useState<object>(null);
  const getSize = useSizeStore((s) => s.getSize);
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
              style={{ alignItems: "center" }}
              direction="row">
              <Tooltip title={makeTitle()}>
                <Typography style={{ fontSize: dataGridheaderFontSize }}>
                  {field.field}
                </Typography>
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
    },
    [getColumnWidth],
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
    } as ShowTableParameter;
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
      if (!(await confirmAboutPrimary(notWarnPri, resp.fields))) {
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
      if (reason.reason == "setPaginationModel") {
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
          console.log("xxxxx", field);
          if (!field) {
            return;
          }
          const row = { ...resp.data.find((e) => e[keyNotInTable] == id) };
          const content = row[field];
          const nullable =
            resp.fields.find((e) => e.field == field).nullable && !!content;
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
          return { resp, clientModel };
        })(),
      },
      toolbar: {
        dbpal: (function (): SqlTableCustomToolBarLeftProps {
          return {
            handleReload,
            where,
            setWhere,
            setShowSetWhere,
            sort,
            setSort,
            setShowSetSort,
            kind,
            allowFiltering,
            setAllowFiltering,
            clientModel,
            isTable,
            tableName,
            uuid,
            ops,
            setOps,
          };
        })(),
      },
    } as GridSlotsComponentsProps;
  }, [
    resp,
    setUpMenu,
    kind,
    isTable,
    uuid,
    tableName,
    handleDelete,
    handleProcessRowUpdate,
    clientModel,
    handleReload,
    where,
    sort,
    allowFiltering,
    ops,
  ]);

  const slots = useMemo(() => {
    return {
      columnMenu: AppColumnMenu,
      toolbar: ({
        dbpal,
      }: GridSlotProps["toolbar"] & { dbpal: SqlTableCustomToolBarLeftProps }) => {
        return (
          <AppDataGridToolBar
            title={tableName}
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
      pagination: CustomPagination,
    } as Partial<GridSlotsComponent>;
  }, [isTable, kind, makePara, setUi, tableName, uuid]);
  const sortModel = clientModel ? "client" : "server";
  const { getColumnVisibility, setColumnVisibility } = useTableColumnVisibleStore(
    (s) => s,
  );
  return (
    <Stack style={{ height: "100%" }}>
      <QueryStatus status={queryStatus} />
      <OpsContext
        value={{
          ops,
          setOps,
          fields: resp.fields,
          uuid,
          handleReload: () => {
            handleReload(false);
          },
          clickedField: clickedField,
        }}>
        <DataGrid
          isCellEditable={() => {
            return isTable;
          }}
          slots={slots}
          apiRef={apiRef}
          onCellClick={() => {
            setClickedField("");
          }}
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
      </OpsContext>

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
          })}>
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

function Menu(props: {
  field: string;
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
  } = props;

  const { showNotification } = useNotification();
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

      {content && (
        <MenuItem
          onClick={() => {
            handleCopyToClipboard(content, showNotification);
          }}>
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

let lastCompare = "=";
function FilterOnValue(props: {
  content: string;
  setWhere;
  field: string;
  handleClose;
  kind: DataSourceKind;
  putValueInSql: (x: string) => Promise<string>;
}) {
  const { content, setWhere, field, handleClose, putValueInSql, kind } = props;
  const compares = ["=", "<>", ">", ">=", "<", "<="];
  const [compare, setCompare] = useState(lastCompare);
  const { showNotification } = useNotification();
  const [formatValue, setFormatValue] = useState("");
  useEffect(() => {
    lastCompare = compare;
  }, [compare]);
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
