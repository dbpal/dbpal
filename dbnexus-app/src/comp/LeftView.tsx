import {
  ReactNode,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
} from 'react';
import {
  progressBarSizeSmall,
  tabsHasId,
  modalStyle,
  initMonacoLanague,
  SimpleFlexSizeEditor,
  makeAutoComplete,
  EscDownEvent,
} from './Common';
import ContentCopyIcon from '@mui/icons-material/ContentCopy';
import { NotificationContext } from './Notification';
import { invoke } from '@tauri-apps/api/core';
import { IoConstructOutline } from 'react-icons/io5';
import {
  Box,
  Button,
  Chip,
  CircularProgress,
  Divider,
  ListItemIcon,
  ListItemText,
  MenuItem,
  MenuList,
  Modal,
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
  columnWidths,
  ConnTypeContext,
  ContextMenuContext,
  DataSourceKind,
  generateRandomString,
  gridStyle,
  handleCopyToClipboard,
  MatchPatternContext,
  MongoTableHandler,
  quoteStyle,
  saveAndSetSize,
  secondConfirm,
  SettingContext,
  TabsSetContext,
  UuidContext,
  VisiableContext,
} from './helper';
import { MenuRefresh } from './CommonComp';
import { MongoTable } from './MongoDB';

enum ViewKind {
  Normal,
  Materrialized,
}
import CalendarViewWeekIcon from '@mui/icons-material/CalendarViewWeek';
import {
  DataGrid,
  GridColDef,
  GridSlotsComponentsProps,
} from '@mui/x-data-grid';
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
  let icon;

  switch (viewkind) {
    case ViewKind.Normal:
      name = 'views';
      icon = <VisibilityIcon></VisibilityIcon>;
      break;
    case ViewKind.Materrialized:
      name = 'materialized views';
      icon = <CalendarViewWeekIcon></CalendarViewWeekIcon>;
      break;
  }
  const matchPattern = useContext(MatchPatternContext);
  return (
    <TreeItem
      itemId={itemId}
      label=<Stack
        direction="row"
        spacing={1}
        onContextMenu={(e) => {
          setUpMenu({
            content: (
              <MenuList>
                <MenuRefresh reload={handleReload}></MenuRefresh>
              </MenuList>
            ),
            style: calMenuStyle(e, 70),
          });
        }}
      >
        {icon}
        <span> {name} </span>
        {loading && (
          <CircularProgress size={progressBarSizeSmall}></CircularProgress>
        )}
      </Stack>
    >
      {views
        .filter((e) => matchPattern(e.name))
        .map((e) => (
          <ViewTree
            viewkind={viewkind}
            key={e.name}
            view={e.name}
            reload={handleReload}
            //@ts-ignore
            size={e.size}
          ></ViewTree>
        ))}
    </TreeItem>
  );
}

function StatementStructure(props: {
  uuid: string;
  kind: DataSourceKind;
  view: string;
  viewKind: ViewKind;
}) {
  const { uuid, kind, view, viewKind } = props;
  const [sql, setSql] = useState('');
  const [loading, setLoading] = useState(false);
  const { showNotification } = useContext(NotificationContext);
  const handleReload = () => {
    setLoading(true);
    invoke<string>('create_view_sql', {
      name: uuid,
      viewName: view,
      materialized: viewKind == ViewKind.Materrialized,
    })
      .then((e) => {
        setSql(e);
      })
      .catch((e) => {
        showNotification('error', 'fetch create sql failed', e);
      })
      .finally(() => {
        setLoading(false);
      });
  };
  useEffect(() => {
    handleReload();
  }, []);
  initMonacoLanague(uuid, kind);
  const [dom, setDom] = useState(null);
  useEffect(() => {
    if (dom) {
      let c = saveAndSetSize(dom, 'statement', 'height');
      return c;
    }
  }, [dom]);

  return (
    <Stack
      spacing={1}
      height="60vh"
      style={{ resize: 'vertical', overflow: 'auto', minHeight: '20vh' }}
      ref={setDom}
    >
      <Stack direction="row">
        <Button
          loading={loading}
          variant="contained"
          onClick={() => {
            handleReload();
          }}
        >
          reload
        </Button>
      </Stack>
      {sql && (
        <SimpleFlexSizeEditor
          readOnly={true}
          language={uuid}
          value={sql}
        ></SimpleFlexSizeEditor>
      )}
    </Stack>
  );
}

