import { useContext, useEffect, useState } from 'react';
import {
  ConnTypeContext,
  DataSourceKind,
  MenuContext,
  MenuRefresh,
  UuidContext,
  progressBarSize,
  setUpMenu,
} from './Common';
import { NotificationContext } from './Notification';
import { secondConfirm } from './Confirm';
import { invoke } from '@tauri-apps/api/core';
import {
  CircularProgress,
  ListItem,
  ListItemIcon,
  ListItemText,
  MenuItem,
  MenuList,
  Stack,
  Typography,
} from '@mui/material';

import DeleteForeverIcon from '@mui/icons-material/DeleteForever';
import VisibilityIcon from '@mui/icons-material/Visibility';

import types from './ApiStruct';
import { TreeItem } from '@mui/x-tree-view/TreeItem';
import ListIcon from '@mui/icons-material/List';
function ViewsTree() {
  const { uuid } = useContext(UuidContext);
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
          itemId={uuid + '-views-' + 'loading'}
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
  const { uuid } = useContext(UuidContext);
  const { conn_type } = useContext(ConnTypeContext);
  let { setMenuContent } = useContext(MenuContext);
  return (
    <TreeItem
      itemId={uuid + '-view-' + view}
      label={
        <Stack
          direction='row'
          onContextMenu={(e) => {
            setMenuContent(
              <ViewMenu
                kind={conn_type}
                reload={reload}
                view={view}
                uuid={uuid}
              />
            );
            setUpMenu(e, 100);
          }}
          spacing={1}
        >
          <VisibilityIcon></VisibilityIcon> <span> {view} </span>
        </Stack>
      }
    >
      <ViewColumnsTree view={view}></ViewColumnsTree>
    </TreeItem>
  );
}

function ViewColumnsTree(props: { view: string }) {
  const { view } = props;
  const [fields, setFields] = useState<types.Field[]>([]);
  const { showNotification } = useContext(NotificationContext);
  const { uuid } = useContext(UuidContext);
  const reload = () => {
    invoke<types.Field[]>('show_view_coumns', { name: uuid, viewName: view })
      .then((fs) => {
        setFields(fs);
      })
      .catch((e) => {
        showNotification('error', 'list column failed', e);
      });
  };
  let { setMenuContent } = useContext(MenuContext);
  useEffect(() => {
    reload();
  }, []);
  let idPrefix = uuid + '-viees-' + view + 'columns';
  return (
    <TreeItem
      itemId={idPrefix}
      label=<Stack
        onContextMenu={(e) => {
          setMenuContent(
            <MenuList>
              <MenuRefresh reload={reload}></MenuRefresh>
            </MenuList>
          );
          setUpMenu(e, 200);
        }}
      >
        columns
      </Stack>
    >
      {fields.map((f) => (
        <TreeItem
          key={f.field}
          itemId={idPrefix + '-' + f.field}
          label=<Stack direction='row' spacing={1}>
            {f.field} <em>({f.typ}) </em>
          </Stack>
        ></TreeItem>
      ))}
    </TreeItem>
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
          <ListItemIcon>
            <ListIcon></ListIcon>
          </ListItemIcon>
          <ListItemText> open table list</ListItemText>
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
          <ListItemIcon>
            <DeleteForeverIcon></DeleteForeverIcon>
          </ListItemIcon>
          <ListItemText>drop view </ListItemText>
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
