import {
  Box,
  Button,
  CircularProgress,
  Divider,
  IconButton,
  ListItemIcon,
  ListItemText,
  MenuItem,
  MenuList,
  Modal,
  Paper,
  Stack,
  TextField,
  Tooltip,
  Typography,
} from '@mui/material';

import {
  memo,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
} from 'react';
import ContentCopyIcon from '@mui/icons-material/ContentCopy';
import { GrConnect } from 'react-icons/gr';
import {
  progressBarSizeSmall,
  appStore,
  AppStoreKey,
  OpenedContext,
  MenuConnectionInfo,
  CreateDatabase,
  modalStyle,
  dropDatabase,
  CreateSchema,
  dropSchema,
  initMonacoLanague,
  makeQueryConnInfo,
  QueryConn,
  SettingContext,
  EscDownEvent,
} from './Common';
import { LuDatabase } from 'react-icons/lu';
import { FaDatabase } from 'react-icons/fa';
import { PiFileSqlFill } from 'react-icons/pi';
import { FaCopy } from 'react-icons/fa';
import types from './ApiStruct';
import { invoke } from '@tauri-apps/api/core';
import { NotificationContext } from './Notification';
import { SimpleTreeView } from '@mui/x-tree-view/SimpleTreeView';
import { TreeItem } from '@mui/x-tree-view/TreeItem';
import DeleteForeverIcon from '@mui/icons-material/DeleteForever';

import { TabContent } from './Tabs';
import wcmatch from 'wildcard-match';

import {
  calMenuStyle,
  columnWidths,
  ConnTypeContext,
  connTypeToDataSourceKind,
  ContextMenuContext,
  datagridContainerHeight,
  DataSourceKind,
  extractConfigNameFromUuid,
  FavoriteSql,
  generateRandomString,
  handleCopyToClipboard,
  MatchPatternContext,
  secondConfirm,
  TabsSetContext,
  UuidContext,
} from './helper';
import { TbDatabaseSearch } from 'react-icons/tb';
import { SqlQuery } from './SqlQuery';
import { RedisQuery } from './RedisQuery';
import { MongoQuery } from './MongoQuery.tsx';
import React from 'react';
import {
  ConnPingTime,
  MenuCloseConn,
  MenuCreateDatabase,
  MenuCreateSchema,
  MenuDeleteConfig,
  MenuNewQuery,
  MenuOpenConn,
  MenuRefresh,
} from './CommonComp.tsx';
import KeyboardArrowDownIcon from '@mui/icons-material/KeyboardArrowDown';
import KeyboardArrowRightIcon from '@mui/icons-material/KeyboardArrowRight';
import { RedisTables, TablesTree, TablesType } from './LeftTable.tsx';
import { ViewKind, ViewsTree } from './LeftView.tsx';
import { CassaandraAggregatesTree, UdfsTree } from './LeftUdf.tsx';
import { ProceduresTree } from './LeftProcedure.tsx';
import {
  DataGrid,
  GridColDef,
  GridSlotsComponentsProps,
  GridSortModel,
} from '@mui/x-data-grid';
import { listen } from '@tauri-apps/api/event';
import { AddConnection } from './AddConnection.tsx';

