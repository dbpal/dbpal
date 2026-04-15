import {
  MenuItem,
  ListItemIcon,
  ListItemText,
  Tooltip,
  Typography,
  MenuList,
  Stack,
  Divider,
  useTheme,
  Button,
  TextField,
  IconButton,
  Autocomplete,
  Select,
  FormControl,
  InputLabel,
  Modal,
  Box,
} from "@mui/material";
import RefreshIcon from "@mui/icons-material/Refresh";
import { ReactNode, useCallback, useEffect, useMemo, useRef, useState } from "react";
import DeleteForeverIcon from "@mui/icons-material/DeleteForever";
import {
  DataGrid,
  GridColDef,
  GridColumnMenu,
  GridColumnMenuItemProps,
  GridColumnMenuProps,
  GridSlotsComponent,
  GridSlotsComponentsProps,
  GridSortModel,
} from "@mui/x-data-grid";

import { FaFolderOpen } from "react-icons/fa";
import CheckIcon from "@mui/icons-material/Check";
import CloseIcon from "@mui/icons-material/Close";
import AddIcon from "@mui/icons-material/Add";
import { BsDatabaseAdd } from "react-icons/bs";
import LinkOffIcon from "@mui/icons-material/LinkOff";
import ContentCopyIcon from "@mui/icons-material/ContentCopy";
import ArrowUpwardIcon from "@mui/icons-material/ArrowUpward";
import ArrowDownwardIcon from "@mui/icons-material/ArrowDownward";
import {
  dataGridStyle,
  DataSourceKind,
  editorSetPositionAndReveal,
  ErrAndLoc,
  errToString,
  extractFieldAndIDFromMouseEvent,
  handleCopyToClipboard,
  pageSizeOptions,
  prettyMs,
  resizeHandlerSize,
  secondConfirm,
  textEllipsisStyle,
} from "./helper";
import { v4 as uuidv4 } from "uuid";

import { TbDatabasePlus } from "react-icons/tb";
import LinkIcon from "@mui/icons-material/Link";
import { EscDownEvent, modalHandleClose, modalStyle, PassWordManager } from "./Common";
import { TreeItem } from "@mui/x-tree-view/TreeItem";
import { editor, Position } from "monaco-editor";
import { openPath } from "@tauri-apps/plugin-opener";
import { FaMarkdown } from "react-icons/fa";
import {
  AppDataGridPagination,
  ClearableTextField,
  ListenElementSize,
} from "./CommonComp2";
import {
  passwordKey,
  savePasswordKey,
  useColumnWidthStore,
  useConnLoading,
  useConnected,
  useConnectionStore,
  useContextMenu,
  useElementSizeStore,
  useFavoriteStore,
  useNotification,
  useQueryLogStore,
  useUserSettings,
  useViewInModalMatchStrStore,
} from "./store";
import { commands, Field, MetaValue, NamedStaff } from "./bindings";

function MenuExportMarkDownTable({ showExportMarkDown }) {
  return (
    <MenuItem onClick={showExportMarkDown}>
      <ListItemIcon>
        <FaMarkdown fontSize="1.5rem" />
      </ListItemIcon>
      <ListItemText>Export MarkDown Table</ListItemText>
    </MenuItem>
  );
}
function LogLevelSelection({ handleChange, level }) {
  const levels = ["off", "trace", "debug", "info", "warn", "error"];
  return (
    <Autocomplete
      options={levels}
      style={{ width: "14rem" }}
      value={level}
      freeSolo={false}
      renderInput={(params) => <TextField {...params} label="logger level" />}
      onChange={(_, level) => {
        handleChange(level);
      }}
    />
  );
}

function MenuNewQuery({ handleNew }) {
  return (
    <MenuItem
      onClick={() => {
        handleNew();
      }}>
      <ListItemIcon>
        <AddIcon />
      </ListItemIcon>
      <ListItemText>New Query </ListItemText>
    </MenuItem>
  );
}

