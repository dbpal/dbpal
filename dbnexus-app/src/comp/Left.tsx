import {
  Autocomplete,
  Button,
  Chip,
  CircularProgress,
  Dialog,
  Divider,
  IconButton,
  ListItemIcon,
  ListItemText,
  MenuItem,
  MenuList,
  Slider,
  Stack,
  TextField,
  Tooltip,
  Typography,
  useTheme,
} from "@mui/material";
import { IoMdSettings } from "react-icons/io";
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
  dropDatabase,
  dropSchema,
  EscDownEvent,
  modalHandleClose,
  PassWordManager,
  QueryConn,
  uuidv4,
} from "./Common";
import { LuDatabase } from "react-icons/lu";
import { FaDatabase } from "react-icons/fa";
import { FaCopy } from "react-icons/fa";
import { SimpleTreeView } from "@mui/x-tree-view/SimpleTreeView";
import { TreeItem } from "@mui/x-tree-view/TreeItem";
import DeleteForeverIcon from "@mui/icons-material/DeleteForever";
import CloseIcon from "@mui/icons-material/Close";
import logoImg from "../../icon.png";

import {
  connTypeToDataSourceKind,
  dataGridStyle,
  DataSourceKind,
  extractConfigNameFromUuid,
  extractFieldAndIDFromMouseEvent,
  handleCopyToClipboard,
  secondConfirm,
  TabContent,
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
  MenuNewQueryNewWindow,
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

import { listen } from "@tauri-apps/api/event";
import { FaBookmark } from "react-icons/fa";
import { SequenceTree } from "./LeftSequence.tsx";
import { openUrl } from "@tauri-apps/plugin-opener";
import { TypesTree } from "./LeftType.tsx";
import {
  AppDataGridPagination,
  ClearableTextField,
  CreateDatabase,
  CreateSchema,
  MenuConnectionInfo,
  ShowingCount,
  StackRowCenter,
} from "./CommonComp2.tsx";
import { EventsTree } from "./LeftEvent.tsx";
import { AllKindChat } from "./Chat.tsx";
import { CreateTable } from "./CreateTable.tsx";
import { AddConnection } from "./AddConnection.tsx";
import { ModifySetting } from "./Settings.tsx";
import { CouchBaseQuery } from "./CouchBase.tsx";
import FilterAltIcon from "@mui/icons-material/FilterAlt";
import FilterAltOutlinedIcon from "@mui/icons-material/FilterAltOutlined";
import {
  useMatcher,
  useConnLoading,
  useConnected,
  useContextMenu,
  useNotification,
  useTabsManage,
  useBoolSwitch,
  useColumnWidthStore,
  useMatcherFn,
  useConnectionStore,
  useFavoriteStore,
  FavoriteSql,
  useOneShotDialogUI,
} from "./store.tsx";
import { commands, DBConnectionConfig } from "./bindings.ts";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import {
  DataGrid,
  GridColDef,
  GridSlotsComponent,
  GridSlotsComponentsProps,
  GridSortModel,
} from "./datagridExport.ts";
const settingId = uuidv4();
const showTopKey = "left-top-show";
function LeftTop() {
  const { matchPattern, setMatchPattern, setMaxTree, maxTree } = useMatcher();
  const [matchPatterInput, setMatchPatternInput] = useState("");
  const { showNotification } = useNotification();
  const { getSwitch, setSwitch } = useBoolSwitch();
  const show = getSwitch(showTopKey);
  const { appendTab, setCurrentTab, tabs } = useTabsManage();
  const [showEditFilter, setShowEditFilter] = useState(false);
  const showAddConnection = useCallback(
    async (conn?: DBConnectionConfig) => {
      appendTab(
        new TabContent(
          "add connection",
          <AddConnection config={conn} />,
          <GrConnect />,
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
      appendTab(
        new TabContent("setting", <ModifySetting />, <IoMdSettings />, settingId),
      );
    });
    return () => {
      unlisten.then((callback) => {
        callback();
      });
    };
  }, [appendTab, setCurrentTab, tabs]);

  const toggleShow = () => {
    setSwitch(showTopKey, !show);
  };
  const theme = useTheme();
  return (
    <Stack style={{ padding: "2px" }}>
      <Stack>
        {getSwitch(showTopKey) ? (
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
          display: show ? "flex" : "none",
        }}>
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
            }}>
            <GrConnect style={{ fontSize: "2.1rem" }}> </GrConnect>
          </IconButton>
        </Tooltip>

        <Tooltip title="Favorite sql statement">
          <IconButton
            onClick={() => {
              appendTab(
                new TabContent("favorite sql", <FavoriteSqlList />, <FaBookmark />),
              );
            }}>
            <FaBookmark style={{ fontSize: "2.1rem" }}> </FaBookmark>
          </IconButton>
        </Tooltip>

        <Tooltip title="copy a connection">
          <IconButton
            onClick={() => {
              appendTab(
                new TabContent("copy connection", <CopyConnection />, <FaCopy />),
              );
            }}>
            <FaCopy style={{ fontSize: "2.2rem" }}> </FaCopy>
          </IconButton>
        </Tooltip>

        <Tooltip
          title={
            matchPattern.length > 0
              ? `filter on ${matchPattern.join(",")}`
              : "show editor filter,can match table,view etc..."
          }>
          <IconButton
            onClick={() => {
              setShowEditFilter(true);
            }}>
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
        <Dialog
          fullWidth
          maxWidth="sm"
          open={showEditFilter}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setShowEditFilter(false);
          })}>
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
            }}>
            <Typography color="info">
              match pattern for database schema,table,view,udf,procedure etc
            </Typography>
            <Stack direction="row" style={{ flexWrap: "wrap", alignItems: "center" }}>
              {matchPattern.length > 1 && (
                <IconButton
                  onClick={() => {
                    setMatchPattern([]);
                    setShowEditFilter(false);
                  }}>
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
                    setMatchPattern(
                      matchPattern.filter((_, eIndex) => eIndex !== index),
                    );
                  }}
                />
              ))}
            </Stack>
            <Stack direction="row" style={{ alignItems: "flex-end" }} spacing={1}>
              <ClearableTextField
                fullWidth
                value={matchPatterInput}
                label="match pattern"
                onChange={(e) => {
                  setMatchPatternInput(e.target.value.trim());
                }}
                onClear={() => setMatchPatternInput("")}
              />
              <Button
                onClick={() => {
                  if (matchPattern.find((e) => e == matchPatterInput)) {
                    showNotification({
                      severity: "warning",
                      title: "the value already exists",
                    });
                    return;
                  }
                  if (!matchPatterInput) {
                    showNotification({
                      severity: "warning",
                      title: "please input match pattern",
                    });
                    return;
                  }
                  setMatchPattern([...matchPattern, matchPatterInput]);
                }}
                variant="outlined">
                apply
              </Button>
            </Stack>
          </Stack>

          <StackRowCenter>
            <span color="info">max tree size({maxTree}):</span>
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
                setMaxTree(value);
              }}
              style={{
                flex: 1,
                width: "40%",
              }}
            />
          </StackRowCenter>
        </Dialog>
      )}
    </Stack>
  );
}

