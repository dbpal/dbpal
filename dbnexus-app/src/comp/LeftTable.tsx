'use strict';
import { ReactNode, useContext, useEffect, useRef, useState } from 'react';
import {
  modalStyle,
  tabsHasId,
  EscDownEvent,
  modalHandleClose,
  makeExtractTableSelectSort,
  makeExtractTableSelectWhere,
  makeShowTableFn,
  makeTableUpdate,
  makeTableDelete,
  makeInvalidCassandraTableCache,
  makePutValueInSql,
  makeShowTableColumn,
  makeTableInsertFn,
} from './Common';
import types from './ApiStruct';
import { NotificationContext } from './Notification';
import { invoke } from '@tauri-apps/api/core';
import { TreeItem } from '@mui/x-tree-view/TreeItem';
import ListIcon from '@mui/icons-material/List';
import EditIcon from '@mui/icons-material/Edit';
import DeleteForeverIcon from '@mui/icons-material/DeleteForever';
import ContentCopyIcon from '@mui/icons-material/ContentCopy';
import PlaylistRemoveIcon from '@mui/icons-material/PlaylistRemove';
import TableViewIcon from '@mui/icons-material/TableView';
import ArrowCircleDownIcon from '@mui/icons-material/ArrowCircleDown';
import { IoConstructOutline } from 'react-icons/io5';
import { HiOutlineDuplicate } from 'react-icons/hi';
import FilterAltIcon from '@mui/icons-material/FilterAlt';

import {
  Box,
  Button,
  Chip,
  CircularProgress,
  Divider,
  FormControlLabel,
  IconButton,
  InputAdornment,
  ListItemIcon,
  ListItemText,
  MenuItem,
  MenuList,
  Modal,
  Stack,
  Switch,
  TextField,
  Tooltip,
} from '@mui/material';
import ClearIcon from '@mui/icons-material/Clear';

import { TabContent } from './Tabs';
import { SqlTableView, SqlTableViewData } from './SqlTableView';
import { ExportTableOrView } from './Export';
import AddIcon from '@mui/icons-material/Add';
import {
  RedisAddHash,
  RedisAddList,
  RedisAddSet,
  RedisAddZset,
  RedisHashTable,
  RedisListTable,
  RedisSetTable,
  RedisStringTable,
  RedisZsetTable,
} from './Redis';
import { MongoTable, MongoTableAdd } from './MongoDB';
import { BSON } from 'bson';
import {
  ConnTypeContext,
  ContextMenuContext,
  DataSourceKind,
  generateRandomString,
  secondConfirm,
  MatchPatternContext,
  TabsSetContext,
  UuidContext,
  handleCopyToClipboard,
  MongoTableHandler,
  getSupportExportKind,
  MuiDataGridData,
  SettingContext,
  RedisTableType,
  SomeTempStore,
  NewWindowTableParameter,
  TableType,
  shrinkAble,
} from './helper';
import { ExceedMaxTreeItem, MenuRefresh, ModalHeader } from './CommonComp';
import { TableStructure } from './TableStructure';
import { WebviewWindow } from '@tauri-apps/api/webviewWindow';

