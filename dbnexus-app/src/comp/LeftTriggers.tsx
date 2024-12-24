import { useContext, useEffect, useState } from 'react';
import {
  MenuContext,
  MenuRefresh,
  progressBarSizeSmall,
  setUpMenu,
  tabsHasId,
  TabsSetContext,
  UuidContext,
} from './Common';
import { NotificationContext } from './Notification';
import types from './ApiStruct';
import { invoke } from '@tauri-apps/api/core';
import {
  CircularProgress,
  ListItemIcon,
  ListItemText,
  MenuItem,
  MenuList,
  Paper,
  Stack,
} from '@mui/material';
import { TreeItem } from '@mui/x-tree-view/TreeItem';
import { secondConfirm } from './Confirm';
import DeleteForeverIcon from '@mui/icons-material/DeleteForever';
import { TabContent } from './Tabs';
import { TriggerDetail } from './TriggerDetail';
import InfoIcon from '@mui/icons-material/Info';
import CheckCircleIcon from '@mui/icons-material/CheckCircle';
import CancelIcon from '@mui/icons-material/Cancel';
function TriggersTree(props: { table: string }) {
  const { uuid } = useContext(UuidContext);
  let { table } = props;
  const [triggers, setTriggers] = useState<types.NamedStaff[]>([]);
  const { showNotification } = useContext(NotificationContext);
  const [loading, setLoading] = useState(false);

  const handleReload = () => {
    setLoading(true);
    invoke<types.NamedStaff[]>('list_trigger', { name: uuid, tableName: table })
      .then((ts) => {
        setLoading(false);
        setTriggers(ts);
      })
      .catch((e) => {
        setLoading(false);
        showNotification('error', 'list view failed', e);
      });
  };
  useEffect(() => {
    handleReload();
  }, []);
  let idPrefix = uuid + '-tables-' + table + '-triggers-';

  let { setMenuContent } = useContext(MenuContext);
  return (
    <TreeItem
      itemId={idPrefix}
      label=<Stack
        spacing={1}
        direction="row"
        onContextMenu={(e) => {
          setMenuContent(
            <MenuList>
              <MenuRefresh reload={handleReload}></MenuRefresh>
            </MenuList>
          );
          setUpMenu(e, 70);
        }}
      >
        <span> triggers </span>
        {loading && (
          <CircularProgress size={progressBarSizeSmall}></CircularProgress>
        )}
      </Stack>
    >
      {triggers.map((trigger) => (
        <TreeItem
          key={trigger.name}
          itemId={idPrefix + '-' + trigger.name}
          label=<Stack
            direction="row"
            spacing={1}
            onContextMenu={(event) => {
              setMenuContent(
                <TriggerMenu
                  table={table}
                  uuid={uuid}
                  reload={handleReload}
                  trigger={trigger.name}
                ></TriggerMenu>
              );
              setUpMenu(event, 70);
            }}
          >
            <span> {trigger.name} </span>
          </Stack>
        ></TreeItem>
      ))}
    </TreeItem>
  );
}

function TriggerMenu(props: {
  table: string;
  uuid: string;
  reload;
  trigger: string;
}) {
  const { showNotification } = useContext(NotificationContext);
  const { table, uuid, reload, trigger } = props;
  const [detail, setDetail] = useState<types.TriggerDetail>(null);
  let callPara = { name: uuid, tableName: table, trigger };

  const handleDrop = async () => {
    const confirm = await secondConfirm({
      title: `Drop trigger '${trigger}'`,
      confirmText: 'Drop',
    });
    if (!confirm) {
      return;
    }
    invoke('drop_trigger', callPara)
      .then(() => {
        showNotification('success', 'Drop Trigger OK');
        reload();
      })
      .catch((e) => {
        showNotification('error', 'Drop trigger failure', e);
      });
  };

  const loadDetails = () => {
    invoke<types.TriggerDetail>('show_trigger', {
      name: uuid,
      tableName: table,
      trigger,
    })
      .then((r) => {
        setDetail(r);
      })
      .catch((e) => {
        showNotification('error', 'show trigger failure', e);
      });
  };
  const handleEnabless = (name: string, operation: string) => {
    invoke(name, callPara)
      .then(() => {
        showNotification('error', `${operation}  OK`);
        loadDetails();
      })
      .catch((e) => {
        showNotification('error', `${operation}  Failure`, e);
      });
  };

  useEffect(() => {
    loadDetails();
  }, []);

  const { setSelectIndex, setTabs, tabs } = useContext(TabsSetContext);

  const handleOpenDetails = () => {
    const id = uuid + '-table-' + table + '-trigger-' + trigger;
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
            table,
            (
              <TriggerDetail
                reloadOP={async () => {
                  return await invoke<types.TriggerDetail>('show_trigger', {
                    name: uuid,
                    tableName: table,
                    trigger,
                  });
                }}
                language="xxx"
              ></TriggerDetail>
            )
          ),
        ];
      });
    }
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
        {detail && // details
          detail.enable && // and details.enable have value
          (triggerIsEnable(detail.enable) ? (
            <MenuItem
              sx={{ color: 'error.main' }}
              onClick={() => {
                handleEnabless('disable_trigger', 'Disable trigger');
              }}
            >
              <ListItemIcon>
                <CancelIcon></CancelIcon>
              </ListItemIcon>
              <ListItemText>disable</ListItemText>
            </MenuItem>
          ) : (
            <MenuItem
              onClick={() => {
                handleEnabless('enable_trigger', 'Enable trigger');
              }}
            >
              <ListItemIcon>
                <CheckCircleIcon color="error"></CheckCircleIcon>
              </ListItemIcon>
              <ListItemText>enable</ListItemText>
            </MenuItem>
          ))}

        <MenuItem
          sx={{ color: 'error.main' }}
          onClick={() => {
            handleDrop();
          }}
        >
          <ListItemIcon color="error">
            <DeleteForeverIcon></DeleteForeverIcon>
          </ListItemIcon>
          <ListItemText>drop</ListItemText>
        </MenuItem>
      </MenuList>
    </Paper>
  );
}

function triggerIsEnable(enable: types.TriggerEnable): boolean {
  if (enable.kind == 'postgres') {
    return enable.enable.toUpperCase() != 'D';
  }
  if (enable.kind == 'bool') {
    return enable.enable;
  }
}

export { TriggersTree };