function LeftTop(props: { matchPattern: string; setMatchPattern }) {
  const { matchPattern, setMatchPattern } = props;
  const [showFavoriteSql, setShowFavoriteSql] = useState(false);
  const [showAddConnection, setShowAddConnection] = useState(false);
  const [copyConn, setCopyConn] = useState<types.DBConnectionConfig>(null);
  const [showCopyConn, setShowCopyConn] = useState(false);
  const [show, setShow] = useState(false);
  useEffect(() => {
    let unlisten = listen('add_config', () => {
      setShowAddConnection(true);
    });
    return () => {
      unlisten.then((e) => {
        e();
      });
    };
  }, []);

  const toggerShow = () => {
    setShow((e) => !e);
  };

  return (
    <Stack spacing={1}>
      <Stack>
        {show ? (
          <IconButton onClick={toggerShow}>
            <KeyboardArrowDownIcon></KeyboardArrowDownIcon>
          </IconButton>
        ) : (
          <IconButton onClick={toggerShow}>
            <KeyboardArrowRightIcon></KeyboardArrowRightIcon>
          </IconButton>
        )}
      </Stack>

      {show && (
        <Stack
          direction="row"
          style={{ justifyContent: 'center', flexWrap: 'wrap' }}
        >
          <Tooltip title="make a connection">
            <IconButton
              onClick={() => {
                setShowAddConnection(true);
              }}
            >
              <GrConnect style={{ fontSize: '2.1rem' }}> </GrConnect>
            </IconButton>
          </Tooltip>

          <Tooltip title="Favorite sql statement">
            <IconButton
              onClick={() => {
                setShowFavoriteSql(true);
              }}
            >
              <PiFileSqlFill style={{ fontSize: '2.8rem' }}> </PiFileSqlFill>
            </IconButton>
          </Tooltip>

          <Tooltip title="copy a connection">
            <IconButton
              onClick={() => {
                setShowCopyConn(true);
              }}
            >
              <FaCopy style={{ fontSize: '2.2rem' }}> </FaCopy>
            </IconButton>
          </Tooltip>
        </Stack>
      )}
      {show && (
        <TextField
          fullWidth
          value={matchPattern}
          size="small"
          label="match pattern for table view udf procedure ..."
          onChange={(e) => {
            setMatchPattern(e.target.value.trim());
          }}
          helperText="name etc"
        ></TextField>
      )}

      {showFavoriteSql && (
        <Modal
          open={showFavoriteSql}
          onClose={(e: EscDownEvent, reason) => {
            if (reason === 'backdropClick') {
              return;
            }
            e.preventDefault();
            setShowFavoriteSql(false);
          }}
        >
          <Box sx={modalStyle} width="75%">
            <FavoriteSqlList
              handleClose={() => {
                setShowFavoriteSql(false);
              }}
            ></FavoriteSqlList>
          </Box>
        </Modal>
      )}

      {showAddConnection && (
        <Modal
          open={showAddConnection}
          onClose={(e: EscDownEvent, reason) => {
            if (reason === 'backdropClick') {
              return;
            }
            e.preventDefault();
            setShowAddConnection(false);
          }}
        >
          <Box sx={modalStyle} width="55%">
            <AddConnection
              handleClose={() => {
                setShowAddConnection(false);
              }}
            ></AddConnection>
          </Box>
        </Modal>
      )}
      {copyConn && (
        <Modal
          open={!!copyConn}
          onClose={(e: EscDownEvent, reason) => {
            if (reason === 'backdropClick') {
              return;
            }

            e.preventDefault();
            setCopyConn(null);
          }}
        >
          <Box sx={modalStyle} width="55%">
            <AddConnection
              handleClose={() => {
                setCopyConn(null);
              }}
              config={copyConn}
            ></AddConnection>
          </Box>
        </Modal>
      )}

      {showCopyConn && (
        <Modal
          open={showCopyConn}
          onClose={(e: EscDownEvent, reason) => {
            if (reason === 'backdropClick') {
              return;
            }
            e.preventDefault();
            setShowCopyConn(false);
          }}
        >
          <Box sx={modalStyle} width="65%">
            <CopyConnection
              handleClose={() => {
                setShowCopyConn(false);
              }}
              setCopyConn={(conn) => {
                setCopyConn(conn);
              }}
            ></CopyConnection>
          </Box>
        </Modal>
      )}
    </Stack>
  );
}

function CopyConnection(props: { handleClose; setCopyConn }) {
  const { handleClose, setCopyConn } = props;
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

  function Menu(props: { content: string }) {
    const { content } = props;
    return (
      <Paper>
        <MenuList>
          {content && (
            <MenuItem
              onClick={() => {
                handleCopyToClipboard(content, showNotification);
              }}
            >
              <ListItemIcon>
                <ContentCopyIcon></ContentCopyIcon>
              </ListItemIcon>
              <ListItemText> Copy </ListItemText>
            </MenuItem>
          )}
        </MenuList>
      </Paper>
    );
  }

  const useRow = (conn) => {
    setCopyConn(conn);
    handleClose();
  };
  const slotProps = useMemo(() => {
    return {
      row: {
        onContextMenu: (e) => {
          const { target } = e;
          let content = (target as HTMLDivElement).innerHTML;
          let field = (target as HTMLDivElement).getAttribute('data-field');
          if (!field) {
            return;
          }
          setUpMenu({
            content: <Menu content={content}></Menu>,
            style: calMenuStyle(e, 200),
          });
        },
        style: { cursor: 'context-menu' },
      },
    } as GridSlotsComponentsProps;
  }, []);

  let cols = useMemo(() => {
    const cols = [
      { field: 'name' },
      {
        field: 'conn_type',
        renderHeader: () => {
          return 'kind';
        },
      },
      { field: 'host' },
      { field: 'path' },
      { field: 'port' },
      {
        field: 'db',
        renderHeader: () => {
          return 'database';
        },
      },
    ] as GridColDef[];
    cols.forEach((e) => {
      const width = columnWidths.getWidth(e.field);
      if (width) {
        e.width = width;
      }
    });
    return cols;
  }, []);

  const onColumnResize = useCallback(({ colDef, width }) => {
    columnWidths.saveColumnWidths(colDef.field, width);
  }, []);

  const onRowDoubleClick = useCallback((row) => {
    useRow(row.row);
  }, []);

  const handleReload = async () => {
    try {
      let conns = await (
        await appStore()
      ).get<types.DBConnectionConfig[]>(AppStoreKey.ConnectionConfig);
      setConns(conns);
    } catch (e) {
      showNotification('error', 'get connections failed', e);
    }
  };
  useEffect(() => {
    handleReload();
  }, []);

  return (
    <Stack spacing={1}>
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

        <Typography color="info">
          double click a row to copy a connection.
        </Typography>
      </Stack>

      <div
        style={{
          display: 'grid',
          height: datagridContainerHeight(conns.length),
        }}
      >
        <DataGrid
          getRowId={getRowId}
          sortModel={sortModel}
          onSortModelChange={onSortModelChange}
          showCellVerticalBorder
          showColumnVerticalBorder
          slotProps={slotProps}
          rowHeight={setting.datagridRowHeight}
          columnHeaderHeight={setting.datagridHeaderHeight}
          columns={cols}
          rows={conns}
          onColumnResize={onColumnResize}
          onRowDoubleClick={onRowDoubleClick}
        ></DataGrid>
      </div>
    </Stack>
  );
}