function MenuCreateDatabase({ handleCreate, kind }) {
  return (
    <MenuItem
      onClick={() => {
        handleCreate();
      }}>
      <ListItemIcon>
        <BsDatabaseAdd />
      </ListItemIcon>
      <ListItemText>
        Create&nbsp;
        {(function () {
          switch (kind) {
            case DataSourceKind.CouchBase:
              return "Bucket";
            default:
              return "Database";
          }
        })()}
      </ListItemText>
    </MenuItem>
  );
}
function MenuRefresh({ reload }) {
  return (
    <MenuItem
      onClick={() => {
        reload();
      }}>
      <ListItemIcon>
        <RefreshIcon />
      </ListItemIcon>
      <ListItemText> Refresh </ListItemText>
    </MenuItem>
  );
}

function MenuCloseConn({ name }) {
  const { showNotification } = useNotification();
  const { removeConnected } = useConnected();
  return (
    <MenuItem
      onClick={() => {
        commands
          .closeConn(name)
          .then(() => {
            removeConnected(name);
          })
          .catch((e) => {
            showNotification({
              severity: "error",
              title: "close connection failed",
              message: e,
            });
          });
      }}>
      <ListItemIcon>
        <LinkOffIcon />
      </ListItemIcon>
      <ListItemText> Close Connection </ListItemText>
    </MenuItem>
  );
}

function ConnPingTime({ uuid }) {
  const [pingTime, setStatus] = useState<number | null>(null);
  const [err, setErr] = useState("");

  useEffect(() => {
    const reloadStatus = () => {
      commands
        .checkConn(uuid)
        .then((data) => {
          setStatus(data);
          setErr("");
        })
        .catch((e) => {
          setErr(errToString(e));
          setStatus(null);
        });
    };
    const interval = setInterval(() => {
      reloadStatus();
    }, 5000);
    return () => {
      clearInterval(interval);
    };
  }, [uuid]);

  return (
    <>
      {typeof pingTime == "number" && (
        <Tooltip title="ping time">
          <Typography color="green"> {prettyMs(pingTime)} </Typography>
        </Tooltip>
      )}
      {err && (
        <Tooltip title={err}>
          <Typography color="error"> err </Typography>
        </Tooltip>
      )}
    </>
  );
}

function MenuCreateSchema(props: { handleCreate; kind: DataSourceKind }) {
  const { handleCreate, kind } = props;
  let name;
  switch (kind) {
    case DataSourceKind.SqlServer:
    case DataSourceKind.Postgres:
    case DataSourceKind.DuckDB:
    case DataSourceKind.Cockroach:
      name = "Schema";
      break;
    case DataSourceKind.Cassandra:
      name = "Key Space";
      break;
    case DataSourceKind.CouchBase:
      name = "Scope";
      break;
    default:
      name = "Database";
      break;
  }
  return (
    <MenuItem
      onClick={() => {
        handleCreate();
      }}>
      <ListItemIcon>
        <TbDatabasePlus />
      </ListItemIcon>
      <ListItemText> {`Create ${name}`} </ListItemText>
    </MenuItem>
  );
}

function MenuOpenConn({ name }) {
  const { showNotification } = useNotification();
  const setConnected = useConnected((s) => s.setConnected);
  const setLoading = useConnLoading((s) => s.setLoading);

  const { getConnection } = useConnectionStore();

  const handleOpenXXX = async () => {
    // try promote use to enter a password.
    const config = await getConnection(name);
    if (!config) {
      return;
    }
    try {
      await commands.establishConn(config);
    } catch (e) {
      showNotification({
        severity: "error",
        title: "establish connection failed",
        message: e,
      });
      return;
    }
    if (false === config[savePasswordKey]) {
      PassWordManager.savePassword(
        PassWordManager.serviceConnection,
        name,
        config[passwordKey],
        false,
      );
    }
    setConnected(name);
  };
  return (
    <MenuItem
      onClick={() => {
        setLoading(name, true);
        handleOpenXXX().finally(() => {
          setLoading(name, false);
        });
      }}>
      <ListItemIcon>
        <LinkIcon />
      </ListItemIcon>
      <ListItemText> Establish Connection </ListItemText>
    </MenuItem>
  );
}

