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
  Stack,
  TextField,
  Tooltip,
  Typography,
} from "@mui/material";
import { IoMdSettings } from "./icons";
import { useCallback, useContext, useEffect, useMemo, useState } from "react";
import {
  dropDatabase,
  dropSchema,
  EscDownEvent,
  modalHandleClose,
  PassWordManager,
  QueryConn,
  uuidv4,
} from "./Common";
import {
  ContentCopyIcon,
  GrConnect,
  LuDatabase,
  FaDatabase,
  FaCopy,
  DeleteForeverIcon,
  CloseIcon,
} from "./icons";
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
  DbnameContext,
  tabsHasID,
} from "./helper";
import { TbDatabaseSearch } from "./icons";

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
  CheckedView,
  MyTreeItem,
} from "./CommonComp.tsx";
import { KeyboardArrowDownIcon, KeyboardArrowRightIcon } from "./icons";
import { RedisTables, TablesTree } from "./LeftTable.tsx";
import { ViewKind, ViewsTree } from "./LeftView.tsx";
import { CassandraAggregatesTree, UdfsTree } from "./LeftUdf.tsx";
import { ProceduresTree } from "./LeftProcedure.tsx";

import { listen } from "@tauri-apps/api/event";
import { FaBookmark } from "./icons";
import { SequenceTree } from "./LeftSequence.tsx";
import { openUrl } from "@tauri-apps/plugin-opener";
import { TypesTree } from "./LeftType.tsx";
import {
  AppDataGridPagination,
  AppDataGridToolBar,
  ClearableTextField,
  CreateDatabase,
  CreateSchema,
  MenuConnectionInfo,
  RefreshButton,
  StackRowCenter,
} from "./CommonComp2.tsx";
import { EventsTree } from "./LeftEvent.tsx";
import { AllKindChat } from "./Chat.tsx";
import { AddConnection } from "./AddConnection.tsx";
import { ModifySetting } from "./Settings.tsx";
import { FilterAltIcon, FilterAltOutlinedIcon } from "./icons";
import {
  useMatcher,
  useConnLoading,
  useConnected,
  useContextMenu,
  useNotification,
  useTabsManage,
  useBoolSwitch,
  useColumnWidthStore,
  useConnectionStore,
  useFavoriteStore,
  FavoriteSql,
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

function LeftTop() {
  const showTopShowKey = "left-top-show";
  const { matchPattern } = useMatcher();
  const { getSwitch, setSwitch } = useBoolSwitch();
  const show = getSwitch(showTopShowKey);
  const { appendTab, tabs, setCurrentTab } = useTabsManage();
  const [showEditFilter, setShowEditFilter] = useState(false);
  const showAddConnection = useCallback(
    (conn?: DBConnectionConfig) => {
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
      const index = tabsHasID(tabs, settingId);

      if (index != -1) {
        setCurrentTab(index);
      } else {
        appendTab(
          new TabContent(
            "setting",
            <ModifySetting />,
            <IoMdSettings />,
            null,
            null,
            settingId,
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
    setSwitch(showTopShowKey, !show);
  };
  return (
    <Stack>
      <Stack>
        {getSwitch(showTopShowKey) ? (
          <IconButton onClick={toggleShow}>
            <KeyboardArrowDownIcon />
          </IconButton>
        ) : (
          <IconButton onClick={toggleShow}>
            <KeyboardArrowRightIcon />
          </IconButton>
        )}
      </Stack>
      {show && (
        <Stack
          direction="row"
          style={{
            justifyContent: "center",
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
      )}
      {show && (
        <Button
          variant="contained"
          color="success"
          onClick={() => {
            openUrl("https://dbpal.site/#donation");
          }}>
          donate
        </Button>
      )}
      {show && <Divider />}
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
          <EditLeftMatchStr
            handleClose={() => {
              setShowEditFilter(false);
            }}
          />
        </Dialog>
      )}
    </Stack>
  );
}

function EditLeftMatchStr({ handleClose }) {
  const { matchPattern, setMatchPattern } = useMatcher();
  const [matchPatterInput, setMatchPatternInput] = useState("");
  const { showNotification } = useNotification();

  return (
    <Stack
      style={{
        borderRadius: "0.5rem",
        padding: "2px 0.5rem",
      }}>
      <Typography color="info">
        match pattern table,view,udf,procedure etc...
      </Typography>
      <Stack direction="row" style={{ flexWrap: "wrap", alignItems: "center" }}>
        {matchPattern.length > 1 && (
          <IconButton
            onClick={() => {
              setMatchPattern([]);
              handleClose();
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
              setMatchPattern(matchPattern.filter((_, eIndex) => eIndex !== index));
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
  const { getColumnWidth } = useColumnWidthStore();
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
            content: <CopyConnectionMenu content={content} />,
            event: e,
          });
        },
        style: { cursor: "context-menu" },
      },
    };
  }, [setUpMenu]);

  const cols = useMemo((): GridColDef[] => {
    const cols: GridColDef[] = [
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
          return <CheckedView value={row.row["read_only"]} />;
        },
        renderHeader: () => {
          return "read only";
        },
      },
    ];
    cols.forEach((e) => {
      const width = getColumnWidth(e.field);

      e.width = width;
    });
    return cols;
  }, [getColumnWidth]);

  const onRowDoubleClick = useCallback(
    (row) => {
      handleRow(row.row);
    },
    [handleRow],
  );

  const slots = useMemo((): Partial<GridSlotsComponent> => {
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
    };
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
            <DbnameContext.Provider
              value={{
                dbName: e.name,
                kind: connTypeToDataSourceKind(e.conn_type),
              }}
              key={e.name}>
              <ConnectionView key={e.name} config={e} />
            </DbnameContext.Provider>
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
          if (
            !(await secondConfirm({
              confirmText: "Delete",
              title: "delete this favorite sql???",
            }))
          ) {
            return;
          }
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
  const { getColumnWidth } = useColumnWidthStore();

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
    async (dbName: string) => {
      const connOpened = is_connected(dbName);
      if (!connOpened) {
        showNotification({
          severity: "error",
          title: `connection '${extractConfigNameFromUuid(dbName)}' not opened????`,
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
        if (!(await checkConnOpened(row.dbName))) {
          return;
        }
        const content = <AllKindChat value={row.sql} favoriteName={row.name} />;
        appendTab(
          new TabContent(
            `${row.dbName}`,
            content,
            <TbDatabaseSearch />,
            row.dbName,
            row.kind as DataSourceKind,
          ),
        );
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
  const slotProps = useMemo((): GridSlotsComponentsProps => {
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
    };
  }, [handleReload, setUpMenu]);

  const columns = useMemo((): GridColDef[] => {
    const cols: GridColDef[] = [
      {
        field: "dbName",
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
    ];
    cols.forEach((e) => {
      const width = getColumnWidth(e.field);
      e.width = width;
    });
    return cols;
  }, [getColumnWidth]);

  const onRowDoubleClick = useCallback(
    (row) => {
      handleRow(row.row);
    },
    [handleRow],
  );

  const handleDelete = useCallback(async () => {
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
      handleReload(false);
    } catch (e) {
      showNotification({ severity: "error", title: "clean failed", message: e });
    }
  }, [clearByUuid, handleReload, showNotification, uuidFilter]);

  const slots = useMemo(() => {
    return {
      toolbar: () => {
        return (
          <AppDataGridToolBar
            titleName="favorite sql"
            left=<>
              <StackRowCenter>
                <Autocomplete
                  options={distinctUuid}
                  style={{ width: "22rem" }}
                  value={uuidFilter}
                  renderInput={(params) => (
                    <TextField {...params} label="filter connection name" />
                  )}
                  freeSolo={false}
                  onChange={(_, dbName) => {
                    setUuidFilter(dbName);
                  }}
                />
                <RefreshButton
                  onClick={() => {
                    handleReload(true);
                  }}
                />
                {fs.length > 0 && uuidFilter && (
                  <Tooltip title={`delete all sql by dbName '${uuidFilter}'`}>
                    <IconButton
                      onClick={() => {
                        handleDelete();
                      }}>
                      <DeleteForeverIcon color="error" />
                    </IconButton>
                  </Tooltip>
                )}
              </StackRowCenter>
            </>
          />
        );
      },
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
  }, [distinctUuid, fs.length, handleDelete, handleReload, uuidFilter]);

  return (
    <Stack style={{ height: "100%" }}>
      <DataGrid
        sortModel={sortModel}
        slots={slots}
        onSortModelChange={onSortModelChange}
        slotProps={slotProps}
        columns={columns}
        rows={fs}
        style={dataGridStyle}
        onRowDoubleClick={onRowDoubleClick}
      />
    </Stack>
  );
}

function ConnectionView(props: { config: DBConnectionConfig }) {
  const { config } = props;

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
}

function SchemaTree(props: { schema?: string; reloadSchemas? }) {
  const { dbName, kind } = useContext(DbnameContext);
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

  const conn_established = is_connected(dbName);
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
  return (
    <MyTreeItem
      enable={conn_established}
      makeMenu={() => {
        return (
          <SchemaMenu
            schema={schema}
            dbName={dbName}
            reloadSchemas={reloadSchemas}
            kind={kind}
          />
        );
      }}
      label=<>
        {schema && <LuDatabase />}
        <span> {schema ? schema : dbName}</span>
        {!schema && conn_established && <ConnPingTime />}
        {loadingConn.isLoading(dbName) && <CircularProgress />}
      </>>
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
    </MyTreeItem>
  );
}

function DatabaseMenu(props: {
  database?: string;
  dbName: string;
  reload: () => void;
  reloadDatabase;
  kind: DataSourceKind;
  schemas: string[];
  handleCreateSchema: () => void;
}) {
  const { database, dbName, reload, reloadDatabase, kind, handleCreateSchema } = props;
  const { showNotification } = useNotification();
  const { appendTab } = useTabsManage();
  const { is_connected } = useConnected();

  const conn_established = is_connected(dbName);

  return (
    <MenuList>
      {conn_established &&
        kind != DataSourceKind.Mongo &&
        kind != DataSourceKind.Redis &&
        kind != DataSourceKind.CouchBase && (
          <MenuNewQuery
            handleNew={() => {
              openNewQuery(appendTab, dbName, kind);
            }}
          />
        )}
      {conn_established &&
        kind != DataSourceKind.Mongo &&
        kind != DataSourceKind.Redis &&
        kind != DataSourceKind.CouchBase && (
          <MenuNewQueryNewWindow
            handleNew={() => {
              openNewQueryWindow(dbName, kind);
            }}
          />
        )}
      {conn_established && kind != DataSourceKind.Oracle && (
        <MenuCreateSchema
          handleCreate={() => {
            handleCreateSchema();
          }}
          kind={kind}
        />
      )}

      {!conn_established && !database && <MenuOpenConn name={dbName} />}

      {conn_established && <MenuRefresh refresh={reload} />}
      {!database && <MenuConnectionInfo dbName={dbName} />}

      {conn_established && !database && <MenuCloseConn name={dbName} />}
      {database && kind != DataSourceKind.DuckDB && (
        <MenuItem
          onClick={() => {
            // we are in database context dbName contains the database name
            // we need remove that
            const index = dbName.indexOf("/");
            dropDatabase({
              dbName: dbName.substring(0, index),
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

      {!database && <MenuDeleteConfig name={dbName} />}
    </MenuList>
  );
}

function CatalogMenu(props: {
  reload: () => void;
  handleCreateDatabase: () => void;
  dbName;
  kind: DataSourceKind;
}) {
  const { dbName, reload, kind, handleCreateDatabase } = props;
  const { is_connected } = useConnected();
  const conn_established = is_connected(dbName);
  return (
    <div>
      <MenuList>
        {!conn_established && <MenuOpenConn name={dbName} />}
        {conn_established && kind != DataSourceKind.DuckDB && (
          <MenuCreateDatabase
            handleCreate={() => {
              handleCreateDatabase();
            }}
            kind={kind}
          />
        )}
        {conn_established && <MenuRefresh refresh={reload} />}
        {<MenuConnectionInfo dbName={dbName} />}
        {conn_established && <MenuCloseConn name={dbName} />}
        <Divider />
        <MenuDeleteConfig name={dbName} />
      </MenuList>
    </div>
  );
}

function openNewQuery(
  appendTab: (_: TabContent) => void,
  dbName: string,
  kind: DataSourceKind,
) {
  console.log("open new query dbName:", dbName);
  const content = <AllKindChat />;
  appendTab(new TabContent(`${dbName}`, content, <TbDatabaseSearch />, dbName, kind));
}

function openNewQueryWindow(dbName: string, kind: DataSourceKind) {
  console.log("open new query dbName:", dbName);

  let queryId = uuidv4();

  const webview = new WebviewWindow(uuidv4(), {
    url: `query.html?dbName=${dbName}&kind=${kind}&queryId=${queryId}`,
    title: `query windows of ${dbName}`,
    maximized: true,
  });
  webview.once("tauri://window-created", () => {
    console.log("create query window succeeded");
  });
  webview.once("tauri://destroyed", () => {
    new QueryConn(dbName, queryId, kind).destroy();
  });
}

function SchemaMenu(props: {
  schema?: string;
  dbName: string;
  kind: DataSourceKind;
  reloadSchemas?;
}) {
  const { schema, dbName, kind, reloadSchemas } = props;
  const { appendTab } = useTabsManage();
  const { showNotification } = useNotification();
  const { is_connected } = useConnected();
  const conn_established = is_connected(dbName);

  return (
    <MenuList>
      {!schema && !conn_established && <MenuOpenConn name={dbName} />}
      {conn_established && (
        <MenuNewQuery
          handleNew={() => {
            openNewQuery(appendTab, dbName, kind);
          }}
        />
      )}
      {conn_established && (
        <MenuNewQueryNewWindow
          handleNew={() => {
            openNewQueryWindow(dbName, kind);
          }}
        />
      )}

      {!schema && <MenuConnectionInfo dbName={dbName} />}

      {!schema && conn_established && <MenuCloseConn name={dbName} />}
      {schema && kind != DataSourceKind.Oracle && (
        <MenuItem
          onClick={() => {
            // we are in database context dbName contains the database name
            // we need remove that
            const index = dbName.lastIndexOf("/");
            dropSchema({
              dbName: dbName.substring(0, index),
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
      {!schema && <MenuDeleteConfig name={dbName} />}
    </MenuList>
  );
}

function CatalogTree() {
  const { dbName: name, kind } = useContext(DbnameContext);
  const { showNotification } = useNotification();
  const [databases, setDatabases] = useState<string[]>([]);
  const { isLoading, setLoading } = useConnLoading();
  const { is_connected } = useConnected();
  const conn_established = is_connected(name);
  const [showCreateDatabase, setShowCreateDatabase] = useState(false);
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

  return (
    <MyTreeItem
      enable={conn_established}
      makeMenu={() => {
        return (
          <CatalogMenu
            reload={handleReload}
            handleCreateDatabase={() => {
              setShowCreateDatabase(true);
            }}
            dbName={name}
            kind={kind}
          />
        );
      }}
      label=<>
        <span>{name} </span>
        {isLoading(name) && <CircularProgress />}
        {conn_established && <ConnPingTime />}
      </>
      key={name}>
      {databases.map((db) => {
        return (
          <DbnameContext.Provider key={db} value={{ dbName: name + "/" + db, kind }}>
            <DatabaseTree key={db} database={db} reloadDatabase={handleReload} />
          </DbnameContext.Provider>
        );
      })}

      {showCreateDatabase && (
        <Dialog
          open={showCreateDatabase}
          onClose={() => {
            setShowCreateDatabase(false);
          }}>
          <CreateDatabase
            handleClose={() => {
              handleReload();
              setShowCreateDatabase(false);
            }}
          />
        </Dialog>
      )}
    </MyTreeItem>
  );
}

function DatabaseTree(props: { database?: string; reloadDatabase?: () => void }) {
  const { dbName: name, kind } = useContext(DbnameContext);
  const { showNotification } = useNotification();
  const [schemas, setSchemas] = useState<string[]>([]);
  const { database, reloadDatabase } = props;
  const { isLoading, setLoading } = useConnLoading();
  const { is_connected } = useConnected();

  const [showCreateSchama, setShowCreateSchema] = useState(false);

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
  return (
    <MyTreeItem
      enable={conn_established}
      makeMenu={() => {
        return (
          <DatabaseMenu
            database={database}
            dbName={name}
            reload={handleReload}
            reloadDatabase={reloadDatabase}
            kind={kind}
            handleCreateSchema={() => {
              setShowCreateSchema(true);
            }}
            schemas={schemas}
          />
        );
      }}
      label=<>
        {database && <FaDatabase />}
        <span>{database ? database : name}</span>
        {isLoading(name) && <CircularProgress />}
        {!database && is_connected(name) && <ConnPingTime />}
      </>
      key={name + database}>
      {schemas.map((schema) => {
        return (
          <DbnameContext.Provider
            key={schema}
            value={{ dbName: name + "/" + schema, kind }}>
            <SchemaTree key={schema} schema={schema} reloadSchemas={handleReload} />
          </DbnameContext.Provider>
        );
      })}

      {showCreateSchama && (
        <Dialog
          open={showCreateSchama}
          onClose={() => {
            setShowCreateSchema(false);
          }}>
          <CreateSchema
            handleClose={() => {
              handleReload();
              setShowCreateSchema(false);
            }}
            schemas={schemas}
            handleReload={handleReload}
          />
        </Dialog>
      )}
    </MyTreeItem>
  );
}

export { Left };
