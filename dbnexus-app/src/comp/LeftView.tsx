import { ReactNode, useContext, useEffect, useState } from 'react';
import {
  progressBarSizeSmall,
  tabsHasId,
  modalStyle,
  DocsAndCount,
} from './Common';
import { NotificationContext } from './Notification';

import { invoke } from '@tauri-apps/api/core';
import {
  Box,
  CircularProgress,
  ListItemIcon,
  ListItemText,
  MenuItem,
  MenuList,
  Modal,
  Paper,
  Stack,
  Typography,
} from '@mui/material';
import ArrowCircleDownIcon from '@mui/icons-material/ArrowCircleDown';
import DeleteForeverIcon from '@mui/icons-material/DeleteForever';
import VisibilityIcon from '@mui/icons-material/Visibility';

import types from './ApiStruct';
import { TreeItem } from '@mui/x-tree-view/TreeItem';
import ListIcon from '@mui/icons-material/List';
import { SqlTableView } from './SqlTableView';
import { TabContent } from './Tabs';
import { ExportTableOrView } from './Export';
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
import { MongoTable } from './MongoDB';
import { BSON, EJSON } from 'bson';

enum ViewKind {
  Normal,
  Materrialized,
}

function ViewsTree(props: { viewkind: ViewKind }) {
  const { viewkind } = props;

  const { uuid } = useContext(UuidContext);
  const [views, setViews] = useState<types.NamedStaff[] | types.ListTable[]>(
    []
  );
  const { showNotification } = useContext(NotificationContext);
  const [loading, setLoading] = useState(false);
  let invokeName = '';
  switch (viewkind) {
    case ViewKind.Normal:
      invokeName = 'list_view';
      break;
    case ViewKind.Materrialized:
      invokeName = 'list_materialized_view';
      break;
  }

  const handleReload = () => {
    setLoading(true);
    invoke<types.NamedStaff[] | types.ListTable[]>(invokeName, { name: uuid })
      .then((ts) => {
        setViews(ts);
      })
      .catch((e) => {
        showNotification('error', 'list view failed', e);
      })
      .finally(() => {
        setLoading(false);
      });
  };
  useEffect(() => {
    handleReload();
  }, []);

  let { setUpMenu } = useContext(ContextMenuContext);
  let itemId;
  switch (viewkind) {
    case ViewKind.Normal:
      itemId = uuid + '-views';
      break;
    case ViewKind.Materrialized:
      itemId = uuid + '-materialized-views';
      break;
  }

  let name;
  switch (viewkind) {
    case ViewKind.Normal:
      name = 'views';
      break;
    case ViewKind.Materrialized:
      name = 'materizlized views';
      break;
  }

  return (
    <TreeItem
      itemId={itemId}
      label=<Stack
        direction="row"
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
        <span> {name} </span>
        {loading && (
          <CircularProgress size={progressBarSizeSmall}></CircularProgress>
        )}
      </Stack>
    >
      {views.map((e) => (
        <ViewTree
          viewkind={viewkind}
          key={e.name}
          view={e.name}
          reload={handleReload}
          size={e.size}
        ></ViewTree>
      ))}
    </TreeItem>
  );
}

function ViewTree(props: {
  view: string;
  reload;
  viewkind: ViewKind;
  size?: string;
}) {
  let { view, reload, viewkind, size } = props;

  const { uuid } = useContext(UuidContext);
  const { kind: conn_type } = useContext(ConnTypeContext);
  let { setUpMenu } = useContext(ContextMenuContext);
  return (
    <TreeItem
      itemId={uuid + '-view-' + view}
      label={
        <Stack
          direction="row"
          spacing={1}
          sx={{ justifyContent: 'space-between' }}
          onContextMenu={(e) => {
            setUpMenu({
              content: (
                <ViewMenu
                  kind={conn_type}
                  reload={reload}
                  view={view}
                  uuid={uuid}
                  viewkind={viewkind}
                />
              ),
              style: calMenuStyle(e, 100),
            });
          }}
        >
          <Stack direction="row" spacing={1}>
            <VisibilityIcon></VisibilityIcon> <span> {view} </span>
          </Stack>
          {viewkind == ViewKind.Materrialized && <span> {size} </span>}
        </Stack>
      }
    >
      <ViewColumnsTree view={view} viewkind={viewkind}></ViewColumnsTree>
    </TreeItem>
  );
}