function TablesTree(props: { typ: TableType }) {
  const { typ } = props;
  const { uuid } = useContext(UuidContext);
  const [tables, setTables] = useState<types.ListTable[]>([]);
  const { showNotification } = useContext(NotificationContext);
  const [loading, setLoading] = useState(false);
  const [exceed, setExceed] = useState(0);
  const matchPattern = useContext(MatchPatternContext);
  const setting = useContext(SettingContext);
  useEffect(() => {
    setExceed(tables.filter((e) => matchPattern(e.name)).length);
  }, [tables, matchPattern]);
  const handleReload = () => {
    setLoading(true);
    let invokeName: string;
    switch (typ) {
      case TableType.Normal:
        invokeName = 'list_table';
        break;
      case TableType.MongoTimeSerial:
        invokeName = 'mongo_list_time_serial_collection';
        break;
    }

    invoke<types.ListTable[]>(invokeName, { name: uuid })
      .then((ts) => {
        setTables(ts);
      })
      .catch((e) => {
        showNotification('error', 'list table failed', e);
      })
      .finally(() => {
        setLoading(false);
      });
  };
  useEffect(() => {
    handleReload();
  }, []);

  const { setUpMenu } = useContext(ContextMenuContext);

  let tablesText;
  switch (typ) {
    case TableType.Normal:
      tablesText = 'tables';
      break;
    case TableType.MongoTimeSerial:
      tablesText = 'time serial';
      break;
  }
  const tablesId = useRef(generateRandomString());
  return (
    <TreeItem
      itemId={tablesId.current}
      label=<Stack
        direction="row"
        sx={{ alignItems: 'center' }}
        onContextMenu={(e) => {
          setUpMenu({
            content: (
              <MenuList>
                <MenuRefresh reload={handleReload}></MenuRefresh>
              </MenuList>
            ),
            event: e,
          });
        }}
      >
        <TableViewIcon></TableViewIcon>
        <span> {tablesText} </span>
        {loading && <CircularProgress></CircularProgress>}
      </Stack>
    >
      {tables
        .filter((e) => matchPattern(e.name))
        .map((e, index) => {
          if (index >= setting.appearance.maxTreeSize) {
            return;
          }
          return (
            <TableTree
              key={e.name}
              size={e.size}
              reloadTables={handleReload}
              table={e.name}
            ></TableTree>
          );
        })}
      <ExceedMaxTreeItem
        exceed={exceed}
        maxTreeSize={setting.appearance.maxTreeSize}
      ></ExceedMaxTreeItem>
    </TreeItem>
  );
}

function TableTree(props: {
  table: string;
  reloadTables;
  size: string | null;
}) {
  const { table, reloadTables, size } = props;
  const { uuid } = useContext(UuidContext);
  const { kind } = useContext(ConnTypeContext);
  const { setUpMenu } = useContext(ContextMenuContext);

  return (
    <TreeItem
      itemId={uuid + '-table-' + table}
      label={
        <Stack
          direction="row"
          justifyContent="space-between"
          onContextMenu={(e) => {
            setUpMenu({
              content: (
                <TableMenu
                  kind={kind}
                  reload={reloadTables}
                  table={table}
                  uuid={uuid}
                />
              ),
              event: e,
            });
          }}
        >
          <Stack direction="row" style={{ alignItems: 'center' }}>
            <TableViewIcon> </TableViewIcon> <span>{table}</span>
          </Stack>
          <span style={{ ...shrinkAble, minWidth: 0 }}> {size} </span>
        </Stack>
      }
    ></TreeItem>
  );
}

function DropTable(props: {
  table: string;
  reloadTables;
  uuid: string;
  handleClose;
  kind: DataSourceKind;
}) {
  let { table, reloadTables, uuid, handleClose, kind } = props;
  const [loading, setLoading] = useState(false);
  const { showNotification } = useContext(NotificationContext);
  const [cascade, setCascade] = useState(false);

  const handleDrop = async () => {
    const confirm = await secondConfirm({
      confirmText: 'Drop',
      title: `drop table '${table}'`,
      kind: 'error',
    });
    if (!confirm) {
      return;
    }
    setLoading(true);
    invoke('drop_table', { name: uuid, tableName: table, cascade })
      .then(() => {
        showNotification('success', 'Drop table OK');
        reloadTables();
        handleClose();
      })
      .catch((e) => {
        showNotification('error', 'Drop table failure', e);
      })
      .finally(() => {
        setLoading(false);
      });
  };

  const supportCascade = (): boolean => {
    switch (kind) {
      case DataSourceKind.Mysql:
      case DataSourceKind.MariaDB:
      case DataSourceKind.TiDB:
      case DataSourceKind.Postgres:
      case DataSourceKind.Cockroach:
      case DataSourceKind.Sqlite:
      case DataSourceKind.LibSql:
      case DataSourceKind.Oracle:
        return true;
      default:
        return false;
    }
  };

  return (
    <Stack>
      {supportCascade() && (
        <FormControlLabel
          control={
            <Switch
              checked={cascade}
              onChange={(e) => setCascade(e.target.checked)}
            />
          }
          label="CASCADE"
        />
      )}
      <Stack direction="row">
        <Button loading={loading} color="error" onClick={handleDrop}>
          drop
        </Button>
      </Stack>
    </Stack>
  );
}

