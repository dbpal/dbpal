import { ReactNode, useContext, useEffect, useState } from 'react';
import {
  ConnTypeContext,
  DataSourceKind,
  MenuContext,
  MenuRefresh,
  UuidContext,
  progressBarSizeSmall,
  calMenuStyle,
  tabsHasId,
  TabsSetContext,
} from './Common';
import { NotificationContext } from './Notification';
import { secondConfirm } from './Confirm';
import { invoke } from '@tauri-apps/api/core';
import {
  CircularProgress,
  ListItemIcon,
  ListItemText,
  MenuItem,
  MenuList,
  Paper,
  Stack,
  Typography,
} from '@mui/material';

import DeleteForeverIcon from '@mui/icons-material/DeleteForever';
import VisibilityIcon from '@mui/icons-material/Visibility';

import types from './ApiStruct';
import { TreeItem } from '@mui/x-tree-view/TreeItem';
import ListIcon from '@mui/icons-material/List';
import { SqlTableView } from './SqlTableView';
import { TabContent } from './Tabs';

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

  let { setUpMenu } = useContext(MenuContext);

  return (
    <TreeItem
      itemId={uuid + '-views'}
      label=<Stack
        direction='row'
        spacing={1}
        onContextMenu={(e) => {
          setUpMenu({
            content: (
              <Paper>
                <MenuList>
                  <MenuRefresh reload={handleReload}></MenuRefresh>
                </MenuList>
              </Paper>
            ),
            style: calMenuStyle(e, 70),
          });
        }}
      >
        <VisibilityIcon></VisibilityIcon>
        <span> views</span>
        {loading && (
          <CircularProgress size={progressBarSizeSmall}></CircularProgress>
        )}
      </Stack>
    >
      {views.map((e) => (
        <ViewTree key={e.name} view={e.name} reload={handleReload}></ViewTree>
      ))}
    </TreeItem>
  );
}

function ViewTree(props: { view: string; reload }) {
  let { view, reload } = props;
  const { uuid } = useContext(UuidContext);
  const { kind: conn_type } = useContext(ConnTypeContext);
  let { setUpMenu } = useContext(MenuContext);
  return (
    <TreeItem
      itemId={uuid + '-view-' + view}
      label={
        <Stack
          direction='row'
          spacing={1}
          onContextMenu={(e) => {
            setUpMenu({
              content: (
                <ViewMenu
                  kind={conn_type}
                  reload={reload}
                  view={view}
                  uuid={uuid}
                />
              ),
              style: calMenuStyle(e, 100),
            });
          }}
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
  const [loading, setLoading] = useState(false);
  const reload = () => {
    setLoading(true);
    invoke<types.Field[]>('show_view_coumns', { name: uuid, viewName: view })
      .then((fs) => {
        setLoading(false);
        setFields(fs);
      })
      .catch((e) => {
        setLoading(false);
        showNotification('error', 'list column failed', e);
      });
  };
  let { setUpMenu } = useContext(MenuContext);
  useEffect(() => {
    reload();
  }, []);
  let idPrefix = uuid + '-viees-' + view + 'columns';
  return (
    <TreeItem
      itemId={idPrefix}
      label=<Stack
        spacing={1}
        direction='row'
        onContextMenu={(e) => {
          setUpMenu({
            content: (
              <Paper>
                <MenuList>
                  <MenuRefresh reload={reload}></MenuRefresh>
                </MenuList>
              </Paper>
            ),
            style: calMenuStyle(e, 200),
          });
        }}
      >
        <span> columns </span>
        {loading && (
          <CircularProgress size={progressBarSizeSmall}></CircularProgress>
        )}
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
  let { view, uuid, reload, kind } = props;
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
  const { tabs, setTabs, setSelectIndex } = useContext(TabsSetContext);

  const handleViewData = () => {
    const id = uuid + '-view-' + view;
    let index = tabsHasId(tabs, id);
    if (typeof index == 'number') {
      setSelectIndex(index);
    } else {
      let list: ReactNode = null;
      switch (kind) {
        case DataSourceKind.MariaDB:
        case DataSourceKind.Mysql:
        case DataSourceKind.MsSql:
        case DataSourceKind.Sqlite:
        case DataSourceKind.Postgres:
          list = (
            <SqlTableView
              reloadOP={async (
                where: string,
                orderBy: string,
                page: number,
                pageSize: number
              ) => {
                let where_null = where ? where : null;
                let orderBy_null = orderBy ? orderBy : null;
                return await invoke<types.MuiDataTableListResp>('show_view', {
                  name: uuid,
                  viewName: view,
                  whereStr: where_null,
                  orderBy: orderBy_null,
                  page,
                  pageSize,
                });
              }}
            ></SqlTableView>
          );
          break;
        case DataSourceKind.Redis:
          throw 'impl this';
        case DataSourceKind.Mongo:
          throw 'impl this';
      }

      setTabs((e) => {
        setSelectIndex(tabs.length);
        return [...e, new TabContent(id, view, list)];
      });
    }
  };

  return (
    <Paper>
      <MenuList>
        <MenuItem onClick={handleViewData}>
          <ListItemIcon>
            <ListIcon></ListIcon>
          </ListItemIcon>
          <ListItemText> view data</ListItemText>
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
            <DeleteForeverIcon color='error'></DeleteForeverIcon>
          </ListItemIcon>
          <ListItemText>drop </ListItemText>
          <Typography
            variant='body2'
            sx={{ color: 'text.secondary' }}
          ></Typography>
        </MenuItem>
      </MenuList>
    </Paper>
  );
}

export { ViewsTree };
