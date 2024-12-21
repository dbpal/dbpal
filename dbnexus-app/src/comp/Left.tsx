import AddIcon from '@mui/icons-material/Add';
import { Button, Stack } from '@mui/material';
import { useContext, useEffect, useState } from 'react';
import { TabContent } from './Tabs';
import {
  connTypeToDataSourceKind,
  tabsHasId,
  TabsSetContext,
  NameAndTypeContext,
  DataSourceKind,
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
            <NameAndTypeContext.Provider
              value={{
                uuid: e.name,
                conn_type: connTypeToDataSourceKind(e.conn_type),
              }}
            >
              <OneConfig key={e.name} config={e}></OneConfig>
            </NameAndTypeContext.Provider>
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
        (config.db ? (
          <SchemaTree isTop={true}></SchemaTree>
        ) : (
          <DatabaseTree isTop={true}></DatabaseTree>
        ))}
    </SimpleTreeView>
  );
}
function SchemaTree(prpos: { schemaname?: string; isTop: boolean }) {
  const { uuid, database, conn_type } = useContext(NameAndTypeContext);
  let { schemaname, isTop } = prpos;
  const getPrefix = () => {
    return uuid + database + schemaname;
  };
  const newUuid = () => {
    switch (conn_type) {
      case DataSourceKind.MariaDB:
        if (schemaname) {
          return uuid + ':' + schemaname;
        } else {
          return uuid;
        }
      default:
        throw 'impl this.';
    }
  };

  return (
    <TreeItem itemId={getPrefix()} label={isTop ? uuid : schemaname}>
      <NameAndTypeContext.Provider
        value={{ uuid: newUuid(), conn_type, database }}
      >
        <TablesTree></TablesTree>
        <ViewsTree></ViewsTree>
      </NameAndTypeContext.Provider>
    </TreeItem>
  );
}

function DatabaseTree(props: { isTop: boolean }) {
  const { uuid: name, database } = useContext(NameAndTypeContext);
  const { showNotification } = useContext(NotificationContext);
  const [databases, setDatabases] = useState<string[]>([]);
  let { isTop } = props;
  const handleReload = () => {
    invoke<string[]>('list_schema', { name })
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
    <>
      <TreeItem itemId={name + database} label={isTop ? name : database}>
        {databases.map((e) => {
          return <SchemaTree key={e} schemaname={e} isTop={false}></SchemaTree>;
        })}
      </TreeItem>
    </>
  );
}

export { Left };
