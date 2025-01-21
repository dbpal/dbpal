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

import { SiProcessingfoundation } from 'react-icons/si';
import { calMenuStyle, DataSourceKind, secondConfirm } from './helper';
function ProceduresTree() {
  const { uuid } = useContext(UuidContext);
  const { kind } = useContext(ConnTypeContext);
  const idPrefix = uuid + '-procudures-';
  const [ps, setPos] = useState<types.NamedStaff[]>([]);
  const { showNotification } = useContext(NotificationContext);
  const reload = () => {
    invoke<types.NamedStaff[]>('list_procedure', { name: uuid })
      .then((ts) => {
        setPos(ts);
      })
      .catch((e) => {
        showNotification('error', 'list procedure failed', e);
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
        direction="row"
        spacing={1}
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
        <SiProcessingfoundation></SiProcessingfoundation>
        <span> procedures </span>
      </Stack>
    >
      {ps.map((p) => (
        <TreeItem
          key={p.name + '-' + extraParametersFromMetaValues(p.metas)}
          itemId={getItemId(p)}
          sx={{ alignItems: 'flex-end' }}
          label=<Stack
            spacing={1}
            direction="row"
            onContextMenu={(e) => {
              setUpMenu({
                content: (
                  <ProcedureMenu
                    reload={reload}
                    uuid={uuid}
                    kind={kind}
                    procedure={p.name}
                    parameters={extraParametersFromMetaValues(p.metas)}
                  ></ProcedureMenu>
                ),
                style: calMenuStyle(e, 100),
              });
            }}
            sx={{ alignItems: 'center' }}
          >
            <SiProcessingfoundation></SiProcessingfoundation>
            <span>{p.name}</span>
            {kind == DataSourceKind.Postgres && (
              <em> ( {extraParametersFromMetaValues(p.metas)} ) </em>
            )}
          </Stack>
        ></TreeItem>
      ))}
    </TreeItem>
  );
}

function ProcedureMenu(props: {
  uuid;
  procedure;
  parameters?: string;
  reload;
  kind: DataSourceKind;
}) {
  const { uuid, procedure, parameters, reload, kind } = props;

  const { setSelectIndex, setTabs, tabs } = useContext(TabsSetContext);
  let arg =
    kind == DataSourceKind.Postgres ? { extra: 'postgres', parameters } : null;
  const { showNotification } = useContext(NotificationContext);

  const handleOpenDetails = () => {
    initMonacoLanague(uuid, kind);
    const id = uuid + '-' + '-procedure-' + procedure;
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
            procedure,
            (
              <DefAndMetasView
                reloadOP={async () => {
                  return await invoke<types.ProcedureDetail>('show_procedure', {
                    name: uuid,
                    procedure,
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
      title: `Drop Procedure '${procedure}'`,
      confirmText: 'drop',
    });
    if (!confirm) {
      return;
    }

    invoke('drop_procedure', { name: uuid, procedure, arg })
      .then(() => {
        showNotification('success', 'Drop procedure OK');
        reload();
      })
      .catch((e) => {
        showNotification('error', 'Drop procedure Failure', e);
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

export { ProceduresTree };
