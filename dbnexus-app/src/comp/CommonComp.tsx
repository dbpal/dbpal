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
  InputAdornment,
} from "@mui/material";
import InfoIcon from "@mui/icons-material/Info";
import AttachFileIcon from "@mui/icons-material/AttachFile";
import RefreshIcon from "@mui/icons-material/Refresh";
import {
  CSSProperties,
  Dispatch,
  ReactNode,
  SetStateAction,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
} from "react";
import ClearIcon from "@mui/icons-material/Clear";
import DeleteForeverIcon from "@mui/icons-material/DeleteForever";
import { FaFolderOpen } from "react-icons/fa";
import CheckIcon from "@mui/icons-material/Check";
import CloseIcon from "@mui/icons-material/Close";
import AddIcon from "@mui/icons-material/Add";
import { BsDatabaseAdd } from "react-icons/bs";
import LinkOffIcon from "@mui/icons-material/LinkOff";
import ContentCopyIcon from "@mui/icons-material/ContentCopy";
import ArrowUpwardIcon from "@mui/icons-material/ArrowUpward";
import ArrowDownwardIcon from "@mui/icons-material/ArrowDownward";
import KeyboardArrowDownIcon from "@mui/icons-material/KeyboardArrowDown";
import KeyboardArrowRightIcon from "@mui/icons-material/KeyboardArrowRight";
import {
  arrayToMarkdownTable,
  dataGridStyle,
  DataSourceKind,
  errToString,
  escapeTSV,
  extractFieldAndIDFromMouseEvent,
  handleCopyToClipboard,
  pageSizeOptions,
  parseCellType,
  prettyMs,
  resizeHandlerSize,
  secondConfirm,
  AppColumnMenuCtx,
  textEllipsisStyle,
  DbnameContext,
  calVirtualRollHeight,
  TreeIdent,
} from "./helper";

import { TbDatabasePlus } from "react-icons/tb";
import LinkIcon from "@mui/icons-material/Link";
import { EscDownEvent, modalHandleClose, modalStyle, PassWordManager } from "./Common";
import { openPath } from "@tauri-apps/plugin-opener";
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
  useFontSize,
  useNotification,
  useQueryLogStore,
  useUserSettings,
  useViewInModalMatchStrStore,
} from "./store";
import { commands, Field, MetaValue, NamedStaff } from "./bindings";

import { open } from "@tauri-apps/plugin-dialog";
import {
  DataGrid,
  GridColDef,
  GridColumnMenu,
  GridColumnMenuItemProps,
  GridColumnMenuProps,
  GridSlotsComponent,
  GridSlotsComponentsProps,
  GridSortModel,
  useGridApiContext,
} from "./datagridExport";
import { AutoSizer, List } from "react-virtualized";

/**
 * Logger level selection control.
 */