function SelectPageSize({ pageSize, handleChange, options = pageSizeOptions }) {
  return (
    <FormControl style={{ width: "10rem" }}>
      <InputLabel>page size </InputLabel>
      <Select
        value={pageSize}
        onChange={(e) => {
          handleChange(e.target.value);
        }}
        label="page size">
        {options.map((e) => (
          <MenuItem value={e} key={e}>
            {e}
          </MenuItem>
        ))}
      </Select>
    </FormControl>
  );
}

function MenuDeleteConfig({ name }) {
  const { showNotification } = useNotification();

  const { removeConnection } = useConnectionStore();
  const { deleteByConnectionName: delFav } = useFavoriteStore();

  const { deleteByName: delQueryLog } = useQueryLogStore();
  const handleDelete = async () => {
    const confirm = await secondConfirm({
      title: `drop connection '${name}'`,
      confirmText: "Drop",
    });
    if (!confirm) {
      return;
    }
    try {
      await commands.closeConn(name);
      await delFav(name);
      await delQueryLog(name);
      await removeConnection(name);
    } catch (e) {
      showNotification({
        severity: "error",
        title: "drop connection failed",
        message: e,
      });
    }
  };
  return (
    <MenuItem
      onClick={() => {
        handleDelete();
      }}
      sx={{ color: "error.main" }}>
      <ListItemIcon>
        <DeleteForeverIcon color="error" />
      </ListItemIcon>
      <ListItemText color="error">Drop Connection</ListItemText>
    </MenuItem>
  );
}

function MetaValueViewMenu(props: { content: string; setSort; field: string }) {
  const { content, setSort, field } = props;
  const { showNotification } = useNotification();
  return (
    <MenuList>
      <MenuItem
        onClick={() => {
          handleCopyToClipboard(content, showNotification);
        }}>
        <ListItemIcon>
          <ContentCopyIcon />
        </ListItemIcon>
        <ListItemText> Copy </ListItemText>
      </MenuItem>
      <Divider />
      <MenuItem
        onClick={() => {
          setSort([{ field, sort: "asc" }] as GridSortModel);
        }}>
        <ListItemIcon>
          <ArrowUpwardIcon />
        </ListItemIcon>
        <ListItemText> {field} ASC </ListItemText>
      </MenuItem>
      <MenuItem
        onClick={() => {
          setSort([{ field, sort: "desc" }] as GridSortModel);
        }}>
        <ListItemIcon>
          <ArrowDownwardIcon />
        </ListItemIcon>
        <ListItemText> {field} DESC </ListItemText>
      </MenuItem>
    </MenuList>
  );
}

