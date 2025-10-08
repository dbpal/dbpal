'use strict';
import {
  ListItemIcon,
  ListItemText,
  MenuItem,
  MenuList,
  Stack,
} from '@mui/material';
import { TreeItem } from '@mui/x-tree-view/TreeItem';
import { useContext, useEffect, useRef, useState } from 'react';
import DeleteForeverIcon from '@mui/icons-material/DeleteForever';
import InfoIcon from '@mui/icons-material/Info';
import { DefAndMetas, tabsHasId } from './Common';
import types from './ApiStruct';
import { invoke } from '@tauri-apps/api/core';
import { NotificationContext } from './Notification';
import { TabContent } from './Tabs';
import { DefAndMetasView } from './UdfAndMetaView';
import { PiTreeStructureFill } from 'react-icons/pi';
import {
  ConnTypeContext,
  ContextMenuContext,
  DataSourceKind,
  generateRandomString,
  MatchPatternContext,
  secondConfirm,
  SettingContext,
  TabsSetContext,
  UuidContext,
} from './helper';
import { ExceedMaxTreeItem, MenuRefresh } from './CommonComp';
function TypesTree() {
  const { uuid } = useContext(UuidContext);
  const { kind } = useContext(ConnTypeContext);
  const [types, setTypes] = useState<types.CreatedTypeDef[]>([]);
  const { showNotification } = useContext(NotificationContext);
  const setting = useContext(SettingContext);

  const reload = () => {
    invoke<types.CreatedTypeDef[]>('list_created_types', { name: uuid })
      .then((ts) => {
        setTypes(ts);
      })
      .catch((e) => {
        showNotification('error', 'list types failed', e);
      });
  };
  const [exceed, setExceed] = useState(0);
  const matchPattern = useContext(MatchPatternContext);
  useEffect(() => {
    setExceed(types.filter((e) => matchPattern(e.name)).length);
  }, [types, matchPattern]);

  const getItemId = (x: types.CreatedTypeDef) => {
    return `${uuid}-type-${x.name}`;
  };
  let { setUpMenu } = useContext(ContextMenuContext);
  useEffect(() => {
    reload();
  }, []);

  const idForTrees = useRef(generateRandomString());
  return (
    <TreeItem
      itemId={idForTrees.current}
      label=<Stack
        direction="row"
        spacing={1}
        sx={{ alignItems: 'center' }}
        onContextMenu={(e) => {
          setUpMenu({
            content: (
              <MenuList>
                <MenuRefresh reload={reload}></MenuRefresh>
              </MenuList>
            ),
            event: e,
          });
        }}
      >
        <PiTreeStructureFill />
        <span> types </span>
      </Stack>
    >
      {types
        .filter((typ) => matchPattern(typ.name))
        .map((typ, index) => {
          if (index >= setting.maxTreeSize) {
            return;
          }
          return (
            <TreeItem
              key={typ.name}
              itemId={getItemId(typ)}
              style={{ alignItems: 'center' }}
              label=<Stack
                direction="row"
                onContextMenu={(e) => {
                  setUpMenu({
                    content: (
                      <TypeMenu
                        reload={reload}
                        uuid={uuid}
                        kind={kind}
                        createdType={typ}
                      ></TypeMenu>
                    ),
                    event: e,
                  });
                }}
                sx={{ alignItems: 'center' }}
                spacing={0.5}
              >
                <PiTreeStructureFill />
                <span> {typ.name} </span>
              </Stack>
            ></TreeItem>
          );
        })}
      <ExceedMaxTreeItem
        exceed={exceed}
        maxTreeSize={setting.maxTreeSize}
      ></ExceedMaxTreeItem>
    </TreeItem>
  );
}

function TypeMenu(props: {
  uuid;
  createdType: types.CreatedTypeDef;
  reload;
  kind: DataSourceKind;
}) {
  const { uuid, createdType, reload, kind } = props;
  const { setSelectTab, setTabs, tabs } = useContext(TabsSetContext);
  const { showNotification } = useContext(NotificationContext);
  const handleOpenDetails = () => {
    const id = uuid + '-' + '-type-' + createdType.name;
    let index = tabsHasId(tabs, id);
    if (typeof index == 'number') {
      setSelectTab(index);
    } else {
      setTabs((e) => {
        setTimeout(() => {
          setSelectTab(tabs.length);
        }, 100);

        return [
          ...e,
          new TabContent(
            id,
            createdType.name,
            (
              <DefAndMetasView
                reloadOP={async () => {
                  return Promise.resolve({
                    def: createdType.sql,
                  } as DefAndMetas);
                }}
                kind={kind}
                uuid={uuid}
              ></DefAndMetasView>
            ),
            <PiTreeStructureFill />
          ),
        ];
      });
    }
  };
  const handleDrop = async () => {
    let confirm = await secondConfirm({
      title: `Drop type '${createdType.name}'`,
      confirmText: 'Drop',
    });
    if (!confirm) {
      return;
    }

    invoke('drop_created_type', { name: uuid, typeName: createdType.name })
      .then(() => {
        showNotification('success', 'Drop type OK');
        reload();
      })

      .catch((e) => {
        showNotification('error', 'Drop type Failure', e);
      });
  };
  return (
    <MenuList>
      <MenuItem
        onClick={() => {
          handleOpenDetails();
        }}
      >
        <ListItemIcon>
          <InfoIcon></InfoIcon>
        </ListItemIcon>
        <ListItemText> Detail </ListItemText>
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
        <ListItemText> Drop</ListItemText>
      </MenuItem>
    </MenuList>
  );
}

export { TypesTree };