function Left_() {
  const [configs, setConfigs] = useState<types.DBConnectionConfig[]>([]);
  const { showNotification } = useContext(NotificationContext);
  const [matchPattern, setMatchPattern] = useState('');
  const handleReload = async () => {
    (await appStore())
      .get<types.DBConnectionConfig[]>(AppStoreKey.ConnectionConfig)
      .then((e) => {
        if (!e) {
          e = [];
        }
        setConfigs(e);
      })
      .catch((e) => {
        showNotification('error', 'list all config failure', e);
      });
  };
  useEffect(() => {
    handleReload();
  }, []);
  appStore().then((appstore) => {
    appstore.onKeyChange(AppStoreKey.ConnectionConfig, () => {
      handleReload();
    });
  });
  const m = useMemo(() => {
    return matchPattern
      ? wcmatch('*' + matchPattern + '*', { flags: 'i' })
      : () => {
          return true;
        };
  }, [matchPattern]);
  return (
    <Stack
      style={{
        overflow: 'auto',
      }}
    >
      <LeftTop
        matchPattern={matchPattern}
        setMatchPattern={setMatchPattern}
      ></LeftTop>
      <Divider></Divider>
      <Stack>
        {configs.map((e) => {
          return (
            <MatchPatternContext.Provider value={m} key={e.name}>
              <ConnTypeContext.Provider
                value={{ kind: connTypeToDataSourceKind(e.conn_type) }}
              >
                <UuidContext.Provider
                  value={{
                    uuid: e.name,
                  }}
                >
                  <OneConfig key={e.name} config={e}></OneConfig>
                </UuidContext.Provider>
              </ConnTypeContext.Provider>
            </MatchPatternContext.Provider>
          );
        })}
      </Stack>
      <div style={{ minHeight: '300px' }}></div>
    </Stack>
  );
}

