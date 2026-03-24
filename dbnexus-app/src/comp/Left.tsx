import {
  Autocomplete,
  Box,
  Button,
  Chip,
  CircularProgress,
  Divider,
  IconButton,
  InputAdornment,
  ListItemIcon,
  ListItemText,
  MenuItem,
  MenuList,
  Modal,
  Slider,
  Stack,
  TextField,
  Tooltip,
  Typography,
  useTheme,
} from "@mui/material";
import { IoMdSettings } from "react-icons/io";
import ClearIcon from "@mui/icons-material/Clear";
import { IoIosCreate } from "react-icons/io";
import {
  ReactNode,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import ContentCopyIcon from "@mui/icons-material/ContentCopy";
import { GrConnect } from "react-icons/gr";
import {
  OpenedContext,
  modalStyle,
  dropDatabase,
  dropSchema,
  EscDownEvent,
  modalHandleClose,
  ConnectionHandler,
  connectionChangeEvent,
  PassWordManager,
  tabsHasId,
} from "./Common";
import { LuDatabase } from "react-icons/lu";
import { FaDatabase } from "react-icons/fa";
import { FaCopy } from "react-icons/fa";
import types from "./ApiStruct";
import { invoke } from "@tauri-apps/api/core";
import { NotificationContext } from "./Notification";
import { SimpleTreeView } from "@mui/x-tree-view/SimpleTreeView";
import { TreeItem } from "@mui/x-tree-view/TreeItem";
import DeleteForeverIcon from "@mui/icons-material/DeleteForever";
import CloseIcon from "@mui/icons-material/Close";
import logoImg from "../../icon.png";
import { TabContent } from "./Tabs";
import wcmatch from "wildcard-match";

import {
  ColumnWidth,
  ConnTypeContext,
  connTypeToDataSourceKind,
  ContextMenuContext,
  dataGridStyle,
  DataSourceKind,
  extractConfigNameFromUuid,
  extractFieldAndIDFromMouseEvent,
  FavoriteSql,
  generateRandomString,
  handleCopyToClipboard,
  SaveBoolToLocalStorage,
  SaveMatchPattern,
  secondConfirm,
  SettingContext,
  TableType,
  TabsSetContext,
  UuidContext,
} from "./helper";
import { TbDatabaseSearch } from "react-icons/tb";
import { SqlQuery } from "./SqlQuery";
import { RedisQuery } from "./RedisQuery";
import { MongoQuery } from "./MongoQuery.tsx";

import {
  ConnPingTime,
  MenuCloseConn,
  MenuCreateDatabase,
  MenuCreateSchema,
  MenuDeleteConfig,
  MenuNewQuery,
  MenuOpenConn,
  MenuRefresh,
  ModalHeader,
  TrueOrFalse,
} from "./CommonComp.tsx";
import KeyboardArrowDownIcon from "@mui/icons-material/KeyboardArrowDown";
import KeyboardArrowRightIcon from "@mui/icons-material/KeyboardArrowRight";
import { RedisTables, TablesTree } from "./LeftTable.tsx";
import { ViewKind, ViewsTree } from "./LeftView.tsx";
import { CassandraAggregatesTree, UdfsTree } from "./LeftUdf.tsx";
import { ProceduresTree } from "./LeftProcedure.tsx";
import {
  DataGrid,
  GridColDef,
  GridSlotsComponent,
  GridSlotsComponentsProps,
  GridSortModel,
} from "@mui/x-data-grid";
import { listen } from "@tauri-apps/api/event";
import { FaBookmark } from "react-icons/fa";
import { SequenceTree } from "./LeftSequence.tsx";
import { openUrl } from "@tauri-apps/plugin-opener";
import { TypesTree } from "./LeftType.tsx";
import {
  AppDataGridPagination,
  CreateDatabase,
  CreateSchema,
  MenuConnectionInfo,
  StackRowCenter,
} from "./CommonComp2.tsx";
import { EventsTree } from "./LeftEvent.tsx";
import { AllKindChat } from "./Chat.tsx";
import { editor } from "monaco-editor";
import { listSchema } from "./handlers.ts";
import { CreateTable } from "./CreateTable.tsx";
import { AddConnection } from "./AddConnection.tsx";
import { ModifySetting } from "./Settings.tsx";
import { CouchBaseQuery } from "./CouchBase.tsx";
import FilterAltIcon from "@mui/icons-material/FilterAlt";
import FilterAltOutlinedIcon from "@mui/icons-material/FilterAltOutlined";
import { useAppConfigStore } from "./store.ts";
const settingId = generateRandomString();

function LeftTop(props: {
  matchPattern: string[];
  setMatchPattern;
  setMaxTreeSize;
  maxTreeSize;
}) {
  const { matchPattern, setMatchPattern, setMaxTreeSize, maxTreeSize } = props;
  const [matchPatterInput, setMatchPatternInput] = useState("");
  const { showNotification } = useContext(NotificationContext);
  const saveLeftTopShow = useRef(new SaveBoolToLocalStorage("left-top-show"));
  const [show, setShow] = useState(saveLeftTopShow.current.get());
  const { appendTab, setCurrentTab, tabs } = useContext(TabsSetContext);
  useEffect(() => {
    saveLeftTopShow.current.save(show);
  }, [show]);
  const [showEditFilter, setShowEditFilter] = useState(false);
  const showAddConnection = useCallback(
    async (conn?: types.DBConnectionConfig) => {
      appendTab(
        new TabContent(
          generateRandomString(),
          "add connection",
          <AddConnection config={conn} />,
          <GrConnect className="tab-icon-size" />,
        ),
      );
    },
    [appendTab],
  );

  useEffect(() => {
    const unlisten = listen("add_config", () => {
      showAddConnection();
    });
    return () => {
      unlisten.then((callback) => {
        callback();
      });
    };
  }, [showAddConnection]);

  useEffect(() => {
    const unlisten = listen("show_setting", () => {
      const index = tabsHasId(tabs, settingId);
      if (typeof index == "number") {
        setCurrentTab(index);
      } else {
        appendTab(
          new TabContent(
            settingId,
            "setting",
            <ModifySetting />,
            <IoMdSettings className="tab-icon-size" />,
          ),
        );
      }
    });
    return () => {
      unlisten.then((callback) => {
        callback();
      });
    };
  }, [appendTab, setCurrentTab, tabs]);

  const toggleShow = () => {
    setShow((e) => !e);
  };
  const theme = useTheme();
  return (
    <Stack style={{ padding: "2px" }}>
      <Stack>
        {show ? (
          <IconButton onClick={toggleShow}>
            <KeyboardArrowDownIcon />
          </IconButton>
        ) : (
          <IconButton onClick={toggleShow}>
            <KeyboardArrowRightIcon />
          </IconButton>
        )}
      </Stack>
      <Stack
        direction="row"
        style={{
          justifyContent: "center",
          // flexWrap: "wrap",
          display: show ? "flex" : "none",
        }}
      >
        <Tooltip title="DBPal site">
          <img
            src={logoImg}
            style={{ width: "3.3rem", cursor: "pointer" }}
            onClick={() => {
              openUrl("https://dbpal.site/");
            }}
            alt="logo"
          />
        </Tooltip>

        <Tooltip title="make a connection">
          <IconButton
            onClick={() => {
              showAddConnection();
            }}
          >
            <GrConnect style={{ fontSize: "2.1rem" }}> </GrConnect>
          </IconButton>
        </Tooltip>

        <Tooltip title="Favorite sql statement">
          <IconButton
            onClick={() => {
              appendTab(
                new TabContent(
                  generateRandomString(),
                  "favorite sql",
                  <FavoriteSqlList />,
                  <FaBookmark />,
                ),
              );
            }}
          >
            <FaBookmark style={{ fontSize: "2.1rem" }}> </FaBookmark>
          </IconButton>
        </Tooltip>

        <Tooltip title="copy a connection">
          <IconButton
            onClick={() => {
              appendTab(
                new TabContent(
                  generateRandomString(),
                  "copy connection",
                  <CopyConnection />,
                  <FaCopy />,
                ),
              );
            }}
          >
            <FaCopy style={{ fontSize: "2.2rem" }}> </FaCopy>
          </IconButton>
        </Tooltip>

        <Tooltip
          title={
            matchPattern.length > 0
              ? `filter on ${matchPattern.join(",")}`
              : "show editor filter,can match table,view etc..."
          }
        >
          <IconButton
            onClick={() => {
              setShowEditFilter(true);
            }}
          >
            {matchPattern.length > 0 ? (
              <FilterAltIcon style={{ fontSize: "2.7rem" }} color="success" />
            ) : (
              <FilterAltOutlinedIcon style={{ fontSize: "2.7rem" }} />
            )}
          </IconButton>
        </Tooltip>
      </Stack>
      {show && <Divider color="info" />}
      {showEditFilter && (
        <Modal
          open={showEditFilter}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setShowEditFilter(false);
          })}
        >
          <Box sx={modalStyle} width="45%">
            <ModalHeader
              title="edit filter"
              handleClose={() => {
                setShowEditFilter(false);
              }}
            />
            <Stack
              style={{
                display: show ? "flex" : "none",
                border: "2px solid " + theme.palette.success.main,
                borderRadius: "0.5rem",
                padding: "2px 0.5rem",
              }}
            >
              <Typography color="info">
                match pattern for database schema,table,view,udf,procedure etc
              </Typography>
              <Stack direction="row" style={{ flexWrap: "wrap", alignItems: "center" }}>
                {matchPattern.length > 1 && (
                  <IconButton
                    onClick={() => {
                      setMatchPattern([]);
                      setShowEditFilter(false);
                    }}
                  >
                    <CloseIcon />
                  </IconButton>
                )}
                {matchPattern.map((e, index) => (
                  <Chip
                    key={index}
                    label={e}
                    color="success"
                    onDelete={() => {
                      setMatchPatternInput(matchPattern[index]);
                      setMatchPattern((arr) =>
                        arr.filter((_, eIndex) => eIndex !== index),
                      );
                    }}
                  />
                ))}
              </Stack>
              <Stack direction="row" style={{ alignItems: "flex-end" }} spacing={1}>
                <TextField
                  fullWidth
                  value={matchPatterInput}
                  label="match pattern"
                  onChange={(e) => {
                    setMatchPatternInput(e.target.value.trim());
                  }}
                  slotProps={{
                    input: {
                      endAdornment: matchPatterInput && (
                        <InputAdornment position="end">
                          <IconButton
                            onClick={() => {
                              setMatchPatternInput("");
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
                <Button
                  onClick={() => {
                    if (matchPattern.find((e) => e == matchPatterInput)) {
                      showNotification("warning", "the value already exists");
                      return;
                    }
                    if (!matchPatterInput) {
                      showNotification("warning", "please input match pattern");
                      return;
                    }
                    setMatchPattern((e: string[]) => [...e, matchPatterInput]);
                  }}
                  variant="outlined"
                >
                  apply
                </Button>
              </Stack>
            </Stack>

            <StackRowCenter>
              <span color="info">max tree size({maxTreeSize}):</span>
              <Slider
                min={(function () {
                  if (import.meta.env.DEV) {
                    return 1;
                  } else {
                    return 100;
                  }
                })()}
                max={1000}
                onChange={(_, value) => {
                  setMaxTreeSize(value);
                }}
                style={{
                  flex: 1,
                  width: "40%",
                }}
              />
            </StackRowCenter>
          </Box>
        </Modal>
      )}
    </Stack>
  );
}

function CopyConnectionMenu(props: { content: string }) {
  const { showNotification } = useContext(NotificationContext);
  const { content } = props;
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
    </MenuList>
  );
}

function CopyConnection() {
  const [conns, setConns] = useState<types.DBConnectionConfig[]>([]);
  const [sortModel, setSortModel] = useState<GridSortModel>([]);
  const getRowId = useCallback((e) => {
    return e.name;
  }, []);
  const onSortModelChange = useCallback((e) => {
    setSortModel(e);
  }, []);

  const { showNotification } = useContext(NotificationContext);
  const setting = useContext(SettingContext);
  const { setUpMenu } = useContext(ContextMenuContext);
  const { appendTab } = useContext(TabsSetContext);

  const handleRow = useCallback(
    async (conn) => {
      const password = await PassWordManager.getPassword(conn.name);
      const cop = { ...conn };
      if (password) {
        cop["password"] = password;
      }
      appendTab(
        new TabContent(
          generateRandomString(),
          "add connection",
          <AddConnection config={cop} />,
          <GrConnect />,
        ),
      );
    },
    [appendTab],
  );

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
            content: <CopyConnectionMenu content={content} />,
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
  }, [setUpMenu, setting]);

  const cols = useMemo(() => {
    const cols = [
      { field: "name" },
      { field: "username" },

      {
        field: "conn_type",
        renderHeader: () => {
          return "kind";
        },
      },
      { field: "host" },
      { field: "port" },
      { field: "path" },
      {
        field: "db",
        renderHeader: () => {
          return "database";
        },
      },
      {
        field: "read_only",
        renderCell: (row) => {
          return <TrueOrFalse value={row.row["read_only"]} />;
        },
        renderHeader: () => {
          return "read only";
        },
      },
    ] as GridColDef[];
    cols.forEach((e) => {
      const width = ColumnWidth.instance.get(e.field);
      if (width) {
        e.width = width;
      }
    });
    return cols;
  }, []);

  const onColumnResize = useCallback(({ colDef, width }) => {
    ColumnWidth.instance.save(colDef.field, width);
  }, []);

  const onRowDoubleClick = useCallback(
    (row) => {
      handleRow(row.row);
    },
    [handleRow],
  );

  const handleReload = useCallback(async () => {
    try {
      const conns = await ConnectionHandler.getAll();
      setConns(conns);
    } catch (e) {
      showNotification("error", "get connections failed", e);
    }
  }, [showNotification]);
  useEffect(() => {
    handleReload();
  }, [handleReload]);
  const slots = useMemo(() => {
    return {
      pagination: () => {
        return (
          <Stack direction="row" flex={1} style={{ justifyContent: "space-between" }}>
            <Stack direction="row">
              <Tooltip title="double click a row to copy a connection.">
                <Typography color="info">copy?</Typography>
              </Tooltip>
            </Stack>
            <AppDataGridPagination />
          </Stack>
        );
      },
    } as Partial<GridSlotsComponent>;
  }, []);
  return (
    <Stack style={{ height: "100%" }}>
      <Stack direction="row">
        {conns.length == 0 && (
          <Button
            onClick={() => {
              handleReload();
            }}
          >
            reload
          </Button>
        )}
      </Stack>
      <DataGrid
        getRowId={getRowId}
        style={dataGridStyle}
        slots={slots}
        sortModel={sortModel}
        onSortModelChange={onSortModelChange}
        slotProps={slotProps}
        columns={cols}
        rows={conns}
        onColumnResize={onColumnResize}
        onRowDoubleClick={onRowDoubleClick}
      />
    </Stack>
  );
}

function Left() {
  const [configs, setConfigs] = useState<types.DBConnectionConfig[]>([]);
  const { showNotification } = useContext(NotificationContext);
  const [matchPattern, setMatchPattern] = useState<string[]>(
    SaveMatchPattern.instance.get(),
  );
  const maxTreeSizeKey = "max-tree-item-size";
  const [maxTreeSize, setMaxTreeSize] = useState(
    parseInt(localStorage.getItem(maxTreeSizeKey)) || 100,
  );
  const setMatchStateFn = useAppConfigStore((state) => state.setMatchPattern);
  const setMaxTreeSizeStateFn = useAppConfigStore((state) => state.setMaxTree);

  useEffect(() => {
    SaveMatchPattern.instance.save(matchPattern);
  }, [matchPattern]);
  useEffect(() => {
    localStorage.setItem(maxTreeSizeKey, maxTreeSize + "");
    setMaxTreeSizeStateFn(maxTreeSize);
  }, [maxTreeSize, setMaxTreeSizeStateFn]);

  const handleReload = useCallback(async () => {
    ConnectionHandler.getAll()
      .then((e) => {
        if (!e) {
          e = [];
        }
        setConfigs(e);
      })
      .catch((e) => {
        showNotification("error", "list all config failure", e);
      });
  }, [showNotification]);
  useEffect(() => {
    handleReload();
  }, [handleReload]);

  useEffect(() => {
    const unlisten = listen(connectionChangeEvent, () => {
      handleReload();
    });
    return () => {
      unlisten.then((e) => {
        e();
      });
    };
  }, [handleReload]);

  useEffect(() => {
    const sfn = (x: string) => {
      if (matchPattern.length == 0) {
        return true;
      }
      for (const mp of matchPattern) {
        const matchFn = wcmatch("*" + mp + "*", { flags: "i" });
        if (matchFn(x)) {
          return true;
        }
      }
      return false;
    };
    setMatchStateFn(sfn);
  }, [matchPattern, setMatchStateFn]);

  return (
    <Stack
      style={{
        overflow: "auto",
      }}
    >
      <LeftTop
        matchPattern={matchPattern}
        setMatchPattern={setMatchPattern}
        maxTreeSize={maxTreeSize}
        setMaxTreeSize={setMaxTreeSize}
      />
      <Stack spacing={0.3}>
        {configs.map((e) => {
          return (
            <ConnTypeContext.Provider
              key={e.name}
              value={{ kind: connTypeToDataSourceKind(e.conn_type) }}
            >
              <UuidContext.Provider
                value={{
                  uuid: e.name,
                }}
              >
                <OneConfig key={e.name} config={e} />
              </UuidContext.Provider>
            </ConnTypeContext.Provider>
          );
        })}
        {configs.length == 0 && (
          <Typography color="info"> trying to add connection </Typography>
        )}
      </Stack>
    </Stack>
  );
}

function FavoriteSqlMenu(props: {
  content: string;
  id: number;
  handleReload: () => void;
}) {
  const { content, id, handleReload } = props;
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

      <MenuItem
        onClick={async () => {
          await FavoriteSql.delete(id, showNotification);
          handleReload();
        }}
      >
        <ListItemIcon>
          <DeleteForeverIcon />
        </ListItemIcon>
        <ListItemText> delete </ListItemText>
      </MenuItem>
    </MenuList>
  );
}