function makeRedisViewTable(uuid, table, redisType: RedisTableType) {
  let tableList;
  switch (redisType) {
    case RedisTableType.String:
      tableList = (
        <RedisStringTable
          modifyOP={async (value) => {
            return await invoke('redis_string_set', {
              name: uuid,
              key: table,
              value,
            });
          }}
          loadContent={async () => {
            return invoke<MuiDataGridData>('show_table', {
              name: uuid,
              tableName: table,
              whereStr: null,
              orderBy: null,
              page: 0,
              pageSize: 0,
            }).then((x) => {
              return x.data[0]['result'];
            });
          }}
        ></RedisStringTable>
      );
      break;
    case RedisTableType.List:
      tableList = (
        <RedisListTable
          reloadOP={async () => {
            return await invoke('show_table', {
              name: uuid,
              tableName: table,
              page: 0,
              pageSize: 0,
              whereStr: null,
              orderBy: null,
            });
          }}
          modifyOP={async (index, value) => {
            return await invoke('redis_list_set', {
              name: uuid,
              key: table,
              value,
              index,
            });
          }}
          delOP={async (count, value) => {
            return await invoke('redis_list_remove', {
              name: uuid,
              key: table,
              value,
              count,
            });
          }}
        ></RedisListTable>
      );
      break;

    case RedisTableType.Set:
      tableList = (
        <RedisSetTable
          reloadOP={async (keyMatch) => {
            return await invoke('show_table', {
              name: uuid,
              tableName: table,
              page: 0,
              pageSize: 0,
              whereStr: keyMatch ? keyMatch : null,
              orderBy: null,
            });
          }}
          modifyOP={async (old, newValue) => {
            return await invoke('redis_set_replace', {
              name: uuid,
              key: table,
              old,
              new: newValue,
            });
          }}
          delOP={async (member) => {
            return await invoke('redis_set_del', {
              name: uuid,
              key: table,
              member,
            });
          }}
        ></RedisSetTable>
      );
      break;
    case RedisTableType.Zset:
      tableList = (
        <RedisZsetTable
          reloadOP={async (keyMatch, min, max) => {
            return await invoke('redis_zset_show_table', {
              name: uuid,
              key: table,
              min: typeof min == 'number' ? min : null,
              max: typeof max == 'number' ? max : null,
              whereStr: keyMatch ? keyMatch : null,
            });
          }}
          modifyOP={async (
            oldMember: string,
            newKey: string,
            newValue: number
          ) => {
            return await invoke('redis_zset_replace', {
              name: uuid,
              key: table,
              oldMember: oldMember,
              newMember: newKey,
              newValue,
            });
          }}
          delOP={async (member) => {
            return await invoke('redis_zset_del', {
              name: uuid,
              key: table,
              member,
            });
          }}
        ></RedisZsetTable>
      );
      break;
    case RedisTableType.Hash:
      tableList = (
        <RedisHashTable
          reloadOP={async (keyMatch) => {
            return await invoke('show_table', {
              name: uuid,
              tableName: table,
              page: 0,
              pageSize: 0,
              whereStr: keyMatch ? keyMatch : null,
              orderBy: null,
            });
          }}
          delOP={async (member) => {
            return await invoke('redis_hash_del', {
              name: uuid,
              key: table,
              member,
            });
          }}
          modifyOP={async (oldMember, newMember, newValue) => {
            return await invoke('redis_hash_replace', {
              name: uuid,
              key: table,
              oldMember,
              newMember,
              newValue,
            });
          }}
        ></RedisHashTable>
      );
      break;
  }
  return tableList;
}

