import {
  MenuItem,
  ListItemIcon,
  ListItemText,
  Tooltip,
  Typography,
  MenuList,
  Stack,
  Divider,
  Chip,
  useTheme,
  Button,
  TextField,
  IconButton,
  Autocomplete,
  Select,
  FormControl,
  InputLabel,
} from "@mui/material";
import RefreshIcon from "@mui/icons-material/Refresh";
import { ReactNode, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { types } from "./ApiStruct";
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
import { invoke } from "@tauri-apps/api/core";
import ContentCopyIcon from "@mui/icons-material/ContentCopy";
import ArrowUpwardIcon from "@mui/icons-material/ArrowUpward";
import ArrowDownwardIcon from "@mui/icons-material/ArrowDownward";
import { IoTimerOutline } from "react-icons/io5";
import {
  dataGridStyle,
  DataSourceKind,
  editorSetPositionAndReveal,
  ErrAndLoc,
  errToString,
  extractFieldAndIDFromMouseEvent,
  FavoriteSql,
  generateRandomString,
  handleCopyToClipboard,
  pageSizeOptions,
  prettyMs,
  QueryLogHandler,
  resizeHandlerSize,
  secondConfirm,
  shrinkAble,
  textEllipsisStyle,
} from "./helper";

import { TbDatabasePlus } from "react-icons/tb";
import LinkIcon from "@mui/icons-material/Link";
import { ConnectionHandler, connectionChangeEvent, PassWordManager } from "./Common";
import { TreeItem } from "@mui/x-tree-view/TreeItem";
import { editor, Position } from "monaco-editor";
import { openPath } from "@tauri-apps/plugin-opener";
import { FaMarkdown } from "react-icons/fa";
import { emit } from "@tauri-apps/api/event";
import { AppDataGridPagination, StackRowCenter } from "./CommonComp2";
import {
  useColumnWidthStore,
  useConnLoading,
  useConnected,
  useContextMenu,
  useElementSizeStore,
  useNotification,
  useUserSettings,
} from "./store";

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
  const { showNotification } = useNotification((s) => s);
  const { removeConnected } = useConnected((s) => s);
  return (
    <MenuItem
      onClick={() => {
        invoke("close_conn", { name })
          .then(() => {
            removeConnected(name);
          })
          .catch((e) => {
            showNotification("error", "close connection failed", e);
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
      invoke<number>("check_conn", { name: uuid })
        .then((ping) => {
          setStatus(ping);
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
  const { showNotification } = useNotification((s) => s);

  const setConnected = useConnected((s) => s.setConnected);

  const setLoading = useConnLoading((s) => s.setLoading);

  const handleOpenXXX = async () => {
    // try promote use to enter a password.
    const config = await ConnectionHandler.getByName(name);
    if (!config) {
      return;
    }
    const key = "save_password";
    let inputPass = "";
    if (false === config[key]) {
      const pass = PassWordManager.getPassword(name);
      if (!pass) {
        inputPass = window.prompt(`enter password for '${name}'`);
        // macos will return null directly
        if (inputPass == null) {
          return;
        }
        config["password"] = inputPass;
      } else {
        config["password"] = pass;
      }
    } else {
      // get password from keyring
      const pass = await PassWordManager.getPassword(name);
      if (pass) {
        config["password"] = pass;
      }
    }
    try {
      await invoke("establish_conn", { config });
    } catch (e) {
      showNotification("error", "establish connection failed", e);
      return;
    }
    if (inputPass) {
      PassWordManager.savePassword(name, inputPass, false);
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
  const { showNotification } = useNotification((s) => s);
  const handleDelete = async () => {
    const confirm = await secondConfirm({
      title: `drop connection '${name}'`,
      confirmText: "Drop",
    });
    if (!confirm) {
      return;
    }
    try {
      await invoke("close_conn", { name });
      await FavoriteSql.deleteByConnectionName(name);
      await QueryLogHandler.deleteByConnectionName(name);
      await ConnectionHandler.deleteConn(name);
      await PassWordManager.deletePassword(name);
      emit(connectionChangeEvent, "");
    } catch (e) {
      showNotification("error", "drop connection failed", e);
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
  const { showNotification } = useNotification((s) => s);
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

function MetaValueView({ values = [] as types.MetaValue[] }) {
  const setting = useUserSettings((s) => s);

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
  const { showNotification } = useNotification((s) => s);

  const reset = useColumnWidthStore((s) => s.resetWidths);

  const handleReset = async () => {
    reset();
    showNotification("success", "reset column width successfully");
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
      }}>
      <StackRowCenter>
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
      </StackRowCenter>

      <Tooltip title={"esc can close this too"}>
        <IconButton color="secondary" onClick={handleClose}>
          <CloseIcon> </CloseIcon>
        </IconButton>
      </Tooltip>
    </Stack>
  );
}

function ViewRowInModal(props: { fields: types.Field[]; data: object; isTable? }) {
  const { fields, data, isTable } = props;
  const theme = useTheme();
  return (
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
            <pre
              style={{
                overflow: "auto",
                opacity: 0.9,
                color: theme.palette.success.main,
              }}>
              {data[f.field]}
            </pre>
          </Stack>
        );
      })}
    </Stack>
  );
}

function ExceedMaxTreeItem(props: { exceed: number; maxTreeSize: number }) {
  const { exceed, maxTreeSize } = props;
  const id = useRef(generateRandomString());
  return (
    <TreeItem
      itemId={id.current}
      style={{ display: exceed > maxTreeSize ? "block" : "none" }}
      label=<Typography color="error" style={{ fontSize: "0.8rem" }}>
        ({exceed}) exceed max tree items({maxTreeSize}),some item not showing
      </Typography>
    />
  );
}

function UdfNameAndParameters(props: { udf: types.NamedStaff; kind: DataSourceKind }) {
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
      <Typography>{udf.name}</Typography>
      {parameter && (
        <Typography
          color="success"
          style={{
            fontStyle: "italic",
            fontSize: "0.8rem",
            ...textEllipsisStyle,
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

function TimeDuration(props: { dur: number }) {
  const { dur } = props;
  return (
    !!dur && (
      <Chip
        label={prettyMs(dur)}
        color={dur > 4000 ? "warning" : "info"}
        icon={<IoTimerOutline fontSize="1.2rem" />}
      />
    )
  );
}

function ExecutingTime(props: {
  loading: boolean;
  disableTime?: boolean;
  title?: string;
}) {
  let { loading, disableTime, title } = props;
  if (!title) {
    title = "executing time";
  }
  const executeTimeRef = useRef(null);
  const timeExecuteClean = useRef<() => void>(null);
  const [lastLoadTime, setLastLoadTime] = useState(new Date());
  const tryExecuteTimeClean = () => {
    if (timeExecuteClean.current) {
      timeExecuteClean.current();
      timeExecuteClean.current = null;
    }
  };
  useEffect(() => {
    return () => {
      tryExecuteTimeClean();
    };
  }, []);
  const timeExecute = () => {
    const now = new Date();
    const setUp = () => {
      const ms = new Date().getTime() - now.getTime();
      (executeTimeRef.current as HTMLDivElement).innerHTML = prettyMs(ms);
    };
    const int = setInterval(() => {
      setUp();
    }, 100);
    timeExecuteClean.current = () => {
      clearInterval(int);
      setUp();
    };
  };
  useEffect(() => {
    tryExecuteTimeClean();
    if (!loading) {
      setLastLoadTime(new Date());
    } else {
      timeExecute();
    }
  }, [loading]);
  return (
    <>
      <Tooltip title={title}>
        <Chip
          color="success"
          label={<span ref={executeTimeRef}> </span>}
          style={shrinkAble}
          icon={<IoTimerOutline fontSize="1.2rem" />}
        />
      </Tooltip>
      {!disableTime && (
        <Tooltip title="loaded at">
          <Typography style={shrinkAble}>
            {lastLoadTime.toLocaleTimeString()}
          </Typography>
        </Tooltip>
      )}
    </>
  );
}

function CopyMenuItem(props: GridColumnMenuItemProps) {
  const { colDef } = props;
  const { showNotification } = useNotification((s) => s);
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
  const { showNotification } = useNotification((s) => s);
  return (
    <>
      {file && (
        <Tooltip title="open with default app (if applicable)">
          <IconButton
            onClick={() => {
              openPath(file)
                .then(() => {})
                .catch((e) => {
                  showNotification("warning", "open export file failed", e);
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
  const { elements, setElement } = useElementSizeStore((s) => s);
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
  ExecutingTime,
  AppColumnMenu,
  SelectColumns,
  OpenFileWithDefaultApp,
  MenuExportMarkDownTable,
  ResizeParentHeight,
  TimeDuration,
};