function ViewStructure(props: {
  uuid: string;
  view: string;
  viewKind: ViewKind;
  kind;
}) {
  const { uuid, view, viewKind, kind } = props;

  const [structure, setStructure] = useState('columns');
  const structures = [{ name: 'columns', color: 'primary' }, ,];

  const supportStatement = [
    DataSourceKind.LibSql,
    DataSourceKind.Oracle,
    DataSourceKind.Mysql,
    DataSourceKind.MariaDB,
    DataSourceKind.TiDB,
    DataSourceKind.Sqlite,
  ].includes(kind);

  if (supportStatement) {
    structures.push({ name: 'statement', color: 'secondary' });
  }

  return (
    <Stack spacing={1}>
      <Stack direction="row" spacing={1} style={{ justifyContent: 'center' }}>
        {structures.map((e) => (
          <Chip
            onClick={() => {
              setStructure(e.name);
            }}
            disabled={structure == e.name}
            key={e.name}
            //@ts-ignore
            color={e.color}
            label={e.name}
          ></Chip>
        ))}
      </Stack>
      {structure == 'columns' && (
        <ColumnsContent uuid={uuid} view={view}></ColumnsContent>
      )}
      {structure == 'statement' && (
        <StatementStructure
          uuid={uuid}
          kind={kind}
          viewKind={viewKind}
          view={view}
        ></StatementStructure>
      )}
    </Stack>
  );
}