function ViewColumnsTree(props: { view: string; viewkind: ViewKind }) {
  const { view, viewkind } = props;
  const [fields, setFields] = useState<types.Field[]>([]);
  const { showNotification } = useContext(NotificationContext);
  const { uuid } = useContext(UuidContext);
  const [loading, setLoading] = useState(false);
  const reload = () => {
    setLoading(true);
    invoke<types.Field[]>('show_view_coumns', { name: uuid, viewName: view })
      .then((fs) => {
        setFields(fs);
      })
      .catch((e) => {
        showNotification('error', 'list column failed', e);
      })
      .finally(() => {
        setLoading(false);
      });
  };
  let { setUpMenu } = useContext(ContextMenuContext);
  useEffect(() => {
    reload();
  }, []);
  let idPrefix = uuid + '-views-' + view + viewkind + '-columns-';
  return (
    <TreeItem
      itemId={idPrefix}
      label=<Stack
        spacing={1}
        direction="row"
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
          label=<Stack direction="row" spacing={1}>
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
  viewkind: ViewKind;
}) {
  let { view, uuid, reload, kind, viewkind } = props;
  const { showNotification } = useContext(NotificationContext);
  const [showExport, setShowExport] = useState(false);
  const handleDrop = async () => {
    let confirm = await secondConfirm({
      title: 'Drop view',
      content: '',
      confirmText: 'Drop',
    });
    if (!confirm) {
      return;
    }
    let invokeName;
    switch (viewkind) {
      case ViewKind.Normal:
        invokeName = 'drop_view';
        break;
      case ViewKind.Materrialized:
        invokeName = 'drop_materialized_view';
        break;
    }
    invoke(invokeName, { name: uuid, viewName: view })
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
        case DataSourceKind.Oracle:
        case DataSourceKind.TiDB:
          list = (
            <SqlTableView
              notWarnPri={true}
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
              put_value_in_sql={async (typ, value) => {
                return invoke('put_value_in_sql', {
                  name: uuid,
                  typ,
                  value: value ? value : null,
                });
              }}
            ></SqlTableView>
          );
          break;

        case DataSourceKind.Mongo:
          list = (
            <MongoTable
              reloadOPTableList={async (filter, sort, page, pageSize) => {
                return await invoke('mongo_show_collection_table_list', {
                  name: uuid,
                  tableName: view,
                  filter: BSON.serialize(filter),
                  sort: BSON.serialize(sort),
                  page,
                  pageSize,
                });
              }}
              realodOPDocs={async (filter, sort, page, pageSize) => {
                try {
                  let x = await invoke('mongo_show_collection_docs', {
                    name: uuid,
                    tableName: view,
                    filter: BSON.serialize(filter),
                    sort: BSON.serialize(sort),
                    page,
                    pageSize,
                  });
                  //@ts-ignore
                  let docs = x.docs.map((e) =>
                    BSON.deserialize(new Uint8Array(e))
                  );
                  //@ts-ignore
                  let count = x.count;
                  return new DocsAndCount(docs, count);
                } catch (e) {
                  throw e;
                }
              }}
              loadOne={async (id) => {
                try {
                  let x = await invoke<number[]>(
                    'mongo_get_one_document_by_id',
                    {
                      name: uuid,
                      tableName: view,
                      id: JSON.stringify(EJSON.serialize(id)),
                    }
                  );
                  return BSON.deserialize(new Uint8Array(x));
                } catch (e) {
                  throw e;
                }
              }}
            ></MongoTable>
          );
          break;

        default:
          showNotification('error', `datasouce ${kind} has no view`);
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
            variant="body2"
            sx={{ color: 'text.secondary' }}
          ></Typography>
        </MenuItem>

        <MenuItem
          onClick={() => {
            setShowExport(true);
          }}
        >
          <ListItemIcon>
            <ArrowCircleDownIcon></ArrowCircleDownIcon>
          </ListItemIcon>
          <ListItemText> export </ListItemText>
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
          <ListItemText>drop </ListItemText>
          <Typography
            variant="body2"
            sx={{ color: 'text.secondary' }}
          ></Typography>
        </MenuItem>
      </MenuList>

      {showExport && (
        <Modal
          open={showExport}
          onClose={(_, reason) => {
            if (reason === 'backdropClick') {
              return;
            }
            setShowExport(false);
          }}
          aria-labelledby="drop"
        >
          <Box sx={modalStyle} width="800px">
            <ExportTableOrView
              uuid={uuid}
              name={view}
              handleClose={() => {
                setShowExport(false);
              }}
              kind={kind}
              support_kind={['csv', 'json', 'jsonline']}
            ></ExportTableOrView>
          </Box>
        </Modal>
      )}
    </Paper>
  );
}

export { ViewsTree, ViewKind };