function makeViewTable(
  uuid,
  table: string,
  kind: DataSourceKind,
  redisTableType: RedisTableType
) {
  let tableList: ReactNode = null;
  switch (kind) {
    case DataSourceKind.MariaDB:
    case DataSourceKind.Mysql:
    case DataSourceKind.SqlServer:
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
      tableList = (
        <SqlTableView
          isTable
          extractTableSelectSort={makeExtractTableSelectSort(uuid)}
          extractTableSelectWhere={makeExtractTableSelectWhere(uuid)}
          invalidCassandraTableViewCache={makeInvalidCassandraTableCache(
            uuid,
            table
          )}
          tableName={table}
          uuid={uuid}
          kind={kind}
          reloadOP={makeShowTableFn(uuid, table)}
          updateOP={makeTableUpdate(uuid, table)}
          deleteOP={makeTableDelete(uuid, table)}
          put_value_in_sql={makePutValueInSql(uuid)}
          addOP={makeTableInsertFn(uuid, table)}
        ></SqlTableView>
      );
      break;
    case DataSourceKind.Redis:
      tableList = makeRedisViewTable(uuid, table, redisTableType);
      break;
    case DataSourceKind.Mongo:
      tableList = (
        <MongoTable
          reloadOPTableList={MongoTableHandler.reloadTableList(uuid, table)}
          reloadOPDocs={MongoTableHandler.reloadDocs(uuid, table)}
          modifyOP={MongoTableHandler.modifyOP(uuid, table)}
          loadOne={MongoTableHandler.loadOne(uuid, table)}
          deleteOP={MongoTableHandler.deleteOP(uuid, table)}
        ></MongoTable>
      );
      break;
  }
  return tableList;
}

function AddTableView(props: {
  uuid;
  table: string;
  kind: DataSourceKind;
  redisTableType: RedisTableType;
}) {
  const { uuid, table, kind, redisTableType } = props;
  const [fields, setFields] = useState<types.Field[]>([]);
  const { showNotification } = useContext(NotificationContext);
  switch (kind) {
    case DataSourceKind.Redis:
      switch (redisTableType) {
        case RedisTableType.String:
          return (
            <RedisStringTable
              modifyOP={async (value) => {
                return await invoke('redis_string_set', {
                  name: uuid,
                  key: table,
                  value,
                });
              }}
              loadContent={async () => {
                return Promise.resolve('');
              }}
            ></RedisStringTable>
          );

        case RedisTableType.List:
          return (
            <RedisAddList
              lpushOP={async (value) => {
                return await invoke('redis_list_lpush', {
                  name: uuid,
                  key: table,
                  value,
                });
              }}
              rpushOP={async (value) => {
                return await invoke('redis_list_rpush', {
                  name: uuid,
                  key: table,
                  value,
                });
              }}
            ></RedisAddList>
          );

        case RedisTableType.Set:
          return (
            <RedisAddSet
              addOP={async (member) => {
                return await invoke('redis_set_add', {
                  name: uuid,
                  key: table,
                  member,
                });
              }}
            ></RedisAddSet>
          );

        case RedisTableType.Zset:
          return (
            <RedisAddZset
              addOP={async (member, value) => {
                return await invoke('redis_zset_add', {
                  name: uuid,
                  key: table,
                  member,
                  value,
                });
              }}
            ></RedisAddZset>
          );

        case RedisTableType.Hash:
          return (
            <RedisAddHash
              addOP={async (member, value) => {
                return await invoke('redis_hash_add', {
                  name: uuid,
                  key: table,
                  member,
                  value,
                });
              }}
            ></RedisAddHash>
          );
      }

    case DataSourceKind.Mongo:
      return (
        <MongoTableAdd
          addOP={async (data) => {
            return invoke<void>('mongo_insert', {
              name: uuid,
              tableName: table,
              doc: BSON.serialize(data),
            });
          }}
        ></MongoTableAdd>
      );
  }

  useEffect(() => {
    makeShowTableColumn(uuid, table)()
      .then((e) => {
        setFields(e);
      })
      .catch((e) => {
        showNotification('error', 'list fields failed', e);
      });
  }, []);

  return (
    <SqlTableViewData
      isAdd
      buttonName="add"
      handleData={makeTableInsertFn(uuid, table)}
      fields={fields}
    ></SqlTableViewData>
  );
}

