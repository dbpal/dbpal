import AddIcon from '@mui/icons-material/Add';
import { Button, Stack } from '@mui/material';
import { useContext, useEffect, useState } from 'react';
import { TabContent } from './Tabs';
import {
  connTypeToDataSourceKind,
  tabsHasId,
  TabsSetContext,
  UuidContext,
  ConnTypeContext,
} from './Common';
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

function Left() {
  let { setTabs, tabs, setSelectIndex } = useContext(TabsSetContext);
  const [configs, setConfigs] = useState<types.DBConnectionConfig[]>([]);
  const { showNotification } = useContext(NotificationContext);
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
  listen('config_added', () => {
    handleReload();
  });

  return (
    <Stack
      style={{
        overflow: 'auto',
      }}
    >
      <Stack direction='row'>
        <Button
          color='primary'
          variant='contained'
          onClick={() => {
            const id = 'add_config';
            let index = tabsHasId(tabs, 'add_config');
            if (typeof index != 'number') {
              setTabs((e) => {
                setSelectIndex(tabs.length);
                return [
                  ...e,
                  new TabContent(id, 'add config', <AddConfig></AddConfig>),
                ];
              });
            } else {
              setSelectIndex(index);
            }
          }}
        >
          <AddIcon></AddIcon>
        </Button>
      </Stack>
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

function SchemaTree(prpos: { schemaname?: string }) {
  const { uuid } = useContext(UuidContext);
  let { schemaname } = prpos;
  const getPrefix = () => {
    return uuid + schemaname;
  };
  return (
    <TreeItem itemId={getPrefix()} label={schemaname ? schemaname : uuid}>
      <TablesTree></TablesTree>
      <ViewsTree></ViewsTree>
      <ProceduresTree></ProceduresTree>
    </TreeItem>
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
  return (
    <TreeItem itemId={name} label={name} key={name}>
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
  return (
    <TreeItem
      itemId={name}
      label={database ? database : name}
      key={name + database}
    >
      {schemas.map((e) => {
        return (
          <UuidContext.Provider value={{ uuid: name + ':' + e }}>
            <SchemaTree key={e} schemaname={e}></SchemaTree>
          </UuidContext.Provider>
        );
      })}
    </TreeItem>
  );
}

export { Left };
