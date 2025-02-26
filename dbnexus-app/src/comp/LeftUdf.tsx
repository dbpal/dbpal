import {
  ListItemIcon,
  ListItemText,
  MenuItem,
  MenuList,
  Paper,
  Stack,
} from '@mui/material';
import { TreeItem } from '@mui/x-tree-view/TreeItem';
import { useContext, useEffect, useState } from 'react';
import DeleteForeverIcon from '@mui/icons-material/DeleteForever';
import InfoIcon from '@mui/icons-material/Info';

import { initMonacoLanague, tabsHasId } from './Common';
import types from './ApiStruct';
import { invoke } from '@tauri-apps/api/core';
import { NotificationContext } from './Notification';
import { TabContent } from './Tabs';
import { DefAndMetasView } from './UdfAndMetaView';

import { TbMathFunction } from 'react-icons/tb';
import {
  calMenuStyle,
  ConnTypeContext,
  ContextMenuContext,
  DataSourceKind,
  secondConfirm,
  TabsSetContext,
  UuidContext,
} from './helper';

import { MenuRefresh } from './CommonComp';
import { join } from 'path';
function UdfsTree() {
  const { uuid } = useContext(UuidContext);
  const { kind } = useContext(ConnTypeContext);
  const idPrefix = uuid + '-udf-';
  const [udfs, setUdfs] = useState<types.NamedStaff[]>([]);
  const { showNotification } = useContext(NotificationContext);
  const reload = () => {
    invoke<types.NamedStaff[]>('list_udf', { name: uuid })
      .then((ts) => {
        setUdfs(ts);
      })
      .catch((e) => {
        showNotification('error', 'list udf failed', e);
      });
  };
  const getItemId = (named: types.NamedStaff) => {
    switch (kind) {
      case DataSourceKind.Postgres:
        return idPrefix + '-' + named.name + '-' + named.parameters;
      case DataSourceKind.Cassandra:
        return (
          idPrefix +
          '-' +
          named.name +
          '-' +
          named.cassandra_parameters.join('-')
        );
      default:
        return idPrefix + '-' + named.name;
    }
  };

  let { setUpMenu } = useContext(ContextMenuContext);
  useEffect(() => {
    reload();
  }, []);
  return (
    <TreeItem
      itemId={idPrefix}
      label=<Stack
        spacing={1}
        direction="row"
        sx={{ alignItems: 'center' }}
        onContextMenu={(e) => {
          setUpMenu({
            content: (
              <Paper>
                <MenuList>
                  <MenuRefresh reload={reload}></MenuRefresh>
                </MenuList>
              </Paper>
            ),
            style: calMenuStyle(e, 70),
          });
        }}
      >
        <TbMathFunction></TbMathFunction>
        <span> udfs </span>
      </Stack>
    >
      {udfs.map((p) => (
        <TreeItem
          key={
            p.name +
            '-' +
            p.parameters +
            (p.cassandra_parameters ? p.cassandra_parameters.join('-') : '')
          }
          itemId={getItemId(p)}
          label=<Stack
            direction="row"
            spacing={1}
            sx={{ alignItems: 'center' }}
            onContextMenu={(e) => {
              setUpMenu({
                content: (
                  <UdfMenu
                    reload={reload}
                    uuid={uuid}
                    kind={kind}
                    udf={p.name}
                    parameters={p.parameters}
                    cassandraParameters={p.cassandra_parameters}
                  ></UdfMenu>
                ),
                style: calMenuStyle(e, 100),
              });
            }}
          >
            <TbMathFunction></TbMathFunction>
            <span>{p.name}</span>
            {kind == DataSourceKind.Postgres && <em> ({p.parameters}) </em>}
            {kind == DataSourceKind.Cassandra && (
              <em> ({p.cassandra_parameters.join(',')}) </em>
            )}
          </Stack>
        ></TreeItem>
      ))}
    </TreeItem>
  );
}

function UdfMenu(props: {
  uuid;
  udf;
  parameters?: string;
  cassandraParameters: string[] | null;
  reload;
  kind: DataSourceKind;
}) {
  const { uuid, udf, parameters, reload, kind, cassandraParameters } = props;

  const { setSelectIndex, setTabs, tabs } = useContext(TabsSetContext);
  let arg = null;
  switch (kind) {
    case DataSourceKind.Postgres:
      arg = { extra: 'postgres', parameters };
      break;

    case DataSourceKind.Cassandra:
      arg = { extra: 'cassandra', parameters: cassandraParameters };
      break;
  }

  const { showNotification } = useContext(NotificationContext);

  const handleOpenDetails = () => {
    initMonacoLanague(uuid, kind);
    const id = uuid + '-' + '-udf-' + udf;
    let index = tabsHasId(tabs, id);
    if (typeof index == 'number') {
      setSelectIndex(index);
    } else {
      setTabs((e) => {
        setSelectIndex(tabs.length);
        return [
          ...e,
          new TabContent(
            id,
            udf,
            (
              <DefAndMetasView
                reloadOP={async () => {
                  return await invoke<types.DefAndMetas>('show_udf', {
                    name: uuid,
                    udf,
                    arg,
                  });
                }}
                language={uuid}
              ></DefAndMetasView>
            )
          ),
        ];
      });
    }
  };

  const handleDrop = async () => {
    let confirm = await secondConfirm({
      title: `Drop Procedure '${udf}'`,
      confirmText: 'drop',
    });
    if (!confirm) {
      return;
    }

    invoke('drop_udf', { name: uuid, udf, arg })
      .then(() => {
        showNotification('success', 'Drop udf OK');
        reload();
      })
      .catch((e) => {
        showNotification('error', 'Drop udf Failure', e);
      });
  };

  return (
    <Paper>
      <MenuList>
        <MenuItem
          onClick={() => {
            handleOpenDetails();
          }}
        >
          <ListItemIcon>
            <InfoIcon></InfoIcon>
          </ListItemIcon>
          <ListItemText> detail </ListItemText>
        </MenuItem>

        <MenuItem
          sx={{ color: 'error.main' }}
          onClick={() => {
            handleDrop();
          }}
        >
          <ListItemIcon>
            <DeleteForeverIcon color="error"></DeleteForeverIcon>
          </ListItemIcon>
          <ListItemText>drop</ListItemText>
        </MenuItem>
      </MenuList>
    </Paper>
  );
}

export { UdfsTree };