const makeConn = async (uuid: string, kind: DataSourceKind) => {
  const queryId = await invoke<string>('query_sql_open_connection', {
    name: uuid,
  });
  console.log('open connection:', queryId);
  return new QueryConn(uuid, queryId, kind);
};
function FavoriteSqlList(props: { handleClose }) {
  const { handleClose } = props;
  const { showNotification } = useContext(NotificationContext);
  const [sqls, setSqls] = useState<FavoriteSql[]>([]);
  const [sortModel, setSortModel] = useState<GridSortModel>([
    { field: 'lastModified', sort: 'desc' },
  ]);
  const handleReload = async (showOK) => {
    try {
      const store = await appStore();
      let obj = await store.get<object>(FavoriteSql.key);
      if (!obj) {
        obj = {};
      }
      let sqls = [];
      Object.values(obj).forEach((e) => {
        sqls.push(e);
      });
      setSqls(sqls);
    } catch (e) {
      showNotification('error', 'list all favorite sql failed', e);
      return;
    }
    if (showOK) {
      showNotification('success', 'fetch favorite sql ok');
    }
  };
  const { setTabs, setSelectTab } = useContext(TabsSetContext);
  const useRow = async (row: FavoriteSql) => {
    initMonacoLanague(row.uuid, row.kind);
    try {
      let connOpend = await invoke<boolean>('conn_opend', { name: row.uuid });
      if (!connOpend) {
        showNotification(
          'error',
          `connection ${extractConfigNameFromUuid(row.uuid)} not opened????`
        );
        return;
      }

      setTabs((tabs) => {
        let content;
        switch (row.kind) {
          case DataSourceKind.Mongo:
            content = (
              <MongoQuery
                favoriteName={row.name}
                value={row.sql}
                uuid={row.uuid}
                saveFavoriteSql={(_name, sql) => {
                  FavoriteSql.save(
                    new FavoriteSql(row.uuid, row.name, row.kind, sql),
                    showNotification
                  );
                }}
              ></MongoQuery>
            );
            break;
          case DataSourceKind.Redis:
            content = (
              <RedisQuery
                language={row.uuid}
                makeConn={async () => {
                  return makeConn(row.uuid, row.kind);
                }}
                saveFavoriteSql={(_name, sql) => {
                  FavoriteSql.save(
                    new FavoriteSql(row.uuid, row.name, row.kind, sql),
                    showNotification
                  );
                }}
                value={row.sql}
                favoriteName={row.name}
              ></RedisQuery>
            );
            break;
          default:
            content = (
              <SqlQuery
                language={row.uuid}
                makeConn={async () => {
                  return makeConn(row.uuid, row.kind);
                }}
                value={row.sql}
                saveFavoriteSql={(_name, sql) => {
                  FavoriteSql.save(
                    new FavoriteSql(row.uuid, row.name, row.kind, sql),
                    showNotification
                  );
                }}
                favoriteName={row.name}
              ></SqlQuery>
            );
        }
        handleClose();
        setSelectTab(tabs.length);
        return [
          ...tabs,
          new TabContent(
            generateRandomString(10),
            `q@${row.uuid}`,
            content,
            <TbDatabaseSearch> </TbDatabaseSearch>
          ),
        ];
      });
    } catch (e) {
      showNotification('error', 'use sql failure', e);
    }
  };
  const onSortModelChange = useCallback((e) => {
    setSortModel(e);
  }, []);

  useEffect(() => {
    handleReload(false);
  }, []);
  const { setUpMenu } = useContext(ContextMenuContext);
  function Menu(props: { content: string; id: string }) {
    const { content, id } = props;
    return (
      <Paper>
        <MenuList>
          {content && (
            <MenuItem
              onClick={() => {
                handleCopyToClipboard(content, showNotification);
              }}
            >
              <ListItemIcon>
                <ContentCopyIcon></ContentCopyIcon>
              </ListItemIcon>
              <ListItemText> Copy </ListItemText>
            </MenuItem>
          )}

          <MenuItem
            onClick={async () => {
              await FavoriteSql.delete(id, showNotification);
              handleReload(false);
            }}
          >
            <ListItemIcon>
              <DeleteForeverIcon></DeleteForeverIcon>
            </ListItemIcon>
            <ListItemText> delete </ListItemText>
          </MenuItem>
        </MenuList>
      </Paper>
    );
  }
  const slotProps = useMemo(() => {
    return {
      row: {
        onContextMenu: (e) => {
          const { target, currentTarget } = e;
          let content = (target as HTMLDivElement).innerHTML;
          let field = (target as HTMLDivElement).getAttribute('data-field');
          if (!field) {
            return;
          }
          let id = (currentTarget as HTMLDivElement).getAttribute('data-id');
          setUpMenu({
            content: <Menu content={content} id={id}></Menu>,
            style: calMenuStyle(e, 200),
          });
        },
        style: { cursor: 'context-menu' },
      },
    } as GridSlotsComponentsProps;
  }, []);

  const setting = useContext(SettingContext);
  const columns = useMemo(() => {
    let cols = [
      {
        field: 'uuid',
        renderHeader: () => {
          return 'connection name';
        },
      },
      { field: 'name' },
      { field: 'kind' },
      { field: 'sql' },
      { field: 'lastModified' },
    ] as GridColDef[];
    cols.forEach((e) => {
      let width = columnWidths.getWidth(e.field);
      if (width) {
        e.width = width;
      }
    });
    return cols;
  }, []);

  const getRowId = useCallback((row) => {
    return FavoriteSql.formatKey(row['uuid'], row['name']);
  }, []);
  const onColumnResize = useCallback(({ colDef, width }) => {
    columnWidths.saveColumnWidths(colDef.field, width);
  }, []);

  const onRowDoubleClick = useCallback((row) => {
    useRow(row.row);
  }, []);

  const handleclean = async () => {
    let confirm = await secondConfirm({
      title: 'clean all favorite sql???',
      confirmText: 'Clean',
    });
    if (!confirm) {
      return;
    }
    FavoriteSql.clear(showNotification);
    handleClose();
  };

  return (
    <Stack spacing={1}>
      <Stack direction="row" spacing={1} style={{ alignItems: 'center' }}>
        <Button
          variant="contained"
          onClick={() => {
            handleReload(true);
          }}
        >
          reload
        </Button>
        {sqls.length > 0 && (
          <Tooltip title="delete all sql">
            <IconButton
              onClick={() => {
                handleclean();
              }}
            >
              <DeleteForeverIcon color="error"></DeleteForeverIcon>
            </IconButton>
          </Tooltip>
        )}
        {sqls.length > 0 && (
          <Typography color="info"> double click a row to use sql. </Typography>
        )}
      </Stack>
      <div
        style={{
          display: 'grid',
          height: datagridContainerHeight(sqls.length),
        }}
      >
        <DataGrid
          getRowId={getRowId}
          sortModel={sortModel}
          onSortModelChange={onSortModelChange}
          showCellVerticalBorder
          showColumnVerticalBorder
          slotProps={slotProps}
          rowHeight={setting.datagridRowHeight}
          columnHeaderHeight={setting.datagridHeaderHeight}
          columns={columns}
          rows={sqls}
          onColumnResize={onColumnResize}
          onRowDoubleClick={onRowDoubleClick}
        ></DataGrid>
      </div>
    </Stack>
  );
}

