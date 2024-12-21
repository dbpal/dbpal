import { useContext, useEffect, useState } from 'react';
import {
  DataSourceKind,
  MenuContext,
  MenuRefresh,
  NameAndTypeContext,
  progressBarSize,
  setUpMenu,
} from './Common';
import { NotificationContext } from './Notification';
import { secondConfirm } from './Confirm';
import { invoke } from '@tauri-apps/api/core';
import {
  CircularProgress,
  ListItem,
  MenuItem,
  MenuList,
  Stack,
  Typography,
} from '@mui/material';
import ListIcon from '@mui/icons-material/List';

import DeleteForeverIcon from '@mui/icons-material/DeleteForever';

import types from './ApiStruct';
import { TreeItem } from '@mui/x-tree-view/TreeItem';

function ViewsTree() {
  const { uuid } = useContext(NameAndTypeContext);
  const [views, setViews] = useState<types.NamedStaff[]>([]);
  const { showNotification } = useContext(NotificationContext);
  const [loading, setLoading] = useState(false);

  const handleReload = () => {
    setLoading(true);
    invoke<types.NamedStaff[]>('list_view', { name: uuid })
      .then((ts) => {
        setLoading(false);
        setViews(ts);
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
      label=<div 
        onContextMenu={(e) => {
          setMenuContent(
            <MenuList>
              <MenuRefresh reload={handleReload}></MenuRefresh>
            </MenuList>
          );
          setUpMenu(e, 70);
        }}
      >
        views
      </div>
    >
      {loading && (
        <TreeItem
          itemId={uuid + '-views' + 'loading'}
          label={<CircularProgress size={progressBarSize}></CircularProgress>}
        ></TreeItem>
      )}

      {views.map((e) => (
        <ViewTree key={e.name} view={e.name} reload={handleReload}></ViewTree>
      ))}
    </TreeItem>
  );
}

function ViewTree(props: { view: string; reload }) {
  let { view, reload } = props;
  const { uuid, conn_type } = useContext(NameAndTypeContext);
  let { setMenuContent } = useContext(MenuContext);
  return (
    <TreeItem
      itemId={uuid + '-' + view}
      onContextMenu={(e) => {
        setMenuContent(
          <ViewMenu kind={conn_type} reload={reload} view={view} uuid={uuid} />
        );
        setUpMenu(e, 100);
      }}
      label={<>{view}</>}
    ></TreeItem>
  );
}

function ViewMenu(props: {
  view: string;
  uuid: string;
  reload;
  kind: DataSourceKind;
}) {
  let { view, uuid, reload } = props;
  const { showNotification } = useContext(NotificationContext);
  const handleDrop = async () => {
    let confirm = await secondConfirm({
      title: 'Drop view',
      content: '',
      confirmText: 'Drop',
    });
    if (!confirm) {
      return;
    }
    invoke('drop_view', { name: uuid, viewName: view })
      .then((_) => {
        showNotification('success', 'Drop View OK');
        reload();
      })
      .catch((e) => {
        showNotification('error', 'Drop View Failure', e);
      });
  };

  return (
    <Stack>
      <MenuList>
        <MenuItem>
          <ListItem>
            <ListIcon></ListIcon>
          </ListItem>
          <ListItem> open table list</ListItem>
          <Typography
            variant='body2'
            sx={{ color: 'text.secondary' }}
          ></Typography>
        </MenuItem>
        <MenuItem
          sx={{ color: 'error.main' }}
          onClick={() => {
            handleDrop();
          }}
        >
          <ListItem>
            <DeleteForeverIcon></DeleteForeverIcon>
          </ListItem>
          <ListItem>drop view </ListItem>
          <Typography
            variant='body2'
            sx={{ color: 'text.secondary' }}
          ></Typography>
        </MenuItem>
      </MenuList>
    </Stack>
  );
}

export { ViewsTree };