function FavoriteSqlList() {
  const { showNotification } = useContext(NotificationContext);
  const [fs, setFs] = useState<FavoriteSql[]>([]);
  const [sortModel, setSortModel] = useState<GridSortModel>([
    { field: "lastModified", sort: "desc" },
  ]);
  const [uuidFilter, setUuidFilter] = useState("");
  const [distinctUuid, setDistinctUuid] = useState<string[]>([]);
  const setting = useContext(SettingContext);
  const handleReload = useCallback(
    async (showOK) => {
      try {
        const us = await FavoriteSql.getDistinctUuid();
        setDistinctUuid(us);
        const fs = await FavoriteSql.getByUuid(uuidFilter);
        setFs(fs);
      } catch (e) {
        showNotification("error", "list all favorite sql failed", e);
        return;
      }
      if (showOK) {
        showNotification("success", "fetch favorite sql ok");
      }
    },
    [showNotification, uuidFilter],
  );

  const { appendTab } = useContext(TabsSetContext);

  const checkConnOpened = useCallback(
    async (uuid: string) => {
      const connOpened = await invoke<boolean>("conn_opened", { name: uuid });
      if (!connOpened) {
        showNotification(
          "error",
          `connection '${extractConfigNameFromUuid(uuid)}' not opened????`,
        );
        return false;
      } else {
        return true;
      }
    },
    [showNotification],
  );

  const handleRow = useCallback(
    async (row: FavoriteSql) => {
      try {
        if (!(await checkConnOpened(row.uuid))) {
          return;
        }
        const content = (
          <AllKindChat
            kind={row.kind}
            uuid={row.uuid}
            value={row.sql}
            favoriteName={row.name}
            language={row.language}
          />
        );
        appendTab(
          new TabContent(
            generateRandomString(10),
            `${row.uuid}`,
            content,
            <TbDatabaseSearch className="tab-icon-size" />,
          ),
        );
      } catch (e) {
        showNotification("error", "use sql failure", e);
      }
    },
    [appendTab, checkConnOpened, showNotification],
  );
  const onSortModelChange = useCallback((e) => {
    setSortModel(e);
  }, []);

  useEffect(() => {
    handleReload(false);
  }, [handleReload, uuidFilter]);

  const { setUpMenu } = useContext(ContextMenuContext);
  const slotProps = useMemo(() => {
    return {
      row: {
        onContextMenu: (e) => {
          const { target } = e;
          const content = (target as HTMLDivElement).innerHTML;
          const { field, id } = extractFieldAndIDFromMouseEvent(e);
          if (!field) {
            return;
          }
          setUpMenu({
            content: (
              <FavoriteSqlMenu
                content={content}
                id={parseInt(id)}
                handleReload={() => {
                  handleReload(false);
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
  }, [handleReload, setUpMenu, setting]);

  const columns = useMemo(() => {
    const cols = [
      {
        field: "uuid",
        renderHeader: () => {
          return "connection name";
        },
      },
      { field: "name" },
      { field: "kind" },
      {
        field: "sql",
        renderHeader: () => {
          return "statement";
        },
      },
      { field: "lastModified" },
    ] as GridColDef[];
    cols.forEach((e) => {
      const width = ColumnWidth.instance.get(e.field);
      if (width) {
        e.width = width;
      }
    });
    return cols;
  }, []);
  const onColumnResize = useCallback(({ colDef, width }) => {
    ColumnWidth.instance.save(colDef.field, width);
  }, []);

  const onRowDoubleClick = useCallback(
    (row) => {
      handleRow(row.row);
    },
    [handleRow],
  );

  const handleDelete = async () => {
    const confirm = await secondConfirm({
      title: "clean all favorite sql???",
      confirmText: "Clean",
      kind: "error",
    });
    if (!confirm) {
      return;
    }
    FavoriteSql.clearByUuid(showNotification, uuidFilter);
  };
  const slots = useMemo(() => {
    return {
      pagination: () => {
        return (
          <Stack direction="row" style={{ justifyContent: "space-between" }} flex={1}>
            <Stack direction="row">
              {fs.length > 0 && (
                <Tooltip title="double click a row to use sql in new tab.">
                  <Typography color="info">use?</Typography>
                </Tooltip>
              )}
            </Stack>
            <AppDataGridPagination />
          </Stack>
        );
      },
    } as Partial<GridSlotsComponent>;
  }, [fs.length]);

  return (
    <Stack style={{ height: "100%" }}>
      <StackRowCenter>
        <Autocomplete
          options={distinctUuid}
          style={{ width: "22rem" }}
          value={uuidFilter}
          renderInput={(params) => (
            <TextField {...params} label="filter connection name" />
          )}
          freeSolo={false}
          onChange={(_, uuid) => {
            setUuidFilter(uuid);
          }}
        />
        <Button
          onClick={() => {
            handleReload(true);
          }}
        >
          reload
        </Button>
        {fs.length > 0 && uuidFilter && (
          <Tooltip title={`delete all sql by uuid '${uuidFilter}'`}>
            <IconButton
              onClick={() => {
                handleDelete();
              }}
            >
              <DeleteForeverIcon color="error" />
            </IconButton>
          </Tooltip>
        )}
      </StackRowCenter>
      <DataGrid
        sortModel={sortModel}
        slots={slots}
        onSortModelChange={onSortModelChange}
        slotProps={slotProps}
        columns={columns}
        rows={fs}
        style={dataGridStyle}
        onColumnResize={onColumnResize}
        onRowDoubleClick={onRowDoubleClick}
      />
    </Stack>
  );
}

function OneConfig(props: { config: types.DBConnectionConfig }) {
  const { config } = props;
  const [opened, setOpened] = useState(false);

  const makeTree = () => {
    switch (config.conn_type) {
      case DataSourceKind.Postgres:
      case DataSourceKind.Cockroach:
        if (!config.db) {
          return <CatalogTree />;
        }
        return config.schema ? <SchemaTree /> : <DatabaseTree />;
      case DataSourceKind.MariaDB:
      case DataSourceKind.Mysql:
      case DataSourceKind.TiDB:
        return config.db ? <SchemaTree /> : <DatabaseTree />;
      case DataSourceKind.Sqlite:
      case DataSourceKind.LibSql:
        return <SchemaTree />;
      case DataSourceKind.Redis:
        return config.db ? <SchemaTree /> : <DatabaseTree />;
      case DataSourceKind.Mongo:
        return config.db ? <SchemaTree /> : <DatabaseTree />;
      case DataSourceKind.SqlServer:
        if (!config.db) {
          return <CatalogTree />;
        }
        return config.schema ? <SchemaTree /> : <DatabaseTree />;
      case DataSourceKind.Oracle:
        return config.schema ? <SchemaTree /> : <DatabaseTree />;
      case DataSourceKind.Cassandra:
        return config.db ? <SchemaTree /> : <DatabaseTree />;
      case DataSourceKind.ClickHouse:
        return config.db ? <SchemaTree /> : <DatabaseTree />;
      case DataSourceKind.FireBird:
        return <SchemaTree />;
      case DataSourceKind.DuckDB:
        if (!config.db) {
          return <CatalogTree />;
        }
        return config.schema ? <SchemaTree /> : <DatabaseTree />;
      case DataSourceKind.Db2:
        return config.schema ? <SchemaTree /> : <DatabaseTree />;
      case DataSourceKind.CouchBase:
        if (!config.bucket) {
          return <CatalogTree />;
        }
        return config.scope ? <SchemaTree /> : <DatabaseTree />;

      default:
        console.error("impl database kind:", config);
    }
  };
  return (
    <OpenedContext.Provider value={{ opened, setOpened }}>
      <SimpleTreeView disableSelection>{makeTree()}</SimpleTreeView>
    </OpenedContext.Provider>
  );
}

function SchemaTree(props: { schema?: string; reloadSchemas? }) {
  const { kind } = useContext(ConnTypeContext);
  const { uuid } = useContext(UuidContext);
  const { schema, reloadSchemas } = props;

  const { opened, setOpened } = useContext(OpenedContext);
  const SUPPORTED_PROCEDURE_KINDS = new Set([
    DataSourceKind.MariaDB,
    DataSourceKind.Mysql,
    DataSourceKind.Postgres,
    DataSourceKind.SqlServer,
    DataSourceKind.Oracle,
    DataSourceKind.Cockroach,
  ]);

  const supportProcedure = SUPPORTED_PROCEDURE_KINDS.has(kind);
  const supportUdf = new Set([
    DataSourceKind.MariaDB,
    DataSourceKind.Mysql,
    DataSourceKind.Postgres,
    DataSourceKind.SqlServer,
    DataSourceKind.Oracle,
    DataSourceKind.Cassandra,
    DataSourceKind.Cockroach,
    DataSourceKind.ClickHouse,
  ]).has(kind);
  const supportSequence = [
    DataSourceKind.MariaDB,
    DataSourceKind.Postgres,
    DataSourceKind.Cockroach,
    DataSourceKind.DuckDB,
  ].includes(kind);

  const supportType = [
    DataSourceKind.Postgres,
    DataSourceKind.Cockroach,
    DataSourceKind.Cassandra,
  ].includes(kind);
  const supportEvent = [DataSourceKind.Mysql, DataSourceKind.MariaDB].includes(kind);
  const { setUpMenu } = useContext(ContextMenuContext);
  const [loading, setLoading] = useState(false);
  const idForTree = useRef(generateRandomString());
  return (
    <TreeItem
      itemId={idForTree.current}
      label=<Stack
        direction="row"
        onContextMenu={(e) => {
          setUpMenu({
            content: (
              <SchemaMenu
                opened={opened}
                schema={schema}
                setLoading={setLoading}
                handleOpened={() => {
                  setOpened(true);
                }}
                handleCloseConn={() => {
                  setOpened(false);
                }}
                uuid={uuid}
                reloadSchemas={reloadSchemas}
                kind={kind}
              />
            ),

            event: e,
          });
        }}
        sx={{
          alignItems: "center",
        }}
      >
        {schema && <LuDatabase />}
        <span> {schema ? schema : uuid}</span>
        {!schema && opened && <ConnPingTime uuid={uuid} />}
        {loading && <CircularProgress />}
      </Stack>
    >
      {opened && kind == DataSourceKind.Sqlite && (
        <>
          <TablesTree typ={TableType.Normal} />
          <ViewsTree viewKind={ViewKind.View} />
        </>
      )}
      {opened && kind == DataSourceKind.CouchBase && (
        <>
          <TablesTree typ={TableType.Normal} />
        </>
      )}

      {opened && kind == DataSourceKind.DuckDB && (
        <>
          <TablesTree typ={TableType.Normal} />
          <ViewsTree viewKind={ViewKind.View} />
        </>
      )}
      {opened && kind == DataSourceKind.FireBird && (
        <>
          <TablesTree typ={TableType.Normal} />
          <ViewsTree viewKind={ViewKind.View} />
        </>
      )}
      {opened && kind == DataSourceKind.LibSql && (
        <>
          <TablesTree typ={TableType.Normal} />
          <ViewsTree viewKind={ViewKind.View} />
        </>
      )}
      {opened && kind == DataSourceKind.Cassandra && (
        <>
          <TablesTree typ={TableType.Normal} />
          <ViewsTree viewKind={ViewKind.MaterializedView} />
        </>
      )}
      {opened && kind == DataSourceKind.Mongo && (
        <>
          <TablesTree typ={TableType.Normal} />
          <TablesTree typ={TableType.MongoTimeSerial} />
          <ViewsTree viewKind={ViewKind.View} />
        </>
      )}
      {opened &&
        (kind == DataSourceKind.Mysql ||
          kind == DataSourceKind.MariaDB ||
          kind == DataSourceKind.TiDB) && (
          <>
            <TablesTree typ={TableType.Normal} />
            <ViewsTree viewKind={ViewKind.View} />
          </>
        )}
      {opened && kind == DataSourceKind.ClickHouse && (
        <>
          <TablesTree typ={TableType.Normal} />
          <ViewsTree viewKind={ViewKind.View} />
          <ViewsTree viewKind={ViewKind.MaterializedView} />
        </>
      )}
      {opened && kind == DataSourceKind.Postgres && (
        <>
          <TablesTree typ={TableType.Normal} />
          <ViewsTree viewKind={ViewKind.View} />
          <ViewsTree viewKind={ViewKind.MaterializedView} />
        </>
      )}
      {opened && kind == DataSourceKind.Cockroach && (
        <>
          <TablesTree typ={TableType.Normal} />
          <ViewsTree viewKind={ViewKind.View} />
          <ViewsTree viewKind={ViewKind.MaterializedView} />
        </>
      )}
      {opened && kind == DataSourceKind.Oracle && (
        <>
          <TablesTree typ={TableType.Normal} />
          <ViewsTree viewKind={ViewKind.View} />
          <ViewsTree viewKind={ViewKind.MaterializedView} />
        </>
      )}
      {opened && kind == DataSourceKind.SqlServer && (
        <>
          <TablesTree typ={TableType.Normal} />
          <ViewsTree viewKind={ViewKind.View} />
        </>
      )}
      {opened && (
        <>
          {supportProcedure && <ProceduresTree />}
          {supportUdf && <UdfsTree />}
        </>
      )}
      {opened && <>{supportSequence && <SequenceTree />}</>}

      {opened && kind == DataSourceKind.Cassandra && <CassandraAggregatesTree />}
      {opened && kind == DataSourceKind.Redis && <RedisTables />}
      {opened && supportType && <TypesTree />}
      {opened && supportEvent && <EventsTree />}
    </TreeItem>
  );
}

function DatabaseMenu(props: {
  database?: string;
  uuid: string;
  reload;
  handleOpened;
  opened;
  handleCloseConn;
  reloadDatabase;
  kind: DataSourceKind;
  schemas: string[];
  setLoading;
}) {
  const {
    database,
    uuid,
    reload,
    handleOpened,
    opened,
    handleCloseConn,
    reloadDatabase,
    kind,
    schemas,
    setLoading,
  } = props;
  const { showNotification } = useContext(NotificationContext);
  const [showCreateSchema, setShowCreateSchema] = useState(false);
  const { appendTab } = useContext(TabsSetContext);
  return (
    <div>
      <MenuList>
        {opened &&
          kind != DataSourceKind.Mongo &&
          kind != DataSourceKind.Redis &&
          kind != DataSourceKind.CouchBase && (
            <MenuNewQuery
              handleNew={() => {
                openNewQuery(appendTab, uuid, kind);
              }}
            />
          )}

        {opened && kind != DataSourceKind.Oracle && (
          <MenuCreateSchema
            handleCreate={() => {
              setShowCreateSchema(true);
            }}
            kind={kind}
          />
        )}

        {!opened && !database && (
          <MenuOpenConn
            name={uuid}
            setLoading={setLoading}
            handleOpened={handleOpened}
          />
        )}

        {opened && <MenuRefresh reload={reload} />}
        {opened && !database && <MenuConnectionInfo uuid={uuid} />}

        {opened && !database && (
          <MenuCloseConn name={uuid} handleClose={handleCloseConn} />
        )}
        {database && kind != DataSourceKind.DuckDB && (
          <MenuItem
            onClick={() => {
              // we are in database context uuid contains the database name
              // we need remove that
              const index = uuid.indexOf("/");
              dropDatabase({
                uuid: uuid.substring(0, index),
                database,
                showNotification,
                okCallback: () => {
                  reloadDatabase(true);
                },
              });
            }}
          >
            <ListItemIcon>
              <DeleteForeverIcon color="error" />
            </ListItemIcon>
            <ListItemText color="error">drop</ListItemText>
          </MenuItem>
        )}

        {!database && <MenuDeleteConfig name={uuid} />}
      </MenuList>

      {showCreateSchema && (
        <Modal
          open={showCreateSchema}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setShowCreateSchema(false);
          })}
        >
          <Box sx={modalStyle} width="40%">
            <ModalHeader
              title={(function () {
                let name = "database";
                switch (kind) {
                  case DataSourceKind.Postgres:
                  case DataSourceKind.Cockroach:
                  case DataSourceKind.SqlServer:
                    name = "schema";
                    break;
                }
                return `create ${name}`;
              })()}
              handleClose={() => {
                setShowCreateSchema(false);
              }}
            />
            <CreateSchema
              uuid={uuid}
              handleClose={() => {
                reload();
                setShowCreateSchema(false);
              }}
              kind={kind}
              schemas={schemas}
              handleReload={reload}
            />
          </Box>
        </Modal>
      )}
    </div>
  );
}

function CatalogMenu(props: {
  uuid;
  reload;
  handleOpened;
  opened;
  handleCloseConn;
  setLoading;
  kind: DataSourceKind;
}) {
  const { uuid, reload, handleOpened, opened, handleCloseConn, kind, setLoading } =
    props;
  const [showCreateDB, setShowCreateDB] = useState(false);
  return (
    <div>
      <MenuList>
        {!opened && (
          <MenuOpenConn
            name={uuid}
            setLoading={setLoading}
            handleOpened={handleOpened}
          />
        )}

        {opened && kind != DataSourceKind.DuckDB && (
          <MenuCreateDatabase
            handleCreate={() => {
              setShowCreateDB(true);
            }}
            kind={kind}
          />
        )}

        {opened && <MenuRefresh reload={reload} />}
        {opened && <MenuConnectionInfo uuid={uuid} />}

        {opened && <MenuCloseConn name={uuid} handleClose={handleCloseConn} />}
        <Divider />
        <MenuDeleteConfig name={uuid} />
      </MenuList>
      {showCreateDB && (
        <Modal
          open={showCreateDB}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setShowCreateDB(false);
          })}
        >
          <Box sx={modalStyle} width="50%">
            <ModalHeader
              handleClose={() => {
                setShowCreateDB(false);
              }}
              title={(function () {
                switch (kind) {
                  case DataSourceKind.CouchBase:
                    return "create bucket";
                  default:
                    return "create database";
                }
              })()}
            />
            <CreateDatabase
              uuid={uuid}
              handleClose={() => {
                reload();
                setShowCreateDB(false);
              }}
              kind={kind}
            />
          </Box>
        </Modal>
      )}
    </div>
  );
}

function AllKindQuery(props: {
  kind: DataSourceKind;
  uuid: string;
  value?: string;
  favoriteName?: string;
  handleEditor: (x: editor.IStandaloneCodeEditor) => void;
  language?: string;
}) {
  const { kind, uuid, value, favoriteName, handleEditor, language } = props;
  let content: ReactNode;
  switch (kind) {
    case DataSourceKind.SqlServer:
    case DataSourceKind.Sqlite:
    case DataSourceKind.Mysql:
    case DataSourceKind.MariaDB:
    case DataSourceKind.Postgres:
    case DataSourceKind.Oracle:
    case DataSourceKind.TiDB:
    case DataSourceKind.Cassandra:
    case DataSourceKind.ClickHouse:
    case DataSourceKind.LibSql:
    case DataSourceKind.FireBird:
    case DataSourceKind.DuckDB:
    case DataSourceKind.Cockroach:
    case DataSourceKind.Db2:
      content = (
        <SqlQuery
          uuid={uuid}
          value={value}
          kind={kind}
          favoriteName={favoriteName}
          handleEditor={handleEditor}
        />
      );
      break;
    case DataSourceKind.Redis:
      content = (
        <RedisQuery
          handleEditor={handleEditor}
          uuid={uuid}
          value={value}
          favoriteName={favoriteName}
        />
      );
      break;

    case DataSourceKind.Mongo:
      content = (
        <MongoQuery
          language={language}
          handleEditor={handleEditor}
          uuid={uuid}
          value={value}
          favoriteName={favoriteName}
        />
      );
      break;

    case DataSourceKind.CouchBase:
      return <CouchBaseQuery uuid={uuid} value={value} handleEditor={handleEditor} />;
    default:
      console.error(`impl this ${kind}`);
  }
  return <div style={{ height: "100%" }}> {content}</div>;
}

function openNewQuery(appendTab, uuid: string, kind: DataSourceKind) {
  console.log("open new query uuid:", uuid);
  const id = uuid + "-query-" + generateRandomString();
  const content = <AllKindChat kind={kind} uuid={uuid} />;
  appendTab(
    new TabContent(
      id,
      `${uuid}`,
      content,
      <TbDatabaseSearch className="tab-icon-size" />,
    ),
  );
}

function openNewCreateTable(appendTab, uuid: string, kind: DataSourceKind) {
  const id = uuid + "-query-" + generateRandomString();
  const content = <CreateTable kind={kind} uuid={uuid} />;
  appendTab(new TabContent(id, `create table`, content, <IoIosCreate> </IoIosCreate>));
}

function SchemaMenu(props: {
  schema?: string;
  uuid: string;
  handleOpened;
  opened;
  handleCloseConn;
  kind: DataSourceKind;
  reloadSchemas?;
  setLoading;
}) {
  const {
    schema,
    uuid,
    handleOpened,
    opened,
    handleCloseConn,
    reloadSchemas,
    kind,
    setLoading,
  } = props;
  const { appendTab } = useContext(TabsSetContext);
  const { showNotification } = useContext(NotificationContext);
  return (
    <MenuList>
      {!schema && !opened && (
        <MenuOpenConn name={uuid} setLoading={setLoading} handleOpened={handleOpened} />
      )}

      {opened && (
        <MenuNewQuery
          handleNew={() => {
            openNewQuery(appendTab, uuid, kind);
          }}
        />
      )}

      {opened &&
        kind != DataSourceKind.Redis &&
        kind != DataSourceKind.Mongo &&
        kind != DataSourceKind.CouchBase && (
          <MenuItem
            onClick={() => {
              openNewCreateTable(appendTab, uuid, kind);
            }}
          >
            <ListItemIcon>
              <IoIosCreate />
            </ListItemIcon>
            <ListItemText>create table </ListItemText>
          </MenuItem>
        )}

      {opened && !schema && <MenuConnectionInfo uuid={uuid} />}

      {!schema && opened && <MenuCloseConn name={uuid} handleClose={handleCloseConn} />}
      {schema && kind != DataSourceKind.Oracle && (
        <MenuItem
          onClick={() => {
            // we are in database context uuid contains the database name
            // we need remove that
            const index = uuid.lastIndexOf("/");
            dropSchema({
              uuid: uuid.substring(0, index),
              schema,
              showNotification,
              okCallback: () => {
                reloadSchemas(true);
              },
            });
          }}
        >
          <ListItemIcon>
            <DeleteForeverIcon color="error" />
          </ListItemIcon>
          <ListItemText color="error">drop</ListItemText>
        </MenuItem>
      )}
      {!schema && <MenuDeleteConfig name={uuid} />}
    </MenuList>
  );
}

function CatalogTree() {
  const { uuid: name } = useContext(UuidContext);
  const { showNotification } = useContext(NotificationContext);
  const [databases, setDatabases] = useState<string[]>([]);
  const [loading, setLoading] = useState(false);
  const { kind } = useContext(ConnTypeContext);

  const handleReload = useCallback(() => {
    setLoading(true);
    invoke<string[]>("list_database", { name })
      .then((ds) => {
        setDatabases(ds);
      })
      .catch((e) => {
        showNotification("error", "list database failed", e);
      })
      .finally(() => {
        setLoading(false);
      });
  }, [name, showNotification]);
  const { opened, setOpened } = useContext(OpenedContext);
  useEffect(() => {
    if (opened) {
      handleReload();
    }
  }, [handleReload, opened]);

  const { setUpMenu } = useContext(ContextMenuContext);

  const matchPattern = useAppConfigStore((state) => state.matchPattern);

  return (
    <TreeItem
      itemId={name}
      label=<Stack
        direction="row"
        onContextMenu={(e) => {
          setUpMenu({
            content: (
              <CatalogMenu
                uuid={name}
                setLoading={setLoading}
                reload={handleReload}
                opened={opened}
                handleOpened={() => {
                  setOpened(true);
                }}
                handleCloseConn={() => {
                  setOpened(false);
                  setDatabases([]);
                }}
                kind={kind}
              />
            ),
            event: e,
          });
        }}
      >
        <span>{name} </span>
        {loading && <CircularProgress />}
        {opened && <ConnPingTime uuid={name} />}
      </Stack>
      key={name}
    >
      {databases
        .filter((db) => matchPattern(db))
        .map((db) => {
          return (
            <UuidContext.Provider key={db} value={{ uuid: name + "/" + db }}>
              <DatabaseTree key={db} database={db} reloadDatabase={handleReload} />
            </UuidContext.Provider>
          );
        })}
    </TreeItem>
  );
}

function DatabaseTree(props: { database?: string; reloadDatabase?: () => void }) {
  const { uuid: name } = useContext(UuidContext);
  const { showNotification } = useContext(NotificationContext);
  const [schemas, setSchemas] = useState<string[]>([]);
  const { database, reloadDatabase } = props;
  const [loading, setLoading] = useState(false);
  const { opened, setOpened } = useContext(OpenedContext);
  const { kind } = useContext(ConnTypeContext);
  const matchPattern = useAppConfigStore((state) => state.matchPattern);
  const handleReload = useCallback(() => {
    setLoading(true);
    listSchema(name)
      .then((ds) => {
        setSchemas(ds);
      })
      .catch((e) => {
        showNotification("error", "list schema failed", e);
      })
      .finally(() => {
        setLoading(false);
      });
  }, [name, showNotification]);

  useEffect(() => {
    if (opened) {
      handleReload();
    }
  }, [handleReload, opened]);
  const { setUpMenu } = useContext(ContextMenuContext);

  return (
    <TreeItem
      itemId={name}
      label=<Stack
        direction="row"
        alignContent="baseline"
        onContextMenu={(e) => {
          setUpMenu({
            content: (
              <DatabaseMenu
                setLoading={setLoading}
                opened={opened}
                handleOpened={() => {
                  setOpened(true);
                }}
                handleCloseConn={() => {
                  setOpened(false);
                  setSchemas([]);
                }}
                database={database}
                uuid={name}
                reload={handleReload}
                reloadDatabase={reloadDatabase}
                kind={kind}
                schemas={schemas}
              />
            ),
            event: e,
          });
        }}
        sx={{ alignItems: "center" }}
      >
        {database && <FaDatabase />}
        <span>{database ? database : name}</span>
        {loading && <CircularProgress />}
        {!database && opened && <ConnPingTime uuid={name} />}
      </Stack>
      key={name + database}
    >
      {schemas
        .filter((schema) => matchPattern(schema))
        .map((schema) => {
          return (
            <UuidContext.Provider key={schema} value={{ uuid: name + "/" + schema }}>
              <SchemaTree key={schema} schema={schema} reloadSchemas={handleReload} />
            </UuidContext.Provider>
          );
        })}
    </TreeItem>
  );
}

export { Left, AllKindQuery };