function ColumnsContent(props: { uuid: string; view: string }) {
  const { uuid, view } = props;
  const setting = useContext(SettingContext);

  const [fields, setFields] = useState<types.Field[]>([]);
  const { showNotification } = useContext(NotificationContext);
  const [loading, setLoading] = useState(false);
  const onColumnResize = useCallback(({ colDef, width }) => {
    columnWidths.saveColumnWidths(colDef.field, width);
  }, []);

  const handleReload = () => {
    setLoading(true);
    invoke<types.Field[]>('show_view_columns', {
      name: uuid,
      viewName: view,
    })
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

  useEffect(() => {
    handleReload();
  }, []);
  let cols = useMemo(() => {
    return [{ field: 'field' }, { field: 'typ' }, ,] as GridColDef[];
  }, []);
  cols.forEach((e) => {
    const width = columnWidths.getWidth(e.field);
    if (width) {
      e.width = width;
    }
  });
  const vis = useContext(VisiableContext);
  const { setUpMenu } = useContext(ContextMenuContext);

  function Menu(props: { content: string }) {
    const { content } = props;

    return (
      <MenuList>
        {content && (
          <MenuItem
            onClick={() => {
              handleCopyToClipboard(content, showNotification);
            }}
          >
            <ListItemIcon>
              <ContentCopyIcon></ContentCopyIcon>
            </ListItemIcon>
            <ListItemText> Copy </ListItemText>
          </MenuItem>
        )}
      </MenuList>
    );
  }

  const slotProps = useMemo(() => {
    return {
      row: {
        onContextMenu: (e) => {
          const { target } = e;
          {
            const field = (target as HTMLDivElement).getAttribute('data-field');
            if (!field) {
              return;
            }
          }
          let content = (target as HTMLDivElement).innerHTML;
          setUpMenu({
            content: <Menu content={content}></Menu>,
            style: calMenuStyle(e, 100),
          });
        },
        style: { cursor: 'context-menu' },
      },
    } as GridSlotsComponentsProps;
  }, []);

  const getRowId = useCallback((e) => {
    return e['field'];
  }, []);

  return (
    <Stack spacing={1}>
      <Stack direction="row" spacing={1}>
        <Button loading={loading} onClick={handleReload} variant="contained">
          reload
        </Button>
      </Stack>

      {vis && (
        <div
          style={{
            display: 'grid',
          }}
        >
          <DataGrid
            style={gridStyle}
            getRowId={getRowId}
            slotProps={slotProps}
            onColumnResize={onColumnResize}
            showCellVerticalBorder
            showColumnVerticalBorder
            rowHeight={setting.datagridRowHeight()}
            columnHeaderHeight={setting.datagridRowHeight()}
            columns={cols}
            rows={fields}
          ></DataGrid>
        </div>
      )}
    </Stack>
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
  let icon;
  switch (viewkind) {
    case ViewKind.Materrialized:
      icon = <CalendarViewWeekIcon></CalendarViewWeekIcon>;
      break;
    case ViewKind.Normal:
      icon = <VisibilityIcon></VisibilityIcon>;
      break;
  }
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
            {icon} <span> {view} </span>
          </Stack>
          {viewkind == ViewKind.Materrialized && <span> {size} </span>}
        </Stack>
      }
    ></TreeItem>
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
  const { tabs, setTabs, setSelectTab } = useContext(TabsSetContext);

  const handleViewData = () => {
    const id = uuid + '-view-' + generateRandomString();

    let list: ReactNode = null;
    switch (kind) {
      case DataSourceKind.MariaDB:
      case DataSourceKind.Mysql:
      case DataSourceKind.MsSql:
      case DataSourceKind.Sqlite:
      case DataSourceKind.Postgres:
      case DataSourceKind.Oracle:
      case DataSourceKind.TiDB:
      case DataSourceKind.Cassandra:
      case DataSourceKind.ClickHouse:
      case DataSourceKind.LibSql:
      case DataSourceKind.FireBird:
      case DataSourceKind.DuckDB:
      case DataSourceKind.Cockroach:
        list = (
          <SqlTableView
            invalidCassandraTableViewCache={async () => {
              return invoke<void>('cassandra_invalid_table_or_view_caching', {
                name: uuid,
                tableName: view,
              });
            }}
            quoteStyle={quoteStyle(kind)}
            autoCompletion={makeAutoComplete(uuid)}
            tableName={view}
            kind={kind}
            notWarnPri={true}
            reloadOP={async (
              where: string,
              orderBy: string,
              page: number,
              pageSize: number,
              extra
            ) => {
              let where_null = where ? where : null;
              let orderBy_null = orderBy ? orderBy : null;
              return await invoke<types.MuiDataGridData>('show_view', {
                name: uuid,
                viewName: view,
                whereStr: where_null,
                orderBy: orderBy_null,
                page,
                pageSize,
                extra,
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
            reloadOPTableList={MongoTableHandler.reloadTableList(uuid, view)}
            reloadOPDocs={MongoTableHandler.reloadDocs(uuid, view)}
            loadOne={MongoTableHandler.loadOne(uuid, view)}
          ></MongoTable>
        );
        break;
      default:
        showNotification('error', `datasouce ${kind} need impl view`);
    }

    setTabs((e) => {
      setSelectTab(tabs.length);
      return [
        ...e,
        new TabContent(id, view, list, <VisibilityIcon> </VisibilityIcon>),
      ];
    });
  };

  const handleSturcture = () => {
    let id = uuid + '-view-' + view + '-structure';
    let index = tabsHasId(tabs, id);
    if (typeof index == 'number') {
      setSelectTab(index);
    } else {
      setTabs((e) => {
        setSelectTab(tabs.length);
        return [
          ...e,
          new TabContent(
            id,
            `${view}`,
            (
              <ViewStructure
                view={view}
                uuid={uuid}
                kind={kind}
                viewKind={viewkind}
              ></ViewStructure>
            ),
            <IoConstructOutline> </IoConstructOutline>
          ),
        ];
      });
    }
  };

  return (
    <div>
      <MenuList>
        <MenuItem onClick={handleViewData}>
          <ListItemIcon>
            <ListIcon></ListIcon>
          </ListItemIcon>
          <ListItemText> View Data</ListItemText>
          <Typography
            variant="body2"
            sx={{ color: 'text.secondary' }}
          ></Typography>
        </MenuItem>

        <MenuItem onClick={handleSturcture}>
          <ListItemIcon>
            <IoConstructOutline
              style={{ fontSize: '1.2rem' }}
            ></IoConstructOutline>
          </ListItemIcon>
          <ListItemText> Structure </ListItemText>
        </MenuItem>
        <MenuItem
          onClick={() => {
            handleCopyToClipboard(view, showNotification);
          }}
        >
          <ListItemIcon>
            <ContentCopyIcon></ContentCopyIcon>
          </ListItemIcon>
          <ListItemText> Copy Name </ListItemText>
        </MenuItem>

        <Divider />
        <MenuItem
          onClick={() => {
            setShowExport(true);
          }}
        >
          <ListItemIcon>
            <ArrowCircleDownIcon></ArrowCircleDownIcon>
          </ListItemIcon>
          <ListItemText> Export </ListItemText>
        </MenuItem>
        <Divider />
        <MenuItem
          sx={{ color: 'error.main' }}
          onClick={() => {
            handleDrop();
          }}
        >
          <ListItemIcon>
            <DeleteForeverIcon color="error"></DeleteForeverIcon>
          </ListItemIcon>
          <ListItemText>Drop </ListItemText>
          <Typography
            variant="body2"
            sx={{ color: 'text.secondary' }}
          ></Typography>
        </MenuItem>
      </MenuList>

      {showExport && (
        <Modal
          open={showExport}
          onClose={(e: EscDownEvent, reason) => {
            if (reason === 'backdropClick') {
              return;
            }

            e.preventDefault();
            setShowExport(false);
          }}
        >
          <Box sx={modalStyle} width="45%">
            <ExportTableOrView
              uuid={uuid}
              name={view}
              kind={kind}
              supportKind={['csv', 'json', 'jsonline']}
            ></ExportTableOrView>
          </Box>
        </Modal>
      )}
    </div>
  );
}

export { ViewsTree, ViewKind };