function MetaValueView({ values = [] as MetaValue[] }) {
  const setting = useUserSettings();

  const getColumnWidth = useColumnWidthStore((s) => s.getColumnWidth);
  const columns = [
    { field: "name", width: getColumnWidth("name"), flex: 1 },
    { field: "value", width: getColumnWidth("value"), flex: 1 },
  ] as GridColDef[];

  const [sortModel, setSortModel] = useState<GridSortModel>([]);
  values.forEach((e, index) => {
    if (!("id" in e)) {
      e["id"] = index;
    }
  });
  const setUpMenu = useContextMenu((state) => state.setUpMenu);
  const slotProps = useMemo(() => {
    return {
      row: {
        onContextMenu: (e) => {
          const { target } = e;
          const content = (target as HTMLDivElement).innerHTML;
          const field = extractFieldAndIDFromMouseEvent(e).field;
          if (!field) {
            return;
          }
          setUpMenu({
            content: (
              <MetaValueViewMenu
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
          delimiter: setting.export_.csvDelimiter,
          includeHeaders: setting.export_.includeHeader,
        },
      },
    } as GridSlotsComponentsProps;
  }, [setUpMenu, setting]);

  const onSortModelChange = useCallback((e) => {
    setSortModel(e);
  }, []);

  const slots = useMemo(() => {
    return {
      pagination: AppDataGridPagination,
    } as Partial<GridSlotsComponent>;
  }, []);
  return (
    <DataGrid
      slots={slots}
      style={dataGridStyle}
      sortModel={sortModel}
      onSortModelChange={onSortModelChange}
      slotProps={slotProps}
      columns={columns}
      rows={values}
    />
  );
}

function ResetColumnWidth() {
  const { showNotification } = useNotification();

  const reset = useColumnWidthStore((s) => s.resetWidths);

  const handleReset = async () => {
    reset();
    showNotification({ severity: "success", title: "reset column width successfully" });
  };
  return (
    <Tooltip title="reset column width">
      <Button onClick={handleReset} variant="outlined">
        <RefreshIcon />
        width
      </Button>
    </Tooltip>
  );
}

function TrueOrFalse(props: { value: boolean }) {
  const { value } = props;
  return value ? <CheckIcon> </CheckIcon> : <CloseIcon />;
}

function ModalHeader(props: {
  title: string;
  handleClose: () => void;
  noTextTransform?: boolean;
  headerExtra?: ReactNode;
}) {
  const { title, handleClose, noTextTransform, headerExtra } = props;
  const theme = useTheme();
  return (
    <Stack
      direction="row"
      style={{
        justifyContent: "space-between",
        alignItems: "center",
        borderBottom: "3px solid " + theme.palette.success.main,
        marginBottom: "0.4rem",
        position: "sticky",
        top: 0,
        backgroundColor: theme.palette.background.paper,
      }}>
      <Stack direction="row" style={{ alignItems: "flex-end" }}>
        <Typography
          variant="h6"
          style={{
            paddingBottom: "0.3rem",
            fontWeight: "bold",
            textTransform: noTextTransform ? undefined : "capitalize",
          }}>
          {title}
        </Typography>
        {headerExtra}
      </Stack>

      <Tooltip title={"esc can close this too"}>
        <IconButton color="secondary" onClick={handleClose}>
          <CloseIcon> </CloseIcon>
        </IconButton>
      </Tooltip>
    </Stack>
  );
}

function HighlightText({
  text,
  normal,
}: {
  text: string;
  normal: (text: string) => ReactNode;
}) {
  const matchStr = useViewInModalMatchStrStore((state) => state.matchStr);

  if (!matchStr) return normal(text);

  // 使用正则拆分字符串，'ig' 表示忽略大小写且全局匹配
  const parts = text.split(new RegExp(`(${matchStr})`, "gi"));

  return (
    <span>
      {parts.map((part, i) =>
        part.toLowerCase() === matchStr.toLowerCase() ? (
          <mark key={i}>{part}</mark>
        ) : (
          part
        ),
      )}
    </span>
  );
}

function ViewRowInModal(props: {
  fields: Field[];
  data: object;
  isTable?;
  handleClose: () => void;
}) {
  const { fields, data, isTable, handleClose } = props;
  const theme = useTheme();
  const { matchStr, setMatchStr } = useViewInModalMatchStrStore();

  const [viewInModalBox, setViewInModalBox] = useState(null);
  if (!data) {
    return <> </>;
  }

  return (
    <Modal
      open={true}
      onClose={modalHandleClose((e: EscDownEvent) => {
        e.preventDefault();
        handleClose();
      })}>
      <Box
        sx={{ resize: "both", ...modalStyle, width: "80%", height: "30vh" }}
        ref={setViewInModalBox}>
        <ModalHeader
          title="View Row"
          handleClose={() => {
            handleClose();
          }}
          headerExtra=<>
            <ClearableTextField
              value={matchStr}
              onClear={() => {
                setMatchStr("");
              }}
              onChange={(e) => {
                setMatchStr(e.target.value);
              }}
              style={{ width: "13rem" }}
              label="match string,only for value"
            />
          </>
        />
        <ListenElementSize
          dom={viewInModalBox}
          set="both"
          name="query-resp-view-in-modals"
        />

        <Stack>
          {fields.map((f) => {
            return (
              <Stack
                style={{
                  borderRadius: "10px",
                  padding: "8px",
                  border: "2px solid " + theme.palette.success.main,
                }}
                key={f.field}>
                <Stack
                  direction="row"
                  style={{ alignItems: "center" }}
                  divider={<span>|</span>}>
                  <Typography variant="subtitle1">{f.field}</Typography>
                  <Typography variant="body1">{f.data_type}</Typography>
                  {isTable && f.is_primary && (
                    <Typography variant="body1"> Primary Key </Typography>
                  )}
                  {isTable && f.nullable && (
                    <Typography variant="body1"> nullable </Typography>
                  )}
                  {isTable && f.is_auto_increment && (
                    <Typography variant="body1"> auto increment </Typography>
                  )}
                  {isTable && f.default_value && (
                    <Typography variant="body1" style={textEllipsisStyle}>
                      default value({f.default_value})
                    </Typography>
                  )}
                </Stack>
                <Divider />
                <HighlightText
                  text={data[f.field]}
                  normal={(text) => {
                    return (
                      <pre
                        style={{
                          overflow: "auto",
                          opacity: 0.9,
                          color: theme.palette.success.main,
                        }}>
                        {text}{" "}
                      </pre>
                    );
                  }}
                />
              </Stack>
            );
          })}
        </Stack>
      </Box>
    </Modal>
  );
}

function ExceedMaxTreeItem(props: { showItems: number; maxTreeSize: number }) {
  const { showItems, maxTreeSize } = props;
  const id = useRef(uuidv4());
  return (
    <TreeItem
      itemId={id.current}
      style={{ display: showItems > maxTreeSize ? "block" : "none" }}
      label=<Typography color="error" style={{ fontSize: "0.8rem" }}>
        ({showItems}) exceed max tree items({maxTreeSize}),some item not doingIt
      </Typography>
    />
  );
}

function UdfNameAndParameters(props: { udf: NamedStaff; kind: DataSourceKind }) {
  const { udf, kind } = props;
  let parameter = "";
  switch (kind) {
    case DataSourceKind.Postgres:
    case DataSourceKind.Cockroach:
      parameter = udf.parameters;
      break;
    case DataSourceKind.Cassandra:
      parameter = udf.cassandra_parameters.join(",");
      break;
  }

  return (
    <>
      <span style={textEllipsisStyle}>{udf.name}</span>
      {parameter && (
        <Typography
          color="success"
          style={{
            ...textEllipsisStyle,
            fontStyle: "italic",
            fontSize: "0.8rem",
          }}>
          ({parameter})
        </Typography>
      )}
    </>
  );
}

function PossibleParseError(props: {
  parseErr: ErrAndLoc;
  editor: editor.IStandaloneCodeEditor;
}) {
  const { parseErr, editor } = props;
  return (
    <Tooltip title="possible error" followCursor>
      <Typography
        style={{
          cursor: parseErr.line && parseErr.column && "pointer",
          ...textEllipsisStyle,
        }}
        onClick={() => {
          if (!parseErr) {
            return;
          }
          if (parseErr.line && parseErr.column) {
            editorSetPositionAndReveal(
              editor,
              new Position(parseErr.line, parseErr.column),
            );
          }
        }}>
        {parseErr.msg}
      </Typography>
    </Tooltip>
  );
}

function CopyMenuItem(props: GridColumnMenuItemProps) {
  const { colDef } = props;
  const { showNotification } = useNotification();
  return (
    <MenuItem
      onClick={() => {
        handleCopyToClipboard(colDef.field, showNotification);
      }}>
      <ListItemIcon> </ListItemIcon>
      <ListItemText> Copy Field </ListItemText>
    </MenuItem>
  );
}

function AppColumnMenu(props: GridColumnMenuProps) {
  return (
    <GridColumnMenu
      {...props}
      slots={{
        // Add new item
        closeColumnMenuCopyField: CopyMenuItem,
      }}
      slotProps={{
        closeColumnMenuCopyField: {
          displayOrder: 15,
        },
      }}
    />
  );
}

function SelectColumns(props: {
  fields: string[];
  includes: string[];
  setIncludes;
  includeAll: boolean;
}) {
  const { fields, includes, setIncludes, includeAll } = props;
  return (
    <Stack direction="row" style={{ alignItems: "flex-end" }}>
      {includeAll && fields.length > includes.length && (
        <Button
          onClick={() => {
            setIncludes(fields);
          }}
          variant="text">
          all
        </Button>
      )}
      <Autocomplete
        style={{ flex: 1 }}
        options={fields}
        value={includes}
        multiple
        onChange={(_, v) => setIncludes(v)}
        renderInput={(params) => <TextField {...params} label="select columns" />}
      />
    </Stack>
  );
}

function OpenFileWithDefaultApp({ file }) {
  const { showNotification } = useNotification();
  return (
    <>
      {file && (
        <Tooltip title="open with default app (if applicable)">
          <IconButton
            onClick={() => {
              openPath(file)
                .then(() => {})
                .catch((e) => {
                  showNotification({
                    severity: "warning",
                    title: "open export file failed",
                    message: e,
                  });
                });
            }}>
            <FaFolderOpen fontSize="1.5rem" />
          </IconButton>
        </Tooltip>
      )}
    </>
  );
}

function ResizeParentHeight(props: {
  box: HTMLDivElement;
  min: number;
  saveHeightName;
}) {
  const { box, min, saveHeightName } = props;
  const { elements, setElement } = useElementSizeStore();
  useEffect(() => {
    const size = elements[saveHeightName];
    if (size && size.height) {
      box.style.height = size.height + "px";
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [box.style, saveHeightName]);

  useEffect(() => {}, []);
  const handleDown = useCallback(
    (e: React.MouseEvent<HTMLDivElement>) => {
      e.preventDefault();
      const startY = e.clientY;
      const startH = box.offsetHeight;
      const onMove = (ev: MouseEvent) => {
        let newH = startH + ev.clientY - startY;
        if (newH < min) {
          newH = min;
        }
        setElement(saveHeightName, { height: newH, width: null });
        box.style.height = newH + "px";
      };
      const onUp = () => {
        document.removeEventListener("mousemove", onMove);
        document.removeEventListener("mouseup", onUp);
      };
      document.addEventListener("mousemove", onMove);
      document.addEventListener("mouseup", onUp);
    },
    [box.offsetHeight, box.style, min, saveHeightName, setElement],
  );

  return (
    <div
      style={{ height: resizeHandlerSize, cursor: "ns-resize" }}
      className="resize-handle"
      onMouseDown={handleDown}
    />
  );
}

export {
  LogLevelSelection,
  MetaValueView,
  MenuNewQuery,
  MenuCreateDatabase,
  MenuRefresh,
  ConnPingTime,
  MenuCloseConn,
  MenuCreateSchema,
  MenuOpenConn,
  SelectPageSize,
  MenuDeleteConfig,
  ResetColumnWidth,
  TrueOrFalse,
  ModalHeader,
  ViewRowInModal,
  ExceedMaxTreeItem,
  UdfNameAndParameters,
  PossibleParseError,
  AppColumnMenu,
  SelectColumns,
  OpenFileWithDefaultApp,
  MenuExportMarkDownTable,
  ResizeParentHeight,
  HighlightText,
};