const OneConfig = memo(OneConfig_);
function OneConfig_(props: { config: types.DBConnectionConfig }) {
  let { config } = props;
  const [opened, setOpened] = useState();

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
        return config.db ? (
          <SchemaTree></SchemaTree>
        ) : (
          <DatabaseTree></DatabaseTree>
        );
      case DataSourceKind.Sqlite:
      case DataSourceKind.LibSql:
        return <SchemaTree></SchemaTree>;
      case DataSourceKind.Redis:
        return config.db ? (
          <SchemaTree></SchemaTree>
        ) : (
          <DatabaseTree></DatabaseTree>
        );
      case DataSourceKind.Mongo:
        return config.db ? (
          <SchemaTree></SchemaTree>
        ) : (
          <DatabaseTree></DatabaseTree>
        );
      case DataSourceKind.MsSql:
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
      default:
        console.log('impl database kind:', config);
        throw `impl this kind`;
    }
  };

  return (
    <OpenedContext.Provider value={{ opened, setOpened }}>
      <SimpleTreeView disableSelection>{makeTree()}</SimpleTreeView>
    </OpenedContext.Provider>
  );
}

function SchemaTree(prpos: { schema?: string; reloadSchemas? }) {
  const { kind } = useContext(ConnTypeContext);
  const { uuid } = useContext(UuidContext);
  const { schema, reloadSchemas } = prpos;
  const getPrefix = () => {
    return uuid + schema;
  };
  const { opened, setOpened } = useContext(OpenedContext);
  let support_procedure =
    kind == DataSourceKind.MariaDB ||
    kind == DataSourceKind.Mysql ||
    kind == DataSourceKind.Postgres ||
    kind == DataSourceKind.MsSql ||
    kind == DataSourceKind.Oracle ||
    kind == DataSourceKind.Cockroach;
  let support_udf =
    kind == DataSourceKind.MariaDB ||
    kind == DataSourceKind.Mysql ||
    kind == DataSourceKind.Postgres ||
    kind == DataSourceKind.MsSql ||
    kind == DataSourceKind.Oracle ||
    kind == DataSourceKind.Cassandra ||
    kind == DataSourceKind.Cockroach;
  let { setUpMenu } = useContext(ContextMenuContext);

  return (
    <TreeItem
      itemId={getPrefix()}
      label=<Stack
        spacing={1}
        direction="row"
        onContextMenu={(e) => {
          setUpMenu({
            content: (
              <SchemaMenu
                opened={opened}
                schema={schema}
                handleOpend={() => {
                  setOpened(true);
                }}
                handleCloseConn={() => {
                  setOpened(false);
                }}
                uuid={uuid}
                reloadSchemas={reloadSchemas}
                kind={kind}
              ></SchemaMenu>
            ),

            style: calMenuStyle(e, 70),
          });
        }}
        sx={{ alignItems: 'center' }}
      >
        {schema && <LuDatabase></LuDatabase>}
        <span> {schema ? schema : uuid}</span>
        {!schema && opened && <ConnPingTime uuid={uuid}></ConnPingTime>}
      </Stack>
    >
      {opened && kind == DataSourceKind.Sqlite && (
        <>
          <TablesTree typ={TablesType.Normal} />
          <ViewsTree viewkind={ViewKind.Normal} />
        </>
      )}

      {opened && kind == DataSourceKind.DuckDB && (
        <>
          <TablesTree typ={TablesType.Normal} />
          <ViewsTree viewkind={ViewKind.Normal} />
        </>
      )}
      {opened && kind == DataSourceKind.FireBird && (
        <>
          <TablesTree typ={TablesType.Normal} />
          <ViewsTree viewkind={ViewKind.Normal} />
        </>
      )}
      {opened && kind == DataSourceKind.LibSql && (
        <>
          <TablesTree typ={TablesType.Normal} />
          <ViewsTree viewkind={ViewKind.Normal} />
        </>
      )}
      {opened && kind == DataSourceKind.Cassandra && (
        <>
          <TablesTree typ={TablesType.Normal} />
          <ViewsTree viewkind={ViewKind.Materrialized}></ViewsTree>
        </>
      )}
      {opened && kind == DataSourceKind.Mongo && (
        <>
          <TablesTree typ={TablesType.Normal} />
          <TablesTree typ={TablesType.MongoTimeSerial} />
          <ViewsTree viewkind={ViewKind.Normal} />
        </>
      )}
      {opened &&
        (kind == DataSourceKind.Mysql ||
          kind == DataSourceKind.MariaDB ||
          kind == DataSourceKind.TiDB) && (
          <>
            <TablesTree typ={TablesType.Normal} />
            <ViewsTree viewkind={ViewKind.Normal} />
          </>
        )}
      {opened && kind == DataSourceKind.ClickHouse && (
        <>
          <TablesTree typ={TablesType.Normal} />
          <ViewsTree viewkind={ViewKind.Normal} />
          <ViewsTree viewkind={ViewKind.Materrialized} />
        </>
      )}
      {opened && kind == DataSourceKind.Postgres && (
        <>
          <TablesTree typ={TablesType.Normal} />
          <ViewsTree viewkind={ViewKind.Normal} />
          <ViewsTree viewkind={ViewKind.Materrialized} />
        </>
      )}
      {opened && kind == DataSourceKind.Cockroach && (
        <>
          <TablesTree typ={TablesType.Normal} />
          <ViewsTree viewkind={ViewKind.Normal} />
          <ViewsTree viewkind={ViewKind.Materrialized} />
        </>
      )}
      {opened && kind == DataSourceKind.Oracle && (
        <>
          <TablesTree typ={TablesType.Normal} />
          <ViewsTree viewkind={ViewKind.Normal} />
          <ViewsTree viewkind={ViewKind.Materrialized} />
        </>
      )}
      {opened && kind == DataSourceKind.MsSql && (
        <>
          <TablesTree typ={TablesType.Normal} />
          <ViewsTree viewkind={ViewKind.Normal} />
        </>
      )}
      {opened && (
        <>
          {support_procedure && <ProceduresTree />}
          {support_udf && <UdfsTree></UdfsTree>}
        </>
      )}
      {opened && kind == DataSourceKind.Cassandra && (
        <CassaandraAggregatesTree />
      )}
      {opened && kind == DataSourceKind.Redis && (
        <RedisTables schema={schema} />
      )}
    </TreeItem>
  );
}