function LogLevelSelection(props: {
  handleChange: (level: string | null) => void;
  level: string | null;
}) {
  const { handleChange, level } = props;
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

/**
 * Menu item for creating a new query.
 */
function MenuNewQuery(props: { handleNew: () => void }) {
  const { handleNew } = props;
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

function MenuNewQueryNewWindow(props: { handleNew: () => void }) {
  const { handleNew } = props;
  return (
    <MenuItem
      onClick={() => {
        handleNew();
      }}>
      <ListItemIcon>
        <AddIcon />
      </ListItemIcon>
      <ListItemText>New Query(new window) </ListItemText>
    </MenuItem>
  );
}

/**
 * Menu item for creating a database or bucket.
 */
function MenuCreateDatabase(props: { handleCreate: () => void; kind: DataSourceKind }) {
  const { handleCreate, kind } = props;
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

/**
 * Menu item for refreshing current data.
 */
function MenuRefresh(props: { refresh: () => void }) {
  const { refresh } = props;
  return (
    <MenuItem
      onClick={() => {
        refresh();
      }}>
      <ListItemIcon>
        <RefreshIcon />
      </ListItemIcon>
      <ListItemText> Refresh </ListItemText>
    </MenuItem>
  );
}

/**
 * Menu item for closing a named connection.
 */
function MenuCloseConn(props: { name: string }) {
  const { name } = props;
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

/**
 * Displays ping statistics for a connection UUID.
 */
function ConnPingTime(props: { dbName: string }) {
  const { dbName } = props;
  const [pingTime, setStatus] = useState<number | null>(null);
  const [err, setErr] = useState("");

  useEffect(() => {
    const reloadStatus = () => {
      commands
        .checkConn(dbName)
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
  }, [dbName]);

  return (
    <>
      {typeof pingTime == "number" && (
        <Tooltip title="ping time">
          <Typography color={pingTime < 4000 ? "success" : "warning"}>
            {prettyMs(pingTime)}
          </Typography>
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

/**
 * Menu item for creating a schema or equivalent namespace object.
 */
function MenuCreateSchema(props: { handleCreate: () => void; kind: DataSourceKind }) {
  const { handleCreate, kind } = props;
  let name = "";
  switch (kind) {
    case DataSourceKind.SqlServer:
    case DataSourceKind.Postgres:
    case DataSourceKind.Cockroach:
    case DataSourceKind.DuckDB:
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

/**
 * Menu item that opens an existing connection and handles credential persistence.
 */
function MenuOpenConn(props: { name: string }) {
  const { name } = props;
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

/**
 * Page size selector for table pagination.
 */
function SelectPageSize(props: {
  pageSize: number;
  handleChange: (value: number) => void;
  options?: number[];
}) {
  const { pageSize, handleChange, options = pageSizeOptions } = props;
  return (
    <FormControl style={{ width: "10rem" }}>
      <InputLabel>page size </InputLabel>
      <Select
        value={pageSize}
        onChange={(e) => {
          handleChange(e.target.value as number);
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

/**
 * Menu item to show available actions for a metadata value column.
 */
function MenuDeleteConfig(props: { name: string }) {
  const { name } = props;
  const { showNotification } = useNotification();

  const { removeConnection } = useConnectionStore();
  const { deleteByConnectionName: delFav } = useFavoriteStore();
  const { removeConnected } = useConnected();
  const { removeLoading } = useConnLoading();
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
      removeConnection(name);
      removeConnected(name);
      removeLoading(name);
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

/**
 * Menu actions for sorting or copying metadata values.
 */
function MetaValueViewMenu(props: {
  content: string;
  setSort: Dispatch<SetStateAction<GridSortModel>>;
  field: string;
}) {
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

/**
 * Table view for metadata values.
 */
function MetaValueView(props: { values: MetaValue[]; showToolbar: boolean }) {
  const { values, showToolbar } = props;

  const getColumnWidth = useColumnWidthStore((s) => s.getColumnWidth);
  const columns = useMemo(() => {
    const columns: GridColDef[] = [{ field: "name" }, { field: "value" }];
    columns.forEach((e) => {
      e.width = getColumnWidth(e.field);
    });
    return columns;
  }, [getColumnWidth]);

  const [sortModel, setSortModel] = useState<GridSortModel>([]);
  values.forEach((e, index) => {
    if (!("id" in e)) {
      e["id"] = index;
    }
  });
  const setUpMenu = useContextMenu((state) => state.setUpMenu);
  const slotProps = useMemo((): GridSlotsComponentsProps => {
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
    };
  }, [setUpMenu]);

  const onSortModelChange = useCallback((e) => {
    setSortModel(e);
  }, []);

  const slots = useMemo((): Partial<GridSlotsComponent> => {
    return {
      pagination: AppDataGridPagination,
    };
  }, []);
  return (
    <DataGrid
      showToolbar={showToolbar}
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

/**
 * Button to reset saved column widths.
 */
function ResetColumnWidth() {
  const { showNotification } = useNotification();
  const { resetWidths, columnWidths } = useColumnWidthStore();
  const handleReset = async () => {
    resetWidths();
    showNotification({ severity: "success", title: "reset column width successfully" });
  };
  return (
    <Tooltip title="reset column width">
      <Button
        onClick={handleReset}
        variant="outlined"
        disabled={Object.keys(columnWidths).length == 0}>
        <RefreshIcon />
        width
      </Button>
    </Tooltip>
  );
}

/**
 * Icon indicator for boolean values.
 */
function CheckedView(props: { value: boolean }) {
  const { value } = props;
  return value ? <CheckIcon /> : <CloseIcon />;
}

/**
 * Header used inside modal dialogs.
 */
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
      <IconButton color="secondary" onClick={handleClose}>
        <CloseIcon> </CloseIcon>
      </IconButton>
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

  if (!matchStr || !text) return normal(text);

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

/**
 * Modal view for inspecting a single row of query results.
 */
function ViewRowInModal(props: {
  fields: Field[];
  data: Record<string, any>;
  isTable?: boolean;
  handleClose: () => void;
}) {
  const { fields, data, isTable, handleClose } = props;
  const theme = useTheme();
  const { matchStr, setMatchStr } = useViewInModalMatchStrStore();
  const { showNotification } = useNotification();
  const [viewInModalBox, setViewInModalBox] = useState<HTMLDivElement | null>(null);
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
        sx={{
          resize: "both",
          ...modalStyle,
          width: "80%",
          height: "30vh",
          backgroundColor: theme.palette.background.paper,
        }}
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
              style={{ width: "16rem" }}
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
                  {data[f.field] && (
                    <IconButton
                      onClick={() => {
                        handleCopyToClipboard(data[f.field], showNotification);
                      }}>
                      <ContentCopyIcon />
                    </IconButton>
                  )}

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
                        {text}
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

/**
 * Display UDF name and parameters in metadata lists.
 */
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
      <span style={{ ...textEllipsisStyle }}>{udf.name}</span>
      {parameter && (
        <Typography
          color="info"
          style={{
            ...textEllipsisStyle,
            fontStyle: "italic",
            fontSize: "0.8rem",
            flexShrink: 6,
          }}>
          ({parameter})
        </Typography>
      )}
    </>
  );
}

function DataGridExtraMenuList(props: GridColumnMenuItemProps) {
  const { colDef, onClick } = props;
  const { showNotification } = useNotification();
  const { dbName, kind } = useContext(DbnameContext);
  const api = useGridApiContext();
  let { selectColumns } = useContext(AppColumnMenuCtx);
  const getSelectFields = (): string[] => {
    if (selectColumns.length) {
      if (selectColumns.includes(colDef.field)) {
        return selectColumns;
      } else {
        return [...selectColumns, colDef.field];
      }
    } else {
      return [colDef.field];
    }
  };

  return (
    <MenuList>
      <MenuItem
        onClick={(event) => {
          handleCopyToClipboard(
            api.current
              .getSortedRowIds()
              .map((id) =>
                getSelectFields()
                  .map((f) => api.current.getCellValue(id, f) + "")
                  .join("\t"),
              )
              .join("\n"),
            showNotification,
          );

          if (onClick) onClick(event);
        }}>
        <ListItemIcon>{/* <ContentCopyIcon /> */}</ListItemIcon>
        <ListItemText> Copy </ListItemText>
      </MenuItem>

      <MenuItem
        onClick={(event) => {
          const fs = getSelectFields();
          handleCopyToClipboard(
            api.current
              .getSortedRowIds()
              .map((id) =>
                fs.map((f) => escapeTSV(api.current.getCellValue(id, f))).join("\t"),
              )
              .join("\n"),
            showNotification,
          );

          if (onClick) onClick(event);
        }}>
        <ListItemIcon>{/* <ContentCopyIcon /> */}</ListItemIcon>
        <ListItemText> Copy as TSV for Excel </ListItemText>
      </MenuItem>

      <MenuItem
        onClick={(event) => {
          let r = [];
          let fs = getSelectFields();
          r.push(fs);

          api.current.getSortedRowIds().forEach((id) => {
            const obj = [];

            for (const f of fs) {
              let value = api.current.getCellValue(id, f);
              obj.push(value ? value : "");
            }
            r.push(obj);
          });
          handleCopyToClipboard(arrayToMarkdownTable(r), showNotification);
          if (onClick) onClick(event);
        }}>
        <ListItemIcon>{/* <ContentCopyIcon /> */}</ListItemIcon>
        <ListItemText> Copy as MarkDown </ListItemText>
      </MenuItem>

      {kind != DataSourceKind.Mongo &&
        kind != DataSourceKind.Redis &&
        kind != DataSourceKind.CouchBase && (
          <MenuItem
            onClick={async (event) => {
              try {
                let values = api.current
                  .getSortedRowIds()
                  .map((id) => api.current.getCellValue(id, colDef.field));
                values = await commands.putValuesInSql(
                  dbName,
                  colDef["data_type"],
                  values,
                );
                handleCopyToClipboard(
                  `(\n${values.join(",\n")}\n)\n`,
                  showNotification,
                );
                if (onClick) onClick(event);
              } catch (e) {
                showNotification({
                  severity: "error",
                  title: "format value in sql failed",
                  message: e,
                });
              }
            }}>
            <ListItemIcon>{/* <ContentCopyIcon /> */}</ListItemIcon>
            <ListItemText> Copy For in Statament </ListItemText>
          </MenuItem>
        )}

      <MenuItem
        onClick={(event) => {
          let r = [];
          let fs = getSelectFields();
          api.current.getSortedRowIds().forEach((id) => {
            let o = {};
            for (const f of fs) {
              o[f] = parseCellType(api.current.getCellValue(id, f));
            }
            r.push(o);
          });
          handleCopyToClipboard(JSON.stringify(r), showNotification);
          if (onClick) onClick(event);
        }}>
        <ListItemIcon>{/* <ContentCopyIcon /> */}</ListItemIcon>
        <ListItemText> Copy as JSON </ListItemText>
      </MenuItem>
      <MenuItem
        onClick={(event) => {
          handleCopyToClipboard(colDef.field, showNotification);
          if (onClick) onClick(event);
        }}>
        <ListItemIcon>{/* <ContentCopyIcon /> */}</ListItemIcon>
        <ListItemText> Copy Column Name </ListItemText>
      </MenuItem>
      <MenuItem
        onClick={(event) => {
          handleCopyToClipboard(colDef["data_type"], showNotification);
          if (onClick) onClick(event);
        }}>
        <ListItemIcon>{/* <ContentCopyIcon /> */}</ListItemIcon>
        <ListItemText> Copy Column Type </ListItemText>
      </MenuItem>
    </MenuList>
  );
}

/**
 * Grid column menu with custom copy field action.
 */
function AppColumnMenu(props: GridColumnMenuProps) {
  return (
    <GridColumnMenu
      {...props}
      slots={{
        // Add new item
        extraMenu: DataGridExtraMenuList,
      }}
      slotProps={{
        extraMenu: {
          displayOrder: 1,
        },
      }}
    />
  );
}

/**
 * Multi-select column chooser.
 */
function SelectColumns(props: {
  fields: string[];
  includes: string[];
  setIncludes: (includes: string[]) => void;
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

/**
 * Button to open a file path with the system default application.
 */
function OpenFileWithDefaultApp(props: { file?: string }) {
  const { file } = props;
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

/**
 * Resize handle that persists parent height in the element size store.
 */
function ResizeParentHeight(props: {
  box: HTMLDivElement;
  min: number;
  saveHeightName: string;
}) {
  const { box, min, saveHeightName } = props;
  const { elements, setElement } = useElementSizeStore();
  useEffect(() => {
    const size = elements[saveHeightName];
    if (size && size.height) {
      // eslint-disable-next-line react-hooks/immutability
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

/**
 * Path selector input with a file chooser button.
 */
function SelectPath(props: {
  path: string;
  setPath: Dispatch<SetStateAction<string>>;
  label: string;
  required: boolean;
  style?: CSSProperties;
}) {
  const { path, setPath, label, required, style } = props;
  return (
    <TextField
      value={path}
      onChange={(event) => {
        setPath(event.target.value);
      }}
      style={{ flex: 1, ...style }}
      label={label}
      required={required}
      slotProps={{
        input: {
          endAdornment: (
            <>
              {path && (
                <InputAdornment position="end">
                  <IconButton
                    onClick={() => {
                      setPath("");
                    }}
                    edge="end">
                    <ClearIcon fontSize="small" />
                  </IconButton>
                </InputAdornment>
              )}
              <IconButton
                onClick={() => {
                  open({ multiple: false }).then((openOk) => {
                    if (openOk) {
                      setPath(openOk);
                    }
                  });
                }}
                color="primary">
                <AttachFileIcon />
              </IconButton>
            </>
          ),
        },
      }}
    />
  );
}

function LongNotice({ children }) {
  return (
    <Tooltip
      slotProps={{
        tooltip: {
          style: { maxWidth: "40vw" },
        },
      }}
      title=<Stack style={{ width: "38vw" }}>{children}</Stack>>
      <InfoIcon color="info" />
    </Tooltip>
  );
}

function VirtualListLeft<T>(props: {
  data: T[];
  makeMenu: (_: T) => ReactNode;
  show: (_: T) => ReactNode;
}) {
  const { data, makeMenu, show } = props;
  const { appearance } = useUserSettings();
  const height = calVirtualRollHeight(data.length, appearance.fontSize);
  const { setUpMenu } = useContextMenu();
  if (data.length == 0) {
    return;
  }
  return (
    <div
      style={{
        overflow: "auto",
        height: height.height,
      }}>
      <AutoSizer>
        {({ width }) => (
          <List
            width={(function () {
              let r = width - 13;
              let min = 10;
              if (r < min) {
                r = min;
              }
              return r;
            })()}
            height={height.height} //   列表高度（可视区域）
            rowCount={data.length} // 总行数
            rowHeight={height.rowHeight} // 每行高度
            rowRenderer={({ index, style }) => {
              const d = data[index];
              return (
                <Stack
                  key={index}
                  style={{
                    ...style,
                    cursor: "context-menu",
                  }}
                  direction="row"
                  sx={{ alignItems: "center" }}
                  onContextMenu={(e) => {
                    setUpMenu({
                      content: makeMenu(d),
                      event: e,
                    });
                  }}>
                  {show(d)}
                </Stack>
              );
            }} // 渲染函数
          />
        )}
      </AutoSizer>
    </div>
  );
}

function MyTreeItem(props: {
  label: ReactNode;
  children: ReactNode;
  makeMenu?: () => ReactNode;
  conn_established: boolean;
}) {
  const ident = useContext(TreeIdent);
  const [collasped, setCollasped] = useState(true);
  const { label, children, makeMenu, conn_established } = props;
  const { setUpMenu } = useContextMenu();
  const fontSize = useFontSize();
  let identNes = ident + Math.ceil(fontSize * 0.5);
  return (
    <Stack style={{ paddingLeft: ident + "px" }}>
      <Stack
        onClick={() => {
          if (!conn_established) {
            return;
          }
          setCollasped(!collasped);
        }}
        style={{ alignItems: "center", cursor: makeMenu ? "context-menu" : undefined }}
        direction="row"
        onContextMenu={
          makeMenu
            ? (e) => {
                e.preventDefault();
                setUpMenu({ content: makeMenu(), event: e });
              }
            : undefined
        }>
        <IconButton disabled={!conn_established}>
          {collasped ? <KeyboardArrowRightIcon /> : <KeyboardArrowDownIcon />}
        </IconButton>

        {label}
      </Stack>
      <TreeIdent value={identNes}>
        <div
          style={{
            display: collasped ? "none" : "block",
            opacity: conn_established ? undefined : "0.7",
            paddingLeft: identNes + "px",
          }}>
          {children}
        </div>
      </TreeIdent>
    </Stack>
  );
}

export {
  MyTreeItem,
  VirtualListLeft,
  LongNotice,
  SelectPath,
  LogLevelSelection,
  MetaValueView,
  MenuNewQuery,
  MenuNewQueryNewWindow,
  MenuCreateDatabase,
  MenuRefresh,
  ConnPingTime,
  MenuCloseConn,
  MenuCreateSchema,
  MenuOpenConn,
  SelectPageSize,
  MenuDeleteConfig,
  ResetColumnWidth,
  CheckedView,
  ModalHeader,
  ViewRowInModal,
  UdfNameAndParameters,
  AppColumnMenu,
  SelectColumns,
  OpenFileWithDefaultApp,
  ResizeParentHeight,
  HighlightText,
};