function CopyConnectionMenu(props: { content: string }) {
  const { showNotification } = useNotification();
  const { content } = props;
  return (
    <MenuList>
      {content && (
        <MenuItem
          onClick={() => {
            handleCopyToClipboard(content, showNotification);
          }}>
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
  const { connections } = useConnectionStore();
  const [sortModel, setSortModel] = useState<GridSortModel>([]);
  const getRowId = useCallback((e) => {
    return e.name;
  }, []);
  const onSortModelChange = useCallback((e) => {
    setSortModel(e);
  }, []);

  const { getColumnWidth, setColumnWidth } = useColumnWidthStore();

  const setUpMenu = useContextMenu((state) => state.setUpMenu);

  const { appendTab } = useTabsManage();

  const handleRow = useCallback(
    async (conn) => {
      const password = await PassWordManager.getPassword(
        PassWordManager.serviceConnection,
        conn.name,
      );
      const cop = { ...conn };
      if (password) {
        cop["password"] = password;
      }
      appendTab(
        new TabContent("add connection", <AddConnection config={cop} />, <GrConnect />),
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
    } as GridSlotsComponentsProps;
  }, [setUpMenu]);

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
      const width = getColumnWidth(e.field);

      e.width = width;
    });
    return cols;
  }, [getColumnWidth]);

  const onColumnResize = useCallback(
    ({ colDef, width }) => {
      setColumnWidth(colDef.field, width);
    },
    [setColumnWidth],
  );

  const onRowDoubleClick = useCallback(
    (row) => {
      handleRow(row.row);
    },
    [handleRow],
  );

  const slots = useMemo(() => {
    return {
      pagination: () => {
        return (
          <Stack direction="row" style={{ justifyContent: "space-between", flex: 1 }}>
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
      <DataGrid
        getRowId={getRowId}
        style={dataGridStyle}
        slots={slots}
        sortModel={sortModel}
        onSortModelChange={onSortModelChange}
        slotProps={slotProps}
        columns={cols}
        rows={connections}
        onColumnResize={onColumnResize}
        onRowDoubleClick={onRowDoubleClick}
      />
    </Stack>
  );
}

function Left() {
  const connections = useConnectionStore((s) => s.connections);

  return (
    <Stack
      style={{
        overflow: "auto",
      }}>
      <LeftTop />

      <Stack spacing={0.3}>
        {connections.map((e) => {
          return (
            <UuidContext.Provider
              value={{
                uuid: e.name,
                kind: connTypeToDataSourceKind(e.conn_type),
              }}
              key={e.name}>
              <AConnection key={e.name} config={e} />
            </UuidContext.Provider>
          );
        })}
        {connections.length == 0 && (
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
  const { showNotification } = useNotification();
  const { deleteById } = useFavoriteStore();
  return (
    <MenuList>
      {content && (
        <MenuItem
          onClick={() => {
            handleCopyToClipboard(content, showNotification);
          }}>
          <ListItemIcon>
            <ContentCopyIcon />
          </ListItemIcon>
          <ListItemText> Copy </ListItemText>
        </MenuItem>
      )}

      <MenuItem
        onClick={async () => {
          try {
            await deleteById(id);
            handleReload();
          } catch (e) {
            showNotification({
              severity: "error",
              title: "delete by id failed",
              message: e,
            });
          }
        }}>
        <ListItemIcon>
          <DeleteForeverIcon />
        </ListItemIcon>
        <ListItemText> delete </ListItemText>
      </MenuItem>
    </MenuList>
  );
}

function FavoriteSqlList() {
  const { showNotification } = useNotification();
  const [fs, setFs] = useState<FavoriteSql[]>([]);

  const [sortModel, setSortModel] = useState<GridSortModel>([
    { field: "lastModified", sort: "desc" },
  ]);

  const [uuidFilter, setUuidFilter] = useState("");
  const [distinctUuid, setDistinctUuid] = useState<string[]>([]);
  const { getColumnWidth, setColumnWidth } = useColumnWidthStore();

  const { getDistinctUuids, getByUuid, clearByUuid } = useFavoriteStore();

  const handleReload = useCallback(
    async (showOK) => {
      try {
        const us = await getDistinctUuids();
        setDistinctUuid(us);
        const fs = await getByUuid(uuidFilter);
        setFs(fs);
      } catch (e) {
        showNotification({
          severity: "error",
          title: "list all favorite sql failed",
          message: e,
        });
        return;
      }
      if (showOK) {
        showNotification({ severity: "success", title: "fetch favorite sql ok" });
      }
    },
    [getByUuid, getDistinctUuids, showNotification, uuidFilter],
  );

  const { appendTab } = useTabsManage();
  const is_connected = useConnected((s) => s.is_connected);
  const checkConnOpened = useCallback(
    async (uuid: string) => {
      const connOpened = is_connected(uuid);
      if (!connOpened) {
        showNotification({
          severity: "error",
          title: `connection '${extractConfigNameFromUuid(uuid)}' not opened????`,
        });
        return false;
      } else {
        return true;
      }
    },
    [is_connected, showNotification],
  );

  const handleRow = useCallback(
    async (row: FavoriteSql) => {
      try {
        if (!(await checkConnOpened(row.uuid))) {
          return;
        }
        const content = (
          <AllKindChat
            kind={row.kind as DataSourceKind}
            uuid={row.uuid}
            value={row.sql}
            favoriteName={row.name}
            language={row.language}
          />
        );
        appendTab(new TabContent(`${row.uuid}`, content, <TbDatabaseSearch />));
      } catch (e) {
        showNotification({ severity: "error", title: "use sql failure", message: e });
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

  const setUpMenu = useContextMenu((state) => state.setUpMenu);
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
    } as GridSlotsComponentsProps;
  }, [handleReload, setUpMenu]);

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
      {
        field: "last_modified",
        renderCell: (params) => {
          console.log(params.value);
          return new Date(params.value).toLocaleString();
        },
        renderHeader: () => {
          return "last modified";
        },
      },
    ] as GridColDef[];
    cols.forEach((e) => {
      const width = getColumnWidth(e.field);
      e.width = width;
    });
    return cols;
  }, [getColumnWidth]);
  const onColumnResize = useCallback(
    ({ colDef, width }) => {
      setColumnWidth(colDef.field, width);
    },
    [setColumnWidth],
  );

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
    try {
      clearByUuid(uuidFilter);
    } catch (e) {
      showNotification({ severity: "error", title: "clean failed", message: e });
    }
  };
  const slots = useMemo(() => {
    return {
      pagination: () => {
        return (
          <Stack direction="row" style={{ justifyContent: "space-between", flex: 1 }}>
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
          }}>
          reload
        </Button>
        {fs.length > 0 && uuidFilter && (
          <Tooltip title={`delete all sql by uuid '${uuidFilter}'`}>
            <IconButton
              onClick={() => {
                handleDelete();
              }}>
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

function AConnection(props: { config: DBConnectionConfig }) {
  const { config } = props;
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
  return <SimpleTreeView disableSelection>{makeTree()}</SimpleTreeView>;
}

function SchemaTree(props: { schema?: string; reloadSchemas? }) {
  const { uuid, kind } = useContext(UuidContext);
  const { schema, reloadSchemas } = props;
  const { is_connected } = useConnected();
  const SUPPORTED_PROCEDURE_KINDS = new Set([
    DataSourceKind.MariaDB,
    DataSourceKind.Mysql,
    DataSourceKind.Postgres,
    DataSourceKind.SqlServer,
    DataSourceKind.Oracle,
    DataSourceKind.Cockroach,
  ]);

  const conn_established = is_connected(uuid);
  const loadingConn = useConnLoading();

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
  const setUpMenu = useContextMenu((state) => state.setUpMenu);
  const idForTree = useRef(uuidv4());
  return (
    <TreeItem
      itemId={idForTree.current}
      label=<Stack
        direction="row"
        onContextMenu={(e) => {
          setUpMenu({
            content: (
              <SchemaMenu
                schema={schema}
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
        }}>
        {schema && <LuDatabase />}
        <span> {schema ? schema : uuid}</span>
        {!schema && conn_established && <ConnPingTime uuid={uuid} />}
        {loadingConn.isLoading(uuid) && <CircularProgress />}
      </Stack>>
      {conn_established && kind == DataSourceKind.Sqlite && (
        <>
          <TablesTree />
          <ViewsTree viewKind={ViewKind.View} />
        </>
      )}
      {conn_established && kind == DataSourceKind.CouchBase && (
        <>
          <TablesTree />
        </>
      )}

      {conn_established && kind == DataSourceKind.DuckDB && (
        <>
          <TablesTree />
          <ViewsTree viewKind={ViewKind.View} />
        </>
      )}
      {conn_established && kind == DataSourceKind.FireBird && (
        <>
          <TablesTree />
          <ViewsTree viewKind={ViewKind.View} />
        </>
      )}
      {conn_established && kind == DataSourceKind.LibSql && (
        <>
          <TablesTree />
          <ViewsTree viewKind={ViewKind.View} />
        </>
      )}
      {conn_established && kind == DataSourceKind.Cassandra && (
        <>
          <TablesTree />
          <ViewsTree viewKind={ViewKind.MaterializedView} />
        </>
      )}
      {conn_established && kind == DataSourceKind.Mongo && (
        <>
          <TablesTree />
          <ViewsTree viewKind={ViewKind.View} />
        </>
      )}
      {conn_established &&
        (kind == DataSourceKind.Mysql ||
          kind == DataSourceKind.MariaDB ||
          kind == DataSourceKind.TiDB) && (
          <>
            <TablesTree />
            <ViewsTree viewKind={ViewKind.View} />
          </>
        )}
      {conn_established && kind == DataSourceKind.ClickHouse && (
        <>
          <TablesTree />
          <ViewsTree viewKind={ViewKind.View} />
          <ViewsTree viewKind={ViewKind.MaterializedView} />
        </>
      )}
      {conn_established && kind == DataSourceKind.Postgres && (
        <>
          <TablesTree />
          <ViewsTree viewKind={ViewKind.View} />
          <ViewsTree viewKind={ViewKind.MaterializedView} />
        </>
      )}
      {conn_established && kind == DataSourceKind.Cockroach && (
        <>
          <TablesTree />
          <ViewsTree viewKind={ViewKind.View} />
          <ViewsTree viewKind={ViewKind.MaterializedView} />
        </>
      )}
      {conn_established && kind == DataSourceKind.Oracle && (
        <>
          <TablesTree />
          <ViewsTree viewKind={ViewKind.View} />
          <ViewsTree viewKind={ViewKind.MaterializedView} />
        </>
      )}
      {conn_established && kind == DataSourceKind.SqlServer && (
        <>
          <TablesTree />
          <ViewsTree viewKind={ViewKind.View} />
        </>
      )}
      {conn_established && (
        <>
          {supportProcedure && <ProceduresTree />}
          {supportUdf && <UdfsTree />}
        </>
      )}
      {conn_established && <>{supportSequence && <SequenceTree />}</>}

      {conn_established && kind == DataSourceKind.Cassandra && (
        <CassandraAggregatesTree />
      )}
      {conn_established && kind == DataSourceKind.Redis && <RedisTables />}
      {conn_established && supportType && <TypesTree />}
      {conn_established && supportEvent && <EventsTree />}
    </TreeItem>
  );
}

function DatabaseMenu(props: {
  database?: string;
  uuid: string;
  reload;
  reloadDatabase;
  kind: DataSourceKind;
  schemas: string[];
}) {
  const { database, uuid, reload, reloadDatabase, kind, schemas } = props;
  const { showNotification } = useNotification();
  const { appendTab } = useTabsManage();
  const { is_connected } = useConnected();

  const conn_established = is_connected(uuid);

  const { setUi } = useOneShotDialogUI();

  return (
    <MenuList>
      {conn_established &&
        kind != DataSourceKind.Mongo &&
        kind != DataSourceKind.Redis &&
        kind != DataSourceKind.CouchBase && (
          <MenuNewQuery
            handleNew={() => {
              openNewQuery(appendTab, uuid, kind);
            }}
          />
        )}
      {conn_established &&
        kind != DataSourceKind.Mongo &&
        kind != DataSourceKind.Redis &&
        kind != DataSourceKind.CouchBase && (
          <MenuNewQueryNewWindow
            handleNew={() => {
              openNewQueryWindow(uuid, kind);
            }}
          />
        )}
      {conn_established && kind != DataSourceKind.Oracle && (
        <MenuCreateSchema
          handleCreate={() => {
            setUi(
              <CreateSchema
                uuid={uuid}
                handleClose={() => {
                  reload();
                  setUi(null);
                }}
                kind={kind}
                schemas={schemas}
                handleReload={reload}
              />,
              "create " +
                (function () {
                  let name = "database";
                  switch (kind) {
                    case DataSourceKind.Postgres:
                    case DataSourceKind.Cockroach:
                    case DataSourceKind.SqlServer:
                      name = "schema";
                      break;
                  }
                  return `create ${name}`;
                })(),
            );
          }}
          kind={kind}
        />
      )}

      {!conn_established && !database && <MenuOpenConn name={uuid} />}

      {conn_established && <MenuRefresh reload={reload} />}
      {conn_established && !database && <MenuConnectionInfo uuid={uuid} />}

      {conn_established && !database && <MenuCloseConn name={uuid} />}
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
          }}>
          <ListItemIcon>
            <DeleteForeverIcon color="error" />
          </ListItemIcon>
          <ListItemText color="error">drop</ListItemText>
        </MenuItem>
      )}

      {!database && <MenuDeleteConfig name={uuid} />}
    </MenuList>
  );
}

function CatalogMenu(props: { uuid; reload; kind: DataSourceKind }) {
  const { uuid, reload, kind } = props;
  const { is_connected } = useConnected();
  const conn_established = is_connected(uuid);
  const { setUi } = useOneShotDialogUI();
  return (
    <div>
      <MenuList>
        {!conn_established && <MenuOpenConn name={uuid} />}
        {conn_established && kind != DataSourceKind.DuckDB && (
          <MenuCreateDatabase
            handleCreate={() => {
              setUi(
                <CreateDatabase
                  uuid={uuid}
                  handleClose={() => {
                    reload();
                    setUi(null);
                  }}
                  kind={kind}
                />,
                (function () {
                  switch (kind) {
                    case DataSourceKind.CouchBase:
                      return "create bucket";
                    default:
                      return "create database";
                  }
                })(),
              );
            }}
            kind={kind}
          />
        )}

        {conn_established && <MenuRefresh reload={reload} />}
        {conn_established && <MenuConnectionInfo uuid={uuid} />}
        {conn_established && <MenuCloseConn name={uuid} />}
        <Divider />
        <MenuDeleteConfig name={uuid} />
      </MenuList>
    </div>
  );
}

function AllKindQuery(props: {
  kind: DataSourceKind;
  uuid: string;
  value?: string;
  favoriteName?: string;
  language?: string;
}) {
  const { kind, uuid, value, favoriteName, language } = props;
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
        <SqlQuery uuid={uuid} value={value} kind={kind} favoriteName={favoriteName} />
      );
      break;
    case DataSourceKind.Redis:
      content = <RedisQuery uuid={uuid} value={value} favoriteName={favoriteName} />;
      break;

    case DataSourceKind.Mongo:
      content = (
        <MongoQuery
          language={language}
          uuid={uuid}
          value={value}
          favoriteName={favoriteName}
        />
      );
      break;
    case DataSourceKind.CouchBase:
      return <CouchBaseQuery uuid={uuid} value={value} />;
    default:
      console.error(`impl this ${kind}`);
  }
  return <div style={{ height: "100%" }}> {content}</div>;
}

function openNewQuery(appendTab, uuid: string, kind: DataSourceKind) {
  console.log("open new query uuid:", uuid);
  const content = <AllKindChat kind={kind} uuid={uuid} />;
  appendTab(new TabContent(`${uuid}`, content, <TbDatabaseSearch />));
}

function openNewQueryWindow(uuid: string, kind: DataSourceKind) {
  console.log("open new query uuid:", uuid);

  let queryId = uuidv4();

  const webview = new WebviewWindow(uuidv4(), {
    url: `query.html?uuid=${uuid}&kind=${kind}&queryId=${queryId}`,
    title: `query windows of ${uuid}`,
    maximized: true,
  });
  webview.once("tauri://window-created", () => {
    console.log("create query window succeeded");
  });
  webview.once("tauri://destroyed", () => {
    new QueryConn(uuid, queryId, kind).destroy();
  });
}

function openNewCreateTable(appendTab, uuid: string, kind: DataSourceKind) {
  const content = <CreateTable kind={kind} uuid={uuid} />;
  appendTab(new TabContent(`create table`, content, <IoIosCreate> </IoIosCreate>));
}

function SchemaMenu(props: {
  schema?: string;
  uuid: string;
  kind: DataSourceKind;
  reloadSchemas?;
}) {
  const { schema, uuid, kind, reloadSchemas } = props;
  const { appendTab } = useTabsManage();
  const { showNotification } = useNotification();
  const { is_connected } = useConnected();
  const conn_established = is_connected(uuid);

  return (
    <MenuList>
      {!schema && !conn_established && <MenuOpenConn name={uuid} />}
      {conn_established && (
        <MenuNewQuery
          handleNew={() => {
            openNewQuery(appendTab, uuid, kind);
          }}
        />
      )}
      {conn_established && (
        <MenuNewQueryNewWindow
          handleNew={() => {
            openNewQueryWindow(uuid, kind);
          }}
        />
      )}

      {conn_established &&
        kind != DataSourceKind.Redis &&
        kind != DataSourceKind.Mongo &&
        kind != DataSourceKind.CouchBase && (
          <MenuItem
            onClick={() => {
              openNewCreateTable(appendTab, uuid, kind);
            }}>
            <ListItemIcon>
              <IoIosCreate />
            </ListItemIcon>
            <ListItemText>create table </ListItemText>
          </MenuItem>
        )}

      {conn_established && !schema && <MenuConnectionInfo uuid={uuid} />}

      {!schema && conn_established && <MenuCloseConn name={uuid} />}
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
          }}>
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
  const { uuid: name, kind } = useContext(UuidContext);
  const { showNotification } = useNotification();
  const [databases, setDatabases] = useState<string[]>([]);
  const { isLoading, setLoading } = useConnLoading();
  const { is_connected } = useConnected();
  const conn_established = is_connected(name);
  const [showingDatabases, setShowingDatabases] = useState<string[]>([]);
  const matchPattern = useMatcherFn();
  useEffect(() => {
    setShowingDatabases(databases.filter(matchPattern));
  }, [databases, matchPattern]);
  const handleReload = useCallback(() => {
    if (!conn_established) {
      setDatabases([]);
      return;
    }
    setLoading(name, true);
    commands
      .listDatabase(name)
      .then((ds) => {
        setDatabases(ds);
      })
      .catch((e) => {
        showNotification({
          severity: "error",
          title: "list database failed",
          message: e,
        });
      })
      .finally(() => {
        setLoading(name, false);
      });
  }, [name, conn_established, setLoading, showNotification]);

  useEffect(() => {
    handleReload();
  }, [handleReload]);
  const setUpMenu = useContextMenu((state) => state.setUpMenu);

  return (
    <TreeItem
      itemId={name}
      label=<Stack
        direction="row"
        onContextMenu={(e) => {
          setUpMenu({
            content: <CatalogMenu uuid={name} reload={handleReload} kind={kind} />,
            event: e,
          });
        }}>
        <span>{name} </span>
        {isLoading(name) && <CircularProgress />}
        {conn_established && <ConnPingTime uuid={name} />}
        <ShowingCount total={databases.length} showing={showingDatabases.length} />
      </Stack>
      key={name}>
      {showingDatabases.map((db) => {
        return (
          <UuidContext.Provider key={db} value={{ uuid: name + "/" + db, kind }}>
            <DatabaseTree key={db} database={db} reloadDatabase={handleReload} />
          </UuidContext.Provider>
        );
      })}
    </TreeItem>
  );
}

function DatabaseTree(props: { database?: string; reloadDatabase?: () => void }) {
  const { uuid: name, kind } = useContext(UuidContext);
  const { showNotification } = useNotification();
  const [schemas, setSchemas] = useState<string[]>([]);
  const { database, reloadDatabase } = props;
  const { isLoading, setLoading } = useConnLoading();
  const { is_connected } = useConnected();
  const matchPattern = useMatcherFn();
  const [showingSchemas, setShowingSchemas] = useState<string[]>([]);
  useEffect(() => {
    setShowingSchemas(schemas.filter((schema) => matchPattern(schema)));
  }, [schemas, matchPattern]);

  const conn_established = is_connected(name);
  const handleReload = useCallback(() => {
    if (!conn_established) {
      setSchemas([]);
      return;
    }
    setLoading(name, true);
    commands
      .listSchema(name)
      .then((ds) => {
        setSchemas(ds);
      })
      .catch((e) => {
        showNotification({
          severity: "error",
          title: "list schema failed",
          message: e,
        });
      })
      .finally(() => {
        setLoading(name, false);
      });
  }, [name, conn_established, setLoading, showNotification]);

  useEffect(() => {
    handleReload();
  }, [handleReload]);
  const setUpMenu = useContextMenu((state) => state.setUpMenu);

  return (
    <TreeItem
      itemId={name}
      label=<Stack
        direction="row"
        sx={{ alignContent: "baseline", alignItems: "center" }}
        onContextMenu={(e) => {
          setUpMenu({
            content: (
              <DatabaseMenu
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
        }}>
        {database && <FaDatabase />}
        <span>{database ? database : name}</span>
        {isLoading(name) && <CircularProgress />}
        {!database && is_connected(name) && <ConnPingTime uuid={name} />}
        <ShowingCount total={schemas.length} showing={showingSchemas.length} />
      </Stack>
      key={name + database}>
      {showingSchemas.map((schema) => {
        return (
          <UuidContext.Provider
            key={schema}
            value={{ uuid: name + "/" + schema, kind }}>
            <SchemaTree key={schema} schema={schema} reloadSchemas={handleReload} />
          </UuidContext.Provider>
        );
      })}
    </TreeItem>
  );
}

export { Left, AllKindQuery };