function DatabaseMenu(props: {
  database?: string;
  uuid: string;
  reload;
  handleOpend;
  opened;
  handleCloseConn;
  reloadDatabase;
  kind: DataSourceKind;
  schemas: string[];
}) {
  const {
    database,
    uuid,
    reload,
    handleOpend,
    opened,
    handleCloseConn,
    reloadDatabase,
    kind,
    schemas,
  } = props;
  const { showNotification } = useContext(NotificationContext);
  const [showCreateSchema, setShowCreateSchema] = useState(false);
  const { setSelectTab, setTabs } = useContext(TabsSetContext);
  return (
    <Paper>
      <MenuList>
        {opened && kind != DataSourceKind.Oracle && (
          <MenuCreateSchema
            handleCreate={() => {
              setShowCreateSchema(true);
            }}
            kind={kind}
          ></MenuCreateSchema>
        )}

        {!opened && !database && (
          <MenuOpenConn name={uuid} handleOpend={handleOpend}></MenuOpenConn>
        )}

        {opened && kind != DataSourceKind.Mongo && (
          <MenuNewQuery
            handle={() => {
              openNewQuery(setSelectTab, setTabs, uuid, kind, showNotification);
            }}
          ></MenuNewQuery>
        )}
        {opened && <MenuRefresh reload={reload}></MenuRefresh>}
        {opened && !database && (
          <MenuConnectionInfo uuid={uuid}></MenuConnectionInfo>
        )}

        {opened && !database && (
          <MenuCloseConn
            name={uuid}
            handleClose={handleCloseConn}
          ></MenuCloseConn>
        )}
        {database && kind != DataSourceKind.DuckDB && (
          <MenuItem
            onClick={() => {
              // we are in database context uuid contains the databse name
              // we need remove that
              let index = uuid.indexOf('/');
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
              <DeleteForeverIcon color="error"></DeleteForeverIcon>
            </ListItemIcon>
            <ListItemText color="error"> drop </ListItemText>
          </MenuItem>
        )}

        {!database && <MenuDeleteConfig name={uuid}></MenuDeleteConfig>}
      </MenuList>

      {showCreateSchema && (
        <Modal
          open={showCreateSchema}
          onClose={(e: EscDownEvent, reason) => {
            if (reason === 'backdropClick') {
              return;
            }
            e.preventDefault();
            setShowCreateSchema(false);
          }}
        >
          <Box sx={modalStyle} width="50%">
            <CreateSchema
              uuid={uuid}
              handleClose={() => {
                reload();
                setShowCreateSchema(false);
              }}
              kind={kind}
              schemas={schemas}
              handleReload={reload}
            ></CreateSchema>
          </Box>
        </Modal>
      )}
    </Paper>
  );
}

function CatalogMenu(props: {
  uuid;
  reload;
  handleOpend;
  opened;
  handleCloseConn;
  kind: DataSourceKind;
}) {
  const { uuid, reload, handleOpend, opened, handleCloseConn, kind } = props;
  const [showCreateDB, setShowCreateDB] = useState(false);
  return (
    <Paper>
      <MenuList>
        {!opened && (
          <MenuOpenConn name={uuid} handleOpend={handleOpend}></MenuOpenConn>
        )}

        {opened && kind != DataSourceKind.DuckDB && (
          <MenuCreateDatabase
            handleCreate={() => {
              setShowCreateDB(true);
            }}
          ></MenuCreateDatabase>
        )}

        {opened && <MenuRefresh reload={reload}></MenuRefresh>}
        {opened && <MenuConnectionInfo uuid={uuid}></MenuConnectionInfo>}

        {opened && (
          <MenuCloseConn
            name={uuid}
            handleClose={handleCloseConn}
          ></MenuCloseConn>
        )}
        <Divider></Divider>
        <MenuDeleteConfig name={uuid}></MenuDeleteConfig>
      </MenuList>
      {showCreateDB && (
        <Modal
          open={showCreateDB}
          onClose={(e: EscDownEvent, reason) => {
            if (reason === 'backdropClick') {
              return;
            }

            e.preventDefault();
            setShowCreateDB(false);
          }}
        >
          <Box sx={modalStyle} width="50%">
            <CreateDatabase
              uuid={uuid}
              handleClose={() => {
                reload();
                setShowCreateDB(false);
              }}
              kind={kind}
            ></CreateDatabase>
          </Box>
        </Modal>
      )}
    </Paper>
  );
}

async function openNewQuery(
  setSelectTab,
  setTabs,
  uuid: string,
  kind: DataSourceKind,
  showNotification
) {
  initMonacoLanague(uuid, kind);

  let id = uuid + '-query-' + generateRandomString();
  let content;
  let connInfo = await makeQueryConnInfo(uuid);

  switch (kind) {
    case DataSourceKind.MsSql:
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
      content = (
        <SqlQuery
          language={uuid}
          makeConn={async () => {
            return makeConn(uuid, kind);
          }}
          connectInfo={connInfo}
          saveFavoriteSql={async (name, sql) => {
            await FavoriteSql.save(
              new FavoriteSql(uuid, name, kind, sql),
              showNotification
            );
          }}
        ></SqlQuery>
      );
      break;
    case DataSourceKind.Redis:
      content = (
        <RedisQuery
          language={uuid}
          makeConn={async () => {
            return makeConn(uuid, kind);
          }}
          connectInfo={connInfo}
          saveFavoriteSql={async (name, sql) => {
            await FavoriteSql.save(
              new FavoriteSql(uuid, name, kind, sql),
              showNotification
            );
          }}
        ></RedisQuery>
      );

      break;

    case DataSourceKind.Mongo:
      content = (
        <MongoQuery
          uuid={uuid}
          saveFavoriteSql={async (name, sql) => {
            await FavoriteSql.save(
              new FavoriteSql(uuid, name, kind, sql),
              showNotification
            );
          }}
        ></MongoQuery>
      );
      break;
    default:
      console.log(`impl this ${kind}`);
  }

  setTabs((e: TabContent[]) => {
    setSelectTab(e.length);
    return [
      ...e,
      new TabContent(
        id,
        `q@${uuid}`,
        content,
        <TbDatabaseSearch> </TbDatabaseSearch>
      ),
    ];
  });
}
function SchemaMenu(props: {
  schema?: string;
  uuid: string;
  handleOpend;
  opened;
  handleCloseConn;
  kind: DataSourceKind;
  reloadSchemas?;
}) {
  const {
    schema,
    uuid,
    handleOpend,
    opened,
    handleCloseConn,
    reloadSchemas,
    kind,
  } = props;
  const { showNotification } = useContext(NotificationContext);
  const { setSelectTab, setTabs } = useContext(TabsSetContext);
  return (
    <Paper>
      <MenuList>
        {!schema && !opened && (
          <MenuOpenConn name={uuid} handleOpend={handleOpend}></MenuOpenConn>
        )}

        {opened && (
          <MenuNewQuery
            handle={() => {
              openNewQuery(setSelectTab, setTabs, uuid, kind, showNotification);
            }}
          ></MenuNewQuery>
        )}
        {opened && !schema && (
          <MenuConnectionInfo uuid={uuid}></MenuConnectionInfo>
        )}

        {!schema && opened && (
          <MenuCloseConn
            name={uuid}
            handleClose={handleCloseConn}
          ></MenuCloseConn>
        )}
        {schema && kind != DataSourceKind.Oracle && (
          <MenuItem
            onClick={() => {
              // we are in database context uuid contains the databse name
              // we need remove that
              let index = uuid.lastIndexOf('/');
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
              <DeleteForeverIcon color="error"></DeleteForeverIcon>
            </ListItemIcon>
            <ListItemText color="error"> drop </ListItemText>
          </MenuItem>
        )}
        {!schema && <MenuDeleteConfig name={uuid}></MenuDeleteConfig>}
      </MenuList>
    </Paper>
  );
}

function CatalogTree(_props: {}) {
  const { uuid: name } = useContext(UuidContext);
  const { showNotification } = useContext(NotificationContext);
  const [databases, setDatabases] = useState<string[]>([]);
  const [loading, setLoading] = useState(false);
  const { kind } = useContext(ConnTypeContext);
  const handleReload = () => {
    setLoading(true);
    invoke<string[]>('list_database', { name })
      .then((ds) => {
        setDatabases(ds);
      })
      .catch((e) => {
        showNotification('error', 'list database failed', e);
      })
      .finally(() => {
        setLoading(false);
      });
  };
  const { opened, setOpened } = useContext(OpenedContext);
  useEffect(() => {
    if (opened) {
      handleReload();
    }
  }, [opened]);

  let { setUpMenu } = useContext(ContextMenuContext);

  return (
    <TreeItem
      itemId={name}
      label=<Stack
        direction="row"
        spacing={1}
        onContextMenu={(e) => {
          setUpMenu({
            content: (
              <CatalogMenu
                uuid={name}
                reload={handleReload}
                opened={opened}
                handleOpend={() => {
                  setOpened(true);
                }}
                handleCloseConn={() => {
                  setOpened(false);
                  setDatabases([]);
                }}
                kind={kind}
              ></CatalogMenu>
            ),
            style: calMenuStyle(e, 70),
          });
        }}
      >
        <span>{name} </span>
        {loading && <CircularProgress size={progressBarSizeSmall} />}
        {opened && <ConnPingTime uuid={name}></ConnPingTime>}
      </Stack>
      key={name}
    >
      {databases.map((e) => {
        return (
          <UuidContext.Provider key={e} value={{ uuid: name + '/' + e }}>
            <DatabaseTree
              key={e}
              database={e}
              reloadDatabase={handleReload}
            ></DatabaseTree>
          </UuidContext.Provider>
        );
      })}
    </TreeItem>
  );
}

function DatabaseTree(props: {
  database?: string;
  reloadDatabase?: () => void;
}) {
  const { uuid: name } = useContext(UuidContext);
  const { showNotification } = useContext(NotificationContext);
  const [schemas, setSchemas] = useState<string[]>([]);
  const { database, reloadDatabase } = props;
  const [loading, setLoading] = useState(false);
  const { opened, setOpened } = useContext(OpenedContext);
  const { kind } = useContext(ConnTypeContext);
  const handleReload = () => {
    setLoading(true);
    invoke<string[]>('list_schema', { name })
      .then((ds) => {
        setSchemas(ds);
      })
      .catch((e) => {
        showNotification('error', 'list schema failed', e);
      })
      .finally(() => {
        setLoading(false);
      });
  };

  useEffect(() => {
    if (opened) {
      handleReload();
    }
  }, [opened]);
  let { setUpMenu } = useContext(ContextMenuContext);

  return (
    <TreeItem
      itemId={name}
      label=<Stack
        spacing={1}
        direction="row"
        alignContent="baseline"
        onContextMenu={(e) => {
          setUpMenu({
            content: (
              <DatabaseMenu
                opened={opened}
                handleOpend={() => {
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
              ></DatabaseMenu>
            ),
            style: calMenuStyle(e, 70),
          });
        }}
        sx={{ alignItems: 'center' }}
      >
        {database && <FaDatabase />}
        <span>{database ? database : name}</span>
        {loading && <CircularProgress size={progressBarSizeSmall} />}
        {!database && opened && <ConnPingTime uuid={name}></ConnPingTime>}
      </Stack>
      key={name + database}
    >
      {schemas.map((e) => {
        return (
          <UuidContext.Provider key={e} value={{ uuid: name + '/' + e }}>
            <SchemaTree
              key={e}
              schema={e}
              reloadSchemas={handleReload}
            ></SchemaTree>
          </UuidContext.Provider>
        );
      })}
    </TreeItem>
  );
}
const Left = React.memo(Left_);

export { Left };
