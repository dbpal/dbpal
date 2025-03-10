import {
  Box,
  Button,
  CircularProgress,
  IconButton,
  ListItemIcon,
  ListItemText,
  MenuItem,
  MenuList,
  Modal,
  Paper,
  Stack,
  Tooltip,
} from '@mui/material';
import { useCallback, useContext, useEffect, useMemo, useState } from 'react';
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
  treeItemCommonStyle,
  SettingContext,
} from './Common';
import { LuDatabase } from 'react-icons/lu';
import { FaDatabase } from 'react-icons/fa';
import { PiFileSqlFill } from 'react-icons/pi';
import types from './ApiStruct';
import { invoke } from '@tauri-apps/api/core';
import { NotificationContext } from './Notification';
import { SimpleTreeView } from '@mui/x-tree-view/SimpleTreeView';
import { TreeItem } from '@mui/x-tree-view/TreeItem';
import DeleteForeverIcon from '@mui/icons-material/DeleteForever';
import { debug } from './debug';
import { TabContent } from './Tabs';
import {
  calMenuStyle,
  columnWidths,
  ConnTypeContext,
  connTypeToDataSourceKind,
  ContextMenuContext,
  datagridContainerHeight,
  DataSourceKind,
  FavoriteSql,
  generateRandomString,
  handleCopyToClipboard,
  TabsSetContext,
  UuidContext,
} from './helper';

import { SqlQuery } from './SqlQuery';
import { RedisQuery } from './RedisQuery';
import { MongoQuery } from './MongoQuery.tsx';
import React from 'react';
import {
  ConnStatus,
  MenuCloseConn,
  MenuCreateDatabase,
  MenuCreateSchema,
  MenuDeleteConfig,
  MenuNewQuery,
  MenuOpenConn,
  MenuRefresh,
} from './CommonComp.tsx';
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
import { AddConfig } from './AddConnection.tsx';

function Left_() {
  const [configs, setConfigs] = useState<types.DBConnectionConfig[]>([]);
  const { showNotification } = useContext(NotificationContext);
  const [showFavoriteSql, setShowFavoriteSql] = useState(false);
  const [showAddConnection, setShowAddConnection] = useState(false);
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

  return (
    <Stack
      style={{
        overflow: 'auto',
      }}
    >
      <Stack
        direction="row"
        style={{ justifyContent: 'flex-start', flexWrap: 'wrap' }}
      >
        <Tooltip title="make a connection">
          <IconButton
            onClick={() => {
              setShowAddConnection(true);
            }}
          >
            <GrConnect style={{ fontSize: '2.2rem' }}> </GrConnect>
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
      </Stack>

      <Stack>
        {configs.map((e) => {
          return (
            <ConnTypeContext.Provider
              value={{ kind: connTypeToDataSourceKind(e.conn_type) }}
              key={e.name}
            >
              <UuidContext.Provider
                value={{
                  uuid: e.name,
                }}
              >
                <OneConfig key={e.name} config={e}></OneConfig>
              </UuidContext.Provider>
            </ConnTypeContext.Provider>
          );
        })}
      </Stack>

      {showFavoriteSql && (
        <Modal
          open={showFavoriteSql}
          onClose={(_, reason) => {
            if (reason === 'backdropClick') {
              return;
            }
            setShowFavoriteSql(false);
          }}
        >
          <Box sx={modalStyle} width="950px">
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
          onClose={(_, reason) => {
            if (reason === 'backdropClick') {
              return;
            }
            setShowAddConnection(false);
          }}
        >
          <Box sx={modalStyle} width="950px">
            <AddConfig
              handleClose={() => {
                setShowAddConnection(false);
              }}
            ></AddConfig>
          </Box>
        </Modal>
      )}
      <div style={{ minHeight: '300px' }}></div>
    </Stack>
  );
}

