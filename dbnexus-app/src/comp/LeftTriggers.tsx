import { useContext, useEffect, useState } from 'react';
import {
  MenuContext,
  MenuRefresh,
  progressBarSizeSmall,
  setUpMenu,
  UuidContext,
} from './Common';
import { NotificationContext } from './Notification';
import types from './ApiStruct';
import { invoke } from '@tauri-apps/api/core';
import { CircularProgress, MenuList, Stack } from '@mui/material';
import { TreeItem } from '@mui/x-tree-view/TreeItem';

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

  let { setMenuContent } = useContext(MenuContext);

  return (
    <TreeItem
      itemId={uuid + '-views'}
      label=<Stack
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
      {triggers.map((e) => (
        <ViewTree key={e.name} view={e.name} reload={handleReload}></ViewTree>
      ))}
    </TreeItem>
  );
}