function TableMenu(props: {
  table: string;
  uuid: string;
  reload;
  kind: DataSourceKind;
  redisTableType?: RedisTableType;
}) {
  let { table, uuid, reload, kind, redisTableType } = props;
  const [showDup, setShowDup] = useState(false);
  const [showRename, setShowRename] = useState(false);
  const [showDrop, setShowDrop] = useState(false);
  const [showExport, setShowExport] = useState(false);

  const { showNotification } = useContext(NotificationContext);
  const { tabs, setTabs, setSelectTab } = useContext(TabsSetContext);

  const handleTruncateTable = async () => {
    const confirm = await secondConfirm({
      title: `Truncate '${table}'`,
      confirmText: 'Truncate',
      content: '',
      kind: 'error',
    });
    if (!confirm) {
      return;
    }
    invoke('truncate_table', { tableName: table, name: uuid })
      .then(() => {
        showNotification('success', `Truncate table OK`);
      })
      .catch((e) => {
        showNotification('error', `Truncate  table failure`, e);
      });
  };

  const handleviewDataNewWindow = () => {
    SomeTempStore.set({
      uuid,
      table,
      kind,
      redisTableType: redisTableType,
      isTable: true,
    } as NewWindowTableParameter);
    const webview = new WebviewWindow(generateRandomString(), {
      title: table,
      url: 'table_view.html',
      width: 800,
      height: 600,
      maximized: true,
    });
    webview.once('tauri://created', () =>
      console.info('create table view successfully')
    );
    webview.once('tauri://error', (e) => {
      showNotification('error', 'create webview failed', e.event.toString());
    });
  };

  const handleViewData = () => {
    const id = uuid + '-table-' + generateRandomString(10);
    let tableList: ReactNode = makeViewTable(uuid, table, kind, redisTableType);
    setTabs((e) => {
      setTimeout(() => {
        setSelectTab(tabs.length);
      }, 100);

      return [
        ...e,
        new TabContent(id, table, tableList, <TableViewIcon> </TableViewIcon>),
      ];
    });
  };
  const handleAddData = async () => {
    const id = generateRandomString(10);

    setTabs((e) => {
      setTimeout(() => {
        setSelectTab(tabs.length);
      }, 100);
      return [
        ...e,
        new TabContent(
          id,
          `${table}*`,
          (
            <AddTableView
              uuid={uuid}
              kind={kind}
              table={table}
              redisTableType={redisTableType}
            ></AddTableView>
          ),
          <TableViewIcon> </TableViewIcon>
        ),
      ];
    });
  };
  const handleAddDataNewWindow = async () => {
    const id = generateRandomString(10);
    SomeTempStore.set({
      uuid,
      table,
      kind,
      redisTableType: redisTableType,
    } as NewWindowTableParameter);
    const webview = new WebviewWindow(id, {
      title: 'table insert',
      url: 'table_add.html',
      width: 800,
      height: 600,
      maximized: true,
    });
    webview.once('tauri://created', () =>
      console.info('create add connection view successfully')
    );
    webview.once('tauri://error', (e) => {
      showNotification('error', 'create webview failed', e.event.toString());
    });
  };

  const handleStructure = () => {
    const id = uuid + '-table-' + table + '-struct';
    const index = tabsHasId(tabs, id);
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
            `${table}`,
            (
              <TableStructure
                kind={kind}
                table={table}
                uuid={uuid}
              ></TableStructure>
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
        <MenuItem
          onClick={() => {
            handleViewData();
          }}
        >
          <ListItemIcon>
            <ListIcon></ListIcon>
          </ListItemIcon>
          <ListItemText> View Data</ListItemText>
        </MenuItem>

        <MenuItem
          onClick={() => {
            handleviewDataNewWindow();
          }}
        >
          <ListItemIcon>
            <ListIcon></ListIcon>
          </ListItemIcon>
          <ListItemText> View Data(new window)</ListItemText>
        </MenuItem>

        {redisTableType != RedisTableType.String && (
          <MenuItem
            onClick={() => {
              handleAddData();
            }}
          >
            <ListItemIcon>
              <AddIcon></AddIcon>
            </ListItemIcon>
            <ListItemText> Add Data </ListItemText>
          </MenuItem>
        )}

        {redisTableType != RedisTableType.String && (
          <MenuItem
            onClick={() => {
              handleAddDataNewWindow();
            }}
          >
            <ListItemIcon>
              <AddIcon></AddIcon>
            </ListItemIcon>
            <ListItemText> Add Data(new window) </ListItemText>
          </MenuItem>
        )}

        <MenuItem onClick={handleStructure}>
          <ListItemIcon>
            <IoConstructOutline
              style={{ fontSize: '1.2rem' }}
            ></IoConstructOutline>
          </ListItemIcon>
          <ListItemText> Structure </ListItemText>
        </MenuItem>

        <MenuItem
          onClick={() => {
            handleCopyToClipboard(table, showNotification);
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
            setShowDup(true);
          }}
        >
          <ListItemIcon>
            <HiOutlineDuplicate
              style={{ fontSize: '1.7rem' }}
            ></HiOutlineDuplicate>
          </ListItemIcon>
          <ListItemText> Duplicate </ListItemText>
        </MenuItem>
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
        <MenuItem
          onClick={() => {
            setShowRename(true);
          }}
        >
          <ListItemIcon>
            <EditIcon></EditIcon>
          </ListItemIcon>
          <ListItemText> Rename </ListItemText>
        </MenuItem>
        <Divider />
        <MenuItem
          sx={{ color: 'error.main' }}
          onClick={() => {
            handleTruncateTable();
          }}
        >
          <ListItemIcon>
            <PlaylistRemoveIcon color="error"></PlaylistRemoveIcon>
          </ListItemIcon>
          <ListItemText> Truncate </ListItemText>
        </MenuItem>
        <MenuItem
          sx={{ color: 'error.main' }}
          onClick={() => {
            setShowDrop(true);
          }}
        >
          <ListItemIcon>
            <DeleteForeverIcon color="error"></DeleteForeverIcon>
          </ListItemIcon>
          <ListItemText>Drop</ListItemText>
        </MenuItem>
      </MenuList>
      {showDup && (
        <Modal
          open={showDup}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setShowDup(false);
          })}
        >
          <Box sx={modalStyle} width="45%">
            <ModalHeader
              title="Duplicate"
              handleClose={() => {
                setShowDup(false);
              }}
            ></ModalHeader>
            <DupTable
              table={table}
              uuid={uuid}
              handleClose={() => {
                setShowDup(false);
              }}
              reloadTables={reload}
            ></DupTable>
          </Box>
        </Modal>
      )}
      {showRename && (
        <Modal
          open={showRename}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setShowRename(false);
          })}
        >
          <Box sx={modalStyle} width="45%">
            <ModalHeader
              handleClose={() => {
                setShowRename(false);
              }}
              title="Rename"
            ></ModalHeader>
            <RenameTable
              uuid={uuid}
              table={table}
              reloadTables={reload}
              handleClose={() => {
                setShowRename(false);
              }}
            ></RenameTable>
          </Box>
        </Modal>
      )}
      {showExport && (
        <Modal
          open={showExport}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setShowExport(false);
          })}
        >
          <Box sx={modalStyle} width="45%">
            <ExportTableOrView
              handleClose={() => {
                setShowExport(false);
              }}
              uuid={uuid}
              name={table}
              supportKind={getSupportExportKind(kind, true)}
            ></ExportTableOrView>
          </Box>
        </Modal>
      )}

      {showDrop && (
        <Modal
          open={showDrop}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setShowDrop(false);
          })}
        >
          <Box sx={modalStyle} width="45%">
            <ModalHeader
              handleClose={() => {
                setShowDrop(false);
              }}
              title="Drop Table"
            ></ModalHeader>
            <DropTable
              uuid={uuid}
              table={table}
              reloadTables={reload}
              handleClose={() => {
                setShowDrop(false);
              }}
              kind={kind}
            ></DropTable>
          </Box>
        </Modal>
      )}
    </div>
  );
}