function FavoriteSqlList(props: { handleClose }) {
  const { handleClose } = props;
  const { showNotification } = useContext(NotificationContext);
  const [sqls, setSqls] = useState<FavoriteSql[]>([]);
  const [sortModel, setSortModel] = useState<GridSortModel>([
    { field: 'createTime', sort: 'desc' },
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

  const onSortModelChange = useCallback((e) => {
    setSortModel(e);
  }, []);

  useEffect(() => {
    handleReload(false);
  }, []);
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
      { field: 'createTime' },
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

  return (
    <Stack spacing={1}>
      <Stack direction="row" spacing={1}>
        <Button
          variant="contained"
          onClick={() => {
            handleReload(true);
          }}
        >
          reload
        </Button>
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
        ></DataGrid>
      </div>
    </Stack>
  );
}

function OneConfig(props: { config: types.DBConnectionConfig }) {
  let { config } = props;
  const [opened, setOpened] = useState(false || debug);
  const makeTree = () => {
    switch (config.conn_type) {
      case 'postgres':
        if (!config.db) {
          return <CatalogTree />;
        }
        return config.schema ? <SchemaTree /> : <DatabaseTree />;
      case 'mariadb':
      case 'mysql':
      case 'tidb':
        return config.db ? (
          <SchemaTree></SchemaTree>
        ) : (
          <DatabaseTree></DatabaseTree>
        );
      case 'sqlite':
      case 'libsql':
        return <SchemaTree></SchemaTree>;
      case 'redis':
        return config.db ? (
          <SchemaTree></SchemaTree>
        ) : (
          <DatabaseTree></DatabaseTree>
        );
      case 'mongo':
        return config.db ? (
          <SchemaTree></SchemaTree>
        ) : (
          <DatabaseTree></DatabaseTree>
        );
      case 'sqlserver':
        if (!config.db) {
          return <CatalogTree />;
        }
        return config.schema ? <SchemaTree /> : <DatabaseTree />;
      case 'oracle':
        return config.schema ? <SchemaTree /> : <DatabaseTree />;
      case 'cassandra':
        return config.db ? <SchemaTree /> : <DatabaseTree />;
      case 'clickhouse':
        return config.db ? <SchemaTree /> : <DatabaseTree />;

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
    kind == DataSourceKind.Oracle;
  let support_udf =
    kind == DataSourceKind.MariaDB ||
    kind == DataSourceKind.Mysql ||
    kind == DataSourceKind.Postgres ||
    kind == DataSourceKind.MsSql ||
    kind == DataSourceKind.Oracle ||
    kind == DataSourceKind.Cassandra;

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
        sx={{ alignItems: 'center', ...treeItemCommonStyle }}
      >
        {schema && <LuDatabase></LuDatabase>}
        <span> {schema ? schema : uuid}</span>
        {!schema && opened && <ConnStatus uuid={uuid}></ConnStatus>}
      </Stack>
    >
      {opened && kind == DataSourceKind.Sqlite && (
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
        {database && (
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
          onClose={(_, reason) => {
            if (reason === 'backdropClick') {
              return;
            }
            setShowCreateSchema(false);
          }}
        >
          <Box sx={modalStyle} width="600px">
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
  //@ts-ignore
  const { uuid, reload, handleOpend, opened, handleCloseConn, kind } = props;

  const [showCreateDB, setShowCreateDB] = useState(false);
  // const { showNotification } = useContext(NotificationContext);
  // const { setSelectTab, setTabs } = useContext(TabsSetContext);
  return (
    <Paper>
      <MenuList>
        {!opened && (
          <MenuOpenConn name={uuid} handleOpend={handleOpend}></MenuOpenConn>
        )}

        {opened && (
          <MenuCreateDatabase
            handleCreate={() => {
              setShowCreateDB(true);
            }}
          ></MenuCreateDatabase>
        )}

        {/* {opened && (
          <MenuNewQuery
            handle={() => {
              openNewQuery(
                setSelectTab,
                setTabs,
                uuid,
                kind,
                showNotification
              );
            }}
          ></MenuNewQuery>
        )} */}

        {opened && <MenuRefresh reload={reload}></MenuRefresh>}
        {opened && <MenuConnectionInfo uuid={uuid}></MenuConnectionInfo>}

        {opened && (
          <MenuCloseConn
            name={uuid}
            handleClose={handleCloseConn}
          ></MenuCloseConn>
        )}

        <MenuDeleteConfig name={uuid}></MenuDeleteConfig>
      </MenuList>
      {showCreateDB && (
        <Modal
          open={showCreateDB}
          onClose={(_, reason) => {
            if (reason === 'backdropClick') {
              return;
            }
            setShowCreateDB(false);
          }}
        >
          <Box sx={modalStyle} width="600px">
            <CreateDatabase
              uuid={uuid}
              handleClose={() => {
                reload();
                setShowCreateDB(false);
              }}
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
  const makeConn = async () => {
    const queryId = await invoke<string>('query_sql_open_connection', {
      name: uuid,
    });
    console.log('open connection:', queryId);
    return new QueryConn(uuid, queryId, kind == DataSourceKind.Oracle);
  };

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
      content = (
        <SqlQuery
          language={uuid}
          makeConn={makeConn}
          connectInfo={connInfo}
          saveFavoriteSql={async (name, sql) => {
            await FavoriteSql.save(
              new FavoriteSql(name, uuid, kind, sql),
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
          makeConn={makeConn}
          connectInfo={connInfo}
          saveFavoriteSql={async (name, sql) => {
            await FavoriteSql.save(
              new FavoriteSql(name, uuid, kind, sql),
              showNotification
            );
          }}
        ></RedisQuery>
      );

      break;

    case DataSourceKind.Mongo:
      content = <MongoQuery uuid={uuid}></MongoQuery>;
      break;
    default:
      throw `impl this ${kind}`;
  }

  setTabs((e: TabContent[]) => {
    setSelectTab(e.length);
    return [...e, new TabContent(id, `q@${uuid}`, content)];
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
        style={{ ...treeItemCommonStyle }}
      >
        <span>{name} </span>
        {loading && <CircularProgress size={progressBarSizeSmall} />}
        {opened && <ConnStatus uuid={name}></ConnStatus>}
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
        sx={{ alignItems: 'center', ...treeItemCommonStyle }}
      >
        {database && <FaDatabase />}
        <span>{database ? database : name}</span>
        {loading && <CircularProgress size={progressBarSizeSmall} />}
        {!database && opened && <ConnStatus uuid={name}></ConnStatus>}
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
