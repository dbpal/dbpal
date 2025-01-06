import {
  CircularProgress,
  MenuList,
  Paper,
  Stack,
  Typography,
} from '@mui/material';
import { useContext, useEffect, useState } from 'react';

import {
  connTypeToDataSourceKind,
  UuidContext,
  ConnTypeContext,
  MenuNewQuery,
  MenuDeleteConfig,
  MenuContext,
  calMenuStyle,
  ConnStatus,
  progressBarSizeSmall,
  DataSourceKind,
  appStore,
  AppStoreKey,
  MenuOpenConn,
  OpenedContext,
  MenuCloseConn,
  MenuRefresh,
  MenuConnectionInfo,
} from './Common';
import { LuDatabase } from 'react-icons/lu';
import { FaDatabase } from 'react-icons/fa';

import types from './ApiStruct';
import { invoke } from '@tauri-apps/api/core';
import { NotificationContext } from './Notification';
import { SimpleTreeView } from '@mui/x-tree-view/SimpleTreeView';
import { TreeItem } from '@mui/x-tree-view/TreeItem';

import { TablesTree } from './LeftTable';
import { ViewKind, ViewsTree } from './LeftView';
import { ProceduresTree } from './LeftProcedure';
import { UdfsTree } from './LeftUdf';

function Left() {
  const [configs, setConfigs] = useState<types.DBConnectionConfig[]>([]);
  const { showNotification } = useContext(NotificationContext);

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
      <div style={{ minHeight: '300px' }}></div>
    </Stack>
  );
}

function OneConfig(props: { config: types.DBConnectionConfig }) {
  let { config } = props;
  const [opened, setOpened] = useState(false);
  const makeTree = () => {
    switch (config.conn_type) {
      case 'postgres':
        if (!config.db) {
          return <CatalogTree />;
        }
        return config.schema ? <SchemaTree /> : <DatabaseTree />;
      case 'mariadb':
      case 'mysql':
        return config.db ? (
          <SchemaTree></SchemaTree>
        ) : (
          <DatabaseTree></DatabaseTree>
        );
      case 'sqlite':
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
    }
  };

  return (
    <OpenedContext.Provider value={{ opened, setOpened }}>
      <SimpleTreeView disableSelection>{makeTree()}</SimpleTreeView>
    </OpenedContext.Provider>
  );
}

function SchemaTree(prpos: { schema?: string }) {
  const { uuid } = useContext(UuidContext);
  let { schema } = prpos;
  const getPrefix = () => {
    return uuid + schema;
  };
  const { opened, setOpened } = useContext(OpenedContext);
  const { kind } = useContext(ConnTypeContext);
  let support_procedure =
    kind == DataSourceKind.MariaDB ||
    kind == DataSourceKind.Mysql ||
    kind == DataSourceKind.Postgres ||
    kind == DataSourceKind.MsSql;
  let support_udf =
    kind == DataSourceKind.MariaDB ||
    kind == DataSourceKind.Mysql ||
    kind == DataSourceKind.Postgres ||
    kind == DataSourceKind.MsSql;
  let { setUpMenu } = useContext(MenuContext);

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
              ></SchemaMenu>
            ),
            style: calMenuStyle(e, 70),
          });
        }}
        sx={{ alignItems: 'center' }}
      >
        {schema && <LuDatabase></LuDatabase>}
        <span> {schema ? schema : uuid}</span>
        {!schema && opened && <ConnStatus uuid={uuid}></ConnStatus>}
      </Stack>
    >
      {opened && (
        <>
          <TablesTree></TablesTree>
          <ViewsTree viewkind={ViewKind.Normal}></ViewsTree>
          {kind == DataSourceKind.Postgres && (
            <ViewsTree viewkind={ViewKind.PostGresMaterrialized}></ViewsTree>
          )}
          {support_procedure && <ProceduresTree></ProceduresTree>}
          {support_udf && <UdfsTree></UdfsTree>}
        </>
      )}
    </TreeItem>
  );
}