function DupTable(props: {
  table: string;
  handleClose;
  uuid: string;
  reloadTables: () => void;
}) {
  const { table, handleClose, uuid, reloadTables } = props;
  const [loading, setLoading] = useState(false);
  const [name, setName] = useState(table);
  const { showNotification } = useContext(NotificationContext);
  const handleRename = () => {
    if (name == table) {
      showNotification('error', 'name not changed');
      return;
    }
    setLoading(true);
    invoke('duplicate_table', { name: uuid, from: table, to: name })
      .then(() => {
        showNotification('success', 'duplicate OK');
        reloadTables();
        handleClose();
      })
      .catch((e) => {
        showNotification('error', 'duplicate failure', e);
      })
      .finally(() => {
        setLoading(false);
      });
  };

  return (
    <Stack>
      <TextField
        value={name}
        label="duplicate name"
        onChange={(e) => {
          setName(e.target.value);
        }}
      ></TextField>
      <Stack direction="row">
        <Button loading={loading} color="primary" onClick={handleRename}>
          duplicate
        </Button>
      </Stack>
    </Stack>
  );
}

function RenameTable(props: {
  table: string;
  handleClose;
  uuid: string;
  reloadTables: () => void;
}) {
  const { table, handleClose, uuid, reloadTables } = props;
  const [loading, setLoading] = useState(false);
  const [newName, setNewName] = useState(table);
  const { showNotification } = useContext(NotificationContext);
  const handleRename = async () => {
    if (newName == table) {
      showNotification('error', 'name not changed');
      return;
    }
    const confirm = await secondConfirm({
      title: 'rename table',
      content: `rename table '${table}'`,
      confirmText: 'Rename',
    });
    if (!confirm) {
      return;
    }
    setLoading(true);
    invoke('rename_table', { name: uuid, old: table, new: newName })
      .then(() => {
        showNotification('success', 'rename OK');
        reloadTables();
        handleClose();
      })
      .catch((e) => {
        showNotification('error', 'rename failure', e);
      })
      .finally(() => {
        setLoading(false);
      });
  };

  return (
    <Stack>
      <TextField
        value={newName}
        label="New Name"
        onChange={(e) => {
          setNewName(e.target.value);
        }}
      ></TextField>
      <Stack direction="row">
        <Button loading={loading} color="primary" onClick={handleRename}>
          rename
        </Button>
      </Stack>
    </Stack>
  );
}

