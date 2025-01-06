import {
  CircularProgress,
  ListItemIcon,
  ListItemText,
  MenuItem,
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
import RefreshIcon from '@mui/icons-material/Refresh';

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

  return <SimpleTreeView>{makeTree()}</SimpleTreeView>;
}

function SchemaTree(prpos: { schema?: string }) {
  const { uuid } = useContext(UuidContext);
  let { schema } = prpos;
  const getPrefix = () => {
    return uuid + schema;
  };
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
            content: <SchemaMenu schema={schema} uuid={uuid}></SchemaMenu>,
            style: calMenuStyle(e, 70),
          });
        }}
        sx={{ alignItems: 'center' }}
      >
        {schema && <LuDatabase></LuDatabase>}
        <span> {schema ? schema : uuid}</span>
        {!schema && <ConnStatus uuid={uuid}></ConnStatus>}
      </Stack>
    >
      <TablesTree></TablesTree>
      <ViewsTree viewkind={ViewKind.Normal}></ViewsTree>
      {kind == DataSourceKind.Postgres && (
        <ViewsTree viewkind={ViewKind.PostGresMaterrialized}></ViewsTree>
      )}
      {support_procedure && <ProceduresTree></ProceduresTree>}
      {support_udf && <UdfsTree></UdfsTree>}
    </TreeItem>
  );
}

function DatabaseMenu(props: { database?: string; uuid; reload }) {
  const { database, uuid, reload } = props;
  return (
    <Paper>
      <MenuList>
        <MenuNewQuery
          handle={() => {
            alert('new query');
          }}
        ></MenuNewQuery>
      </MenuList>
      <MenuItem
        onClick={() => {
          reload();
        }}
      >
        <ListItemIcon>
          <RefreshIcon></RefreshIcon>
        </ListItemIcon>
        <ListItemText> refresh </ListItemText>
      </MenuItem>

      {!database && <MenuDeleteConfig name={uuid}></MenuDeleteConfig>}
    </Paper>
  );
}

function CatalogMenu(props: { uuid; reload }) {
  const { uuid, reload } = props;
  return (
    <Paper>
      <MenuList>
        <MenuNewQuery
          handle={() => {
            alert('new query');
          }}
        ></MenuNewQuery>
        <MenuItem
          onClick={() => {
            reload();
          }}
        >
          <ListItemIcon>
            <RefreshIcon></RefreshIcon>
          </ListItemIcon>
          <ListItemText> refresh </ListItemText>
        </MenuItem>
        <MenuDeleteConfig name={uuid}></MenuDeleteConfig>
      </MenuList>
    </Paper>
  );
}

function SchemaMenu(props: { schema?: string; uuid }) {
  const { schema, uuid } = props;
  return (
    <Paper>
      <MenuList>
        <MenuNewQuery
          handle={() => {
            alert('new query');
          }}
        ></MenuNewQuery>
      </MenuList>
      {!schema && <MenuDeleteConfig name={uuid}></MenuDeleteConfig>}
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
  useEffect(() => {
    handleReload();
  }, []);
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
              <CatalogMenu uuid={name} reload={handleReload}></CatalogMenu>
            ),
            style: calMenuStyle(e, 70),
          });
        }}
      >
        <Typography>{name} </Typography>
        {loading && <CircularProgress size={progressBarSizeSmall} />}
        <ConnStatus uuid={name}></ConnStatus>
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
    handleReload();
  }, []);
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
        {!database && <ConnStatus uuid={name}></ConnStatus>}
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