function DatabaseMenu(props: {
  database?: string;
  uuid;
  reload;
  handleOpend;
  opened;
  handleCloseConn;
}) {
  const { database, uuid, reload, handleOpend, opened, handleCloseConn } =
    props;

  return (
    <Paper>
      <MenuList>
        {!opened && !database && (
          <MenuOpenConn name={uuid} handleOpend={handleOpend}></MenuOpenConn>
        )}

        {opened && (
          <MenuNewQuery
            handle={() => {
              alert('new query');
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
        {!database && <MenuDeleteConfig name={uuid}></MenuDeleteConfig>}
      </MenuList>
    </Paper>
  );
}

function CatalogMenu(props: {
  uuid;
  reload;
  handleOpend;
  opened;
  handleCloseConn;
}) {
  const { uuid, reload, handleOpend, opened, handleCloseConn } = props;
  return (
    <Paper>
      <MenuList>
        {!opened && (
          <MenuOpenConn name={uuid} handleOpend={handleOpend}></MenuOpenConn>
        )}

        {opened && (
          <MenuNewQuery
            handle={() => {
              alert('new query');
            }}
          ></MenuNewQuery>
        )}

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
    </Paper>
  );
}

function SchemaMenu(props: {
  schema?: string;
  uuid;
  handleOpend;
  opened;
  handleCloseConn;
}) {
  const { schema, uuid, handleOpend, opened, handleCloseConn } = props;
  return (
    <Paper>
      <MenuList>
        {!schema && !opened && (
          <MenuOpenConn name={uuid} handleOpend={handleOpend}></MenuOpenConn>
        )}

        {opened && (
          <MenuNewQuery
            handle={() => {
              alert('new query');
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

        {!schema && <MenuDeleteConfig name={uuid}></MenuDeleteConfig>}
      </MenuList>
    </Paper>
  );
}

function CatalogTree(props: {}) {
  const { uuid: name } = useContext(UuidContext);
  const { showNotification } = useContext(NotificationContext);
  const [databases, setDatabases] = useState<string[]>([]);
  const [loading, setLoading] = useState(false);

  const handleReload = () => {
    setLoading(true);
    invoke<string[]>('list_database', { name })
      .then((ds) => {
        setLoading(false);
        setDatabases(ds);
      })
      .catch((e) => {
        setLoading(false);
        showNotification('error', 'list database failed', e);
      });
  };
  const { opened, setOpened } = useContext(OpenedContext);
  useEffect(() => {
    if (opened) {
      handleReload();
    }
  }, [opened]);

  let { setUpMenu } = useContext(MenuContext);

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
              ></CatalogMenu>
            ),
            style: calMenuStyle(e, 70),
          });
        }}
      >
        <Typography>{name} </Typography>
        {loading && <CircularProgress size={progressBarSizeSmall} />}
        {opened && <ConnStatus uuid={name}></ConnStatus>}
      </Stack>
      key={name}
    >
      {databases.map((e) => {
        return (
          <UuidContext.Provider key={e} value={{ uuid: name + ':' + e }}>
            <DatabaseTree key={e} database={e}></DatabaseTree>
          </UuidContext.Provider>
        );
      })}
    </TreeItem>
  );
}

function DatabaseTree(props: { database?: string }) {
  const { uuid: name } = useContext(UuidContext);
  const { showNotification } = useContext(NotificationContext);
  const [schemas, setSchemas] = useState<string[]>([]);
  let { database } = props;
  const [loading, setLoading] = useState(false);
  const { opened, setOpened } = useContext(OpenedContext);

  const handleReload = () => {
    setLoading(true);
    invoke<string[]>('list_schema', { name })
      .then((ds) => {
        setLoading(false);
        setSchemas(ds);
      })
      .catch((e) => {
        setLoading(false);
        showNotification('error', 'list schema failed', e);
      });
  };

  useEffect(() => {
    if (opened) {
      handleReload();
    }
  }, [opened]);
  let { setUpMenu } = useContext(MenuContext);

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
        {!database && opened && <ConnStatus uuid={name}></ConnStatus>}
      </Stack>
      key={name + database}
    >
      {schemas.map((e) => {
        return (
          <UuidContext.Provider key={e} value={{ uuid: name + ':' + e }}>
            <SchemaTree key={e} schema={e}></SchemaTree>
          </UuidContext.Provider>
        );
      })}
    </TreeItem>
  );
}

export { Left };