function RedisOneTableKind(props: {
  tables: string[];
  typ: RedisTableType;
  kind: DataSourceKind;
  handleReload;
  uuid;
}) {
  const { tables, typ, kind, handleReload, uuid } = props;
  let name;
  switch (typ) {
    case RedisTableType.String:
      name = 'string';
      break;
    case RedisTableType.List:
      name = 'list';
      break;
    case RedisTableType.Set:
      name = 'set';
      break;
    case RedisTableType.Zset:
      name = 'zset';
      break;
    case RedisTableType.Hash:
      name = 'hash';
      break;
  }
  const tablesID = useRef(generateRandomString());
  const { setUpMenu } = useContext(ContextMenuContext);
  return (
    <TreeItem
      itemId={tablesID.current}
      label=<Stack
        direction="row"
        sx={{ alignItems: 'center' }}
        onContextMenu={(e) => {
          setUpMenu({
            content: (
              <MenuList>
                <MenuRefresh reload={handleReload}></MenuRefresh>
              </MenuList>
            ),
            event: e,
          });
        }}
      >
        <span>{name}</span>
      </Stack>
    >
      {tables.map((table) => (
        <TreeItem
          key={table}
          itemId={uuid + '-key-' + name + '-' + table}
          label=<Stack
            direction="row"
            onContextMenu={(e) => {
              setUpMenu({
                content: (
                  <TableMenu
                    kind={kind}
                    reload={handleReload}
                    table={table}
                    uuid={uuid}
                    redisTableType={typ}
                  />
                ),
                event: e,
              });
            }}
          >
            <span> {table} </span>
          </Stack>
        ></TreeItem>
      ))}
    </TreeItem>
  );
}

