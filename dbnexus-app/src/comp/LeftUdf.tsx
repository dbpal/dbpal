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

import {
  ConnTypeContext,
  extraParametersFromMetaValues,
  initMonacoLanague,
  MenuContext,
  MenuRefresh,
  tabsHasId,
  TabsSetContext,
  UuidContext,
} from './Common';
import types from './ApiStruct';
import { invoke } from '@tauri-apps/api/core';
import { NotificationContext } from './Notification';
import { TabContent } from './Tabs';
import { DefAndMetasView } from './UdfAndMetaView';

import { TbMathFunction } from 'react-icons/tb';
import { calMenuStyle, DataSourceKind, secondConfirm } from './helper';
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
  const getItemId = (x: types.NamedStaff) => {
    switch (kind) {
      case DataSourceKind.Postgres:
        let p = extraParametersFromMetaValues(x.metas);
        return idPrefix + '-' + x.name + '-' + p;
      default:
        return idPrefix + '-' + x.name;
    }
  };

  let { setUpMenu } = useContext(MenuContext);
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
          key={p.name + '-' + extraParametersFromMetaValues(p.metas)}
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
                    parameters={extraParametersFromMetaValues(p.metas)}
                  ></UdfMenu>
                ),
                style: calMenuStyle(e, 100),
              });
            }}
          >
            <TbMathFunction></TbMathFunction>
            <span>{p.name}</span>
            {kind == DataSourceKind.Postgres && (
              <em> ({extraParametersFromMetaValues(p.metas)}) </em>
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
  reload;
  kind: DataSourceKind;
}) {
  const { uuid, udf, parameters, reload, kind } = props;

  const { setSelectIndex, setTabs, tabs } = useContext(TabsSetContext);
  let arg =
    kind == DataSourceKind.Postgres ? { extra: 'postgres', parameters } : null;
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
                  return await invoke<types.UdfDetail>('show_udf', {
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
