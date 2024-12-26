import { Box, MenuList, Modal, Paper, Stack, Typography } from '@mui/material';
import { useContext, useEffect, useState } from 'react';

import {
  connTypeToDataSourceKind,
  TabsSetContext,
  UuidContext,
  ConnTypeContext,
  MenuNewQuery,
  MenuDeleteConfig,
  MenuContext,
  calMenuStyle,
  modalStyle,
} from './Common';
import { LuDatabase } from 'react-icons/lu';
import { FaDatabase } from 'react-icons/fa';
import AddConfig from './AddConnection';
import types from './ApiStruct';
import { invoke } from '@tauri-apps/api/core';
import { NotificationContext } from './Notification';
import { SimpleTreeView } from '@mui/x-tree-view/SimpleTreeView';
import { TreeItem } from '@mui/x-tree-view/TreeItem';
import { listen } from '@tauri-apps/api/event';

import { TablesTree } from './LeftTable';
import { ViewsTree } from './LeftView';
import { ProceduresTree } from './LeftProcedure';
import { UdfsTree } from './LeftUdf';

function Left() {
  const [configs, setConfigs] = useState<types.DBConnectionConfig[]>([]);
  const { showNotification } = useContext(NotificationContext);
  const [showAddConnection, setShowAddConnection] = useState(false);
  const handleReload = () => {
    invoke<types.DBConnectionConfig[]>('list_all_config')
      .then((e) => {
        setConfigs(e);
      })
      .catch((e) => {
        showNotification('error', 'list all config failure', e);
      });
  };

  useEffect(() => {
    handleReload();
  }, []);
  listen('connection_config_changed', () => {
    handleReload();
  });

  listen('add_connection', () => {
    setShowAddConnection(true);
  });

  return (
    <Stack
      style={{
        overflow: 'auto',
      }}
    >
      {showAddConnection && (
        <Modal
          open={showAddConnection}
          onClose={() => {
            setShowAddConnection(false);
          }}
          aria-labelledby='add column'
        >
          <Box sx={modalStyle} width='600px'>
            <AddConfig></AddConfig>
          </Box>
        </Modal>
      )}

      <Stack>
        {configs.map((e) => {
          return (
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
        return config.db ? (
          <SchemaTree></SchemaTree>
        ) : (
          <DatabaseTree></DatabaseTree>
        );
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
  let { setUpMenu } = useContext(MenuContext);
  return (
    <TreeItem
      itemId={getPrefix()}
      label=<Stack
        direction='row'
        onContextMenu={(e) => {
          setUpMenu({
            content: <SchemaMenu schema={schema} uuid={uuid}></SchemaMenu>,
            style: calMenuStyle(e, 70),
          });
        }}
      >
        {schema && <LuDatabase></LuDatabase>}
        <span> {schema ? schema : uuid}</span>
      </Stack>
    >
      <TablesTree></TablesTree>
      <ViewsTree></ViewsTree>
      <ProceduresTree></ProceduresTree>
      <UdfsTree></UdfsTree>
    </TreeItem>
  );
}

function DatabaseMenu(props: { database?: string; uuid }) {
  const { database, uuid } = props;
  return (
    <Paper>
      <MenuList>
        <MenuNewQuery
          handle={() => {
            alert('new query');
          }}
        ></MenuNewQuery>
      </MenuList>
      {!database && <MenuDeleteConfig name={uuid}></MenuDeleteConfig>}
    </Paper>
  );
}

function CatalogMenu(props: { uuid }) {
  const { database, uuid } = props;
  return (
    <Paper>
      <MenuList>
        <MenuNewQuery
          handle={() => {
            alert('new query');
          }}
        ></MenuNewQuery>
      </MenuList>
      <MenuDeleteConfig name={uuid}></MenuDeleteConfig>
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

  const handleReload = () => {
    invoke<string[]>('list_database', { name })
      .then((ds) => {
        setDatabases(ds);
      })
      .catch((e) => {
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
        onContextMenu={(e) => {
          setUpMenu({
            content: <CatalogMenu uuid={name}></CatalogMenu>,
            style: calMenuStyle(e, 70),
          });
        }}
      >
        <Typography>{name} </Typography>
      </Stack>
      key={name}
    >
      {databases.map((e) => {
        return (
          <UuidContext.Provider value={{ uuid: name + ':' + e }}>
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
  const handleReload = () => {
    invoke<string[]>('list_schema', { name })
      .then((ds) => {
        setSchemas(ds);
      })
      .catch((e) => {
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
        direction='row'
        alignContent='baseline'
        onContextMenu={(e) => {
          setUpMenu({
            content: (
              <DatabaseMenu database={database} uuid={name}></DatabaseMenu>
            ),
            style: calMenuStyle(e, 70),
          });
        }}
      >
        <FaDatabase />
        <span>{database ? database : name}</span>
      </Stack>
      key={name + database}
    >
      {schemas.map((e) => {
        return (
          <UuidContext.Provider value={{ uuid: name + ':' + e }}>
            <SchemaTree key={e} schema={e}></SchemaTree>
          </UuidContext.Provider>
        );
      })}
    </TreeItem>
  );
}

export { Left };