function RedisTables() {
  const { kind } = useContext(ConnTypeContext);
  const { uuid } = useContext(UuidContext);
  const [filter, setFilter] = useState('');
  const [tables, setTables] = useState<types.RedisListTable>();
  const [loading, setLoading] = useState(false);
  const { showNotification } = useContext(NotificationContext);
  const max = 50;

  const handleReload = () => {
    setLoading(true);
    invoke<types.RedisListTable>('redis_list_table', {
      name: uuid,
      filter: filter ? filter : null,
      max,
    })
      .then((e) => {
        setTables(e);
      })
      .catch((e) => {
        showNotification('error', 'list redis table failed', e);
      })
      .finally(() => {
        setLoading(false);
      });
  };
  useEffect(() => {
    handleReload();
  }, []);
  useEffect(() => {
    handleReload();
  }, [filter]);

  const [showEditFilter, setShowEditFilter] = useState(false);
  const filterTreeID = useRef(generateRandomString());
  return (
    <>
      <TreeItem
        itemId={filterTreeID.current}
        label=<Stack
          direction="row"
          style={{ cursor: 'pointer' }}
          onClick={() => {
            setShowEditFilter(true);
          }}
        >
          {tables && tables.has_more ? (
            <Tooltip title={`result exceed ${max},some result are omitted`}>
              <Chip
                color="error"
                icon={<FilterAltIcon />}
                label={`${filter}`}
                variant="outlined"
              />
            </Tooltip>
          ) : (
            <Chip icon={<FilterAltIcon />} label={`${filter}`} />
          )}
          {loading && <CircularProgress></CircularProgress>}
        </Stack>
      ></TreeItem>
      {tables && (
        <>
          <RedisOneTableKind
            kind={kind}
            typ={RedisTableType.String}
            tables={tables.string_tables}
            handleReload={handleReload}
            uuid={uuid}
          ></RedisOneTableKind>
          <RedisOneTableKind
            kind={kind}
            typ={RedisTableType.List}
            tables={tables.list_tables}
            handleReload={handleReload}
            uuid={uuid}
          ></RedisOneTableKind>
          <RedisOneTableKind
            kind={kind}
            typ={RedisTableType.Set}
            tables={tables.set_tables}
            handleReload={handleReload}
            uuid={uuid}
          ></RedisOneTableKind>
          <RedisOneTableKind
            kind={kind}
            typ={RedisTableType.Zset}
            tables={tables.zset_tables}
            handleReload={handleReload}
            uuid={uuid}
          ></RedisOneTableKind>
          <RedisOneTableKind
            kind={kind}
            typ={RedisTableType.Hash}
            tables={tables.hash_tables}
            handleReload={handleReload}
            uuid={uuid}
          ></RedisOneTableKind>
        </>
      )}

      {showEditFilter && (
        <Modal
          open={showEditFilter}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setShowEditFilter(false);
          })}
        >
          <Box sx={modalStyle} width="45%">
            <ModalHeader
              handleClose={() => {
                setShowEditFilter(false);
              }}
              title="Edit key match string"
            ></ModalHeader>
            <TextField
              value={filter}
              fullWidth
              label="filer"
              helperText="key filter , can be '*key' or 'key*' etc..."
              onChange={(e) => {
                setFilter(e.target.value);
              }}
              slotProps={{
                input: {
                  endAdornment: filter && (
                    <InputAdornment position="end">
                      <IconButton
                        onClick={() => {
                          setFilter('');
                        }}
                        edge="end"
                      >
                        <ClearIcon fontSize="small" />
                      </IconButton>
                    </InputAdornment>
                  ),
                },
              }}
            ></TextField>
          </Box>
        </Modal>
      )}
    </>
  );
}

export { TablesTree, RedisTables, makeRedisViewTable, AddTableView };
