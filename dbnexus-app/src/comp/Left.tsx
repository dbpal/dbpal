import AddIcon from '@mui/icons-material/Add';
import { Button, Stack } from '@mui/material';
import { useContext, useEffect, useState } from 'react';
import { TabContent } from './Tabs';
import {
  connTypeToDataSourceKind,
  tabsHasId,
  TabsSetContext,
  UuidContext,
  DataSourceKind,
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
      <Stack direction="row">
        <Button
          color="primary"
          variant="contained"
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
              value={{ conn_type: connTypeToDataSourceKind(e.conn_type) }}
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
  return (
    <SimpleTreeView>
      {config.conn_type == 'mariadb' &&
        (config.db ? <SchemaTree></SchemaTree> : <DatabaseTree></DatabaseTree>)}

      {config.conn_type == 'postgres' &&
        (config.schema ? <SchemaTree /> : <DatabaseTree />)}
    </SimpleTreeView>
  );
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
