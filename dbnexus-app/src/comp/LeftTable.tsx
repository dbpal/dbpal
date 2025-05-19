import { ReactNode, useContext, useEffect, useState } from 'react';
import {
  modalStyle,
  progressBarSizeSmall,
  tabsHasId,
  DocsAndCount,
  makeAutoComplete,
  EscDownEvent,
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
import {
  Box,
  Button,
  CircularProgress,
  Divider,
  FormControlLabel,
  ListItemIcon,
  ListItemText,
  MenuItem,
  MenuList,
  Modal,
  Paper,
  Stack,
  Switch,
  TextField,
  Tooltip,
} from '@mui/material';

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
import { BSON, EJSON } from 'bson';
import {
  calMenuStyle,
  ConnTypeContext,
  ContextMenuContext,
  DataSourceKind,
  generateRandomString,
  secondConfirm,
  MatchPatternContext,
  TabsSetContext,
  UuidContext,
  handleCopyToClipboard,
} from './helper';
import { MenuRefresh } from './CommonComp';
import { TableStructure } from './TableStructure';

enum TablesType {
  Normal,
  MongoTimeSerial,
}

function TablesTree(props: { typ: TablesType }) {
  const { typ } = props;

  const { uuid } = useContext(UuidContext);
  const [tables, setTables] = useState<types.ListTable[]>([]);
  const { showNotification } = useContext(NotificationContext);
  const [loading, setLoading] = useState(false);

  const handleReload = () => {
    setLoading(true);
    let invokeName;
    switch (typ) {
      case TablesType.Normal:
        invokeName = 'list_table';
        break;
      case TablesType.MongoTimeSerial:
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

  let { setUpMenu } = useContext(ContextMenuContext);

  let tablesText;
  switch (typ) {
    case TablesType.Normal:
      tablesText = 'tables';
      break;
    case TablesType.MongoTimeSerial:
      tablesText = 'time serial';
      break;
  }
  const matchPattern = useContext(MatchPatternContext);
  return (
    <TreeItem
      itemId={uuid + '-tables-' + typ + '-'}
      label=<Stack
        spacing={1}
        direction="row"
        sx={{ alignItems: 'center' }}
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
        <TableViewIcon></TableViewIcon>
        <span> {tablesText} </span>
        {loading && (
          <CircularProgress size={progressBarSizeSmall}></CircularProgress>
        )}
      </Stack>
    >
      {tables
        .filter((e) => matchPattern(e.name))
        .map((e) => {
          return (
            <TableTree
              key={e.name}
              size={e.size}
              reloadTables={handleReload}
              table={e.name}
            ></TableTree>
          );
        })}
    </TreeItem>
  );
}

function TableTree(props: {
  table: string;
  reloadTables;
  size: string | null;
}) {
  let { table, reloadTables, size } = props;
  const { uuid } = useContext(UuidContext);
  const { kind } = useContext(ConnTypeContext);
  let { setUpMenu } = useContext(ContextMenuContext);

  return (
    <TreeItem
      itemId={uuid + '-table-' + table}
      label={
        <Stack
          direction="row"
          spacing={1}
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
              style: calMenuStyle(e, 200),
            });
          }}
        >
          <Stack direction="row" spacing={1}>
            <TableViewIcon> </TableViewIcon> <span>{table}</span>
          </Stack>
          <span> {size} </span>
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
      title: `are u sure to drop table '${table}'`,
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

  const suportCascade = (): boolean => {
    switch (kind) {
      case DataSourceKind.Mysql:
      case DataSourceKind.MariaDB:
      case DataSourceKind.TiDB:
      case DataSourceKind.Postgres:
      case DataSourceKind.Sqlite:
      case DataSourceKind.Cockroach:
      case DataSourceKind.LibSql:
      case DataSourceKind.Oracle:
        return true;
      default:
        return false;
    }
  };

  return (
    <Stack spacing={1}>
      {suportCascade() && (
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
      <Stack direction="row" spacing={1}>
        <Button
          loading={loading}
          color="error"
          variant="contained"
          onClick={handleDrop}
        >
          drop
        </Button>
      </Stack>
    </Stack>
  );
}

async function makeViewTable(
  uuid,
  table: string,
  kind: DataSourceKind,
  redisTableType: RedisTableType,
  showNotification
) {
  let tableList: ReactNode = null;
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
      tableList = (
        <SqlTableView
          autoCompletion={makeAutoComplete(uuid)}
          tableName={table}
          kind={kind}
          reloadOP={async (
            where: string,
            orderBy: string,
            page: number,
            pageSize: number,
            extra
          ) => {
            let where_null = where ? where : null;
            let orderBy_null = orderBy ? orderBy : null;
            return await invoke<types.MuiDataGridData>('show_table', {
              name: uuid,
              tableName: table,
              whereStr: where_null,
              orderBy: orderBy_null,
              page,
              pageSize,
              extra,
            });
          }}
          updateOP={async (wheres, values) => {
            return await invoke<types.SqlExecuteRet>('update', {
              name: uuid,
              tableName: table,
              values,
              wheres,
            });
          }}
          deleteOP={async (wheres) => {
            return await invoke<types.SqlExecuteRet>('delete', {
              name: uuid,
              tableName: table,
              wheres,
            });
          }}
          put_value_in_sql={async (typ, value) => {
            return invoke('put_value_in_sql', {
              name: uuid,
              typ,
              value: value ? value : null,
            });
          }}
          addOP={async (values) => {
            return invoke('insert', {
              name: uuid,
              tableName: table,
              values,
            });
          }}
        ></SqlTableView>
      );
      break;
    case DataSourceKind.Redis:
      switch (redisTableType) {
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
              content={await (async () => {
                try {
                  let x = await invoke<types.MuiDataGridData>('show_table', {
                    name: uuid,
                    tableName: table,
                    whereStr: null,
                    orderBy: null,
                    page: 0,
                    pageSize: 0,
                  });
                  return x.data[0]['value'];
                } catch (e) {
                  showNotification(
                    'success',
                    'load redis string table failed',
                    e
                  );
                }
              })()}
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
              modifyOP={async (old, newx) => {
                return await invoke('redis_set_replace', {
                  name: uuid,
                  key: table,
                  old,
                  new: newx,
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

      break;
    case DataSourceKind.Mongo:
      tableList = (
        <MongoTable
          reloadOPTableList={async (filter, sort, page, pageSize) => {
            return await invoke('mongo_show_collection_table_list', {
              name: uuid,
              tableName: table,
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
                tableName: table,
                filter: BSON.serialize(filter),
                sort: BSON.serialize(sort),
                page,
                pageSize,
              });
              //@ts-ignore
              let docs = x.docs.map((e) => BSON.deserialize(new Uint8Array(e)));
              //@ts-ignore
              let count = x.count;
              return new DocsAndCount(docs, count);
            } catch (e) {
              throw e;
            }
          }}
          modifyOP={async (doc) => {
            return invoke('mongo_update_one', {
              name: uuid,
              tableName: table,
              doc: BSON.serialize(doc),
            });
          }}
          loadOne={async (id) => {
            try {
              let x = await invoke<number[]>('mongo_get_one_document_by_id', {
                name: uuid,
                tableName: table,
                id: JSON.stringify(EJSON.serialize(id)),
              });
              return BSON.deserialize(new Uint8Array(x));
            } catch (e) {
              throw e;
            }
          }}
          deleteOP={async (ids) => {
            return invoke('mongo_delete_ids', {
              name: uuid,
              tableName: table,
              ids: JSON.stringify(EJSON.serialize(ids)),
            });
          }}
        ></MongoTable>
      );
      break;
  }
  return tableList;
}

async function makeAddTable(
  uuid,
  table: string,
  kind: DataSourceKind,
  redisTableType: RedisTableType
) {
  let tableList: ReactNode = null;
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
      tableList = (
        <SqlTableViewData
          buttonName="add"
          handleData={async (values) => {
            return invoke('insert', {
              name: uuid,
              tableName: table,
              values,
            });
          }}
          fields={await (async () => {
            return invoke('show_table_columns', {
              name: uuid,
              tableName: table,
            });
          })()}
        ></SqlTableViewData>
      );
      break;
    case DataSourceKind.Redis:
      switch (redisTableType) {
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
              content={''}
            ></RedisStringTable>
          );
          break;
        case RedisTableType.List:
          tableList = (
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
          break;

        case RedisTableType.Set:
          tableList = (
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
          break;
        case RedisTableType.Zset:
          tableList = (
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
          break;
        case RedisTableType.Hash:
          tableList = (
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
          break;
      }
      break;
    case DataSourceKind.Mongo:
      tableList = (
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
      break;
  }

  return tableList;
}

function TableMenu(props: {
  table: string;
  uuid: string;
  reload;
  kind: DataSourceKind;
  redisTableType?: RedisTableType;
}) {
  let { table, uuid, reload, kind, redisTableType } = props;
  const [showDup, setshowDup] = useState(false);
  const [showRename, setShowRename] = useState(false);
  const [showDrop, setShowDrop] = useState(false);
  const [showExport, setShowExport] = useState(false);

  const { showNotification } = useContext(NotificationContext);
  const { tabs, setTabs, setSelectTab } = useContext(TabsSetContext);

  const handleTruncateTable = async () => {
    const confirm = await secondConfirm({
      title: 'Truncate',
      confirmText: 'Truncate',
      content: '',
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

  const handleViewData = async () => {
    const id = uuid + '-table-' + generateRandomString(10);
    let tableList: ReactNode = await makeViewTable(
      uuid,
      table,
      kind,
      redisTableType,
      showNotification
    );
    setTabs((e) => {
      setSelectTab(tabs.length);
      return [
        ...e,
        new TabContent(id, table, tableList, <TableViewIcon> </TableViewIcon>),
      ];
    });
  };
  const handleAddData = async () => {
    const id = generateRandomString(10);
    let tableList: ReactNode = await makeAddTable(
      uuid,
      table,
      kind,
      redisTableType
    );
    setTabs((e) => {
      setSelectTab(tabs.length);
      return [
        ...e,
        new TabContent(
          id,
          `${table}*`,
          tableList,
          <TableViewIcon> </TableViewIcon>
        ),
      ];
    });
  };
  const handleStrucure = () => {
    let id = uuid + '-table-' + table + '-struct';
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

  const getExportKind = () => {
    switch (kind) {
      case DataSourceKind.Redis:
        return ['csv', 'sql', 'json', 'jsonline'];
      default:
        return ['csv', 'sql', 'json', 'jsonline'];
    }
  };

  return (
    <Paper>
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
        {kind != DataSourceKind.Redis && (
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
        {kind == DataSourceKind.Redis &&
          redisTableType != RedisTableType.String && (
            <MenuItem
              onClick={() => {
                handleAddData();
              }}
            >
              <ListItemIcon>
                <AddIcon></AddIcon>
              </ListItemIcon>
              <ListItemText> Add Data</ListItemText>
            </MenuItem>
          )}

        <MenuItem onClick={handleStrucure}>
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

        <Divider></Divider>

        <MenuItem
          onClick={() => {
            setshowDup(true);
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
        <Divider></Divider>
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
          onClose={(e: EscDownEvent, reason) => {
            if (reason === 'backdropClick') {
              return;
            }

            e.preventDefault();
            setshowDup(false);
          }}
        >
          <Box sx={modalStyle} width="45%">
            <DupTable
              table={table}
              uuid={uuid}
              handleClose={() => {
                setshowDup(false);
              }}
              reloadTables={reload}
            ></DupTable>
          </Box>
        </Modal>
      )}
      {showRename && (
        <Modal
          open={showRename}
          onClose={(e: EscDownEvent, reason) => {
            if (reason === 'backdropClick') {
              return;
            }

            e.preventDefault();
            setShowRename(false);
          }}
        >
          <Box sx={modalStyle} width="45%">
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
              name={table}
              handleClose={() => {
                setShowExport(false);
              }}
              supportKind={getExportKind()}
              kind={kind}
            ></ExportTableOrView>
          </Box>
        </Modal>
      )}

      {showDrop && (
        <Modal
          open={showDrop}
          onClose={(e: EscDownEvent, reason) => {
            if (reason === 'backdropClick') {
              return;
            }

            e.preventDefault();
            setShowDrop(false);
          }}
        >
          <Box sx={modalStyle} width="45%">
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
    </Paper>
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
        showNotification('success', 'Duplicate OK');
        reloadTables();
        handleClose();
      })
      .catch((e) => {
        showNotification('error', 'Duplicate failure', e);
      })
      .finally(() => {
        setLoading(false);
      });
  };

  return (
    <Stack spacing={1}>
      <TextField
        size="small"
        value={name}
        label="duplicate name"
        onChange={(e) => {
          setName(e.target.value);
        }}
      ></TextField>
      <Stack direction="row" spacing={1}>
        <Button
          loading={loading}
          color="primary"
          variant="contained"
          onClick={handleRename}
        >
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
      content: `are u sure going to rename table '${table}'`,
      confirmText: 'Rename',
    });
    if (!confirm) {
      return;
    }
    setLoading(true);
    invoke('rename_table', { name: uuid, old: table, new: newName })
      .then(() => {
        showNotification('success', 'Rename OK');
        reloadTables();
        handleClose();
      })
      .catch((e) => {
        showNotification('error', 'Rename failure', e);
      })
      .finally(() => {
        setLoading(false);
      });
  };

  return (
    <Stack spacing={1}>
      <TextField
        size="small"
        value={newName}
        label="New Name"
        onChange={(e) => {
          setNewName(e.target.value);
        }}
      ></TextField>
      <Stack direction="row" spacing={1}>
        <Button
          loading={loading}
          color="primary"
          variant="contained"
          onClick={handleRename}
        >
          rename
        </Button>
      </Stack>
    </Stack>
  );
}

function RedisTables(prpos: { schema?: string }) {
  const { schema } = prpos;

  const { kind } = useContext(ConnTypeContext);
  const { uuid } = useContext(UuidContext);
  const getPrefix = () => {
    return uuid + schema + '-table-';
  };
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
  let filterTreeID = getPrefix() + '-filter-';

  let stringTableTreePrefix = getPrefix() + '-table-string-';
  let listTableTreePrefix = getPrefix() + '-table-list-';
  let setTableTreePrefix = getPrefix() + '-table-set-';
  let zsetTableTreePrefix = getPrefix() + '-table-sset-';
  let hashTableTreePrefix = getPrefix() + '-table-hash-';

  const makeOneKind = (
    idPrefix: string,
    tables: string[],
    typ: RedisTableType
  ) => {
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
    return (
      <TreeItem
        itemId={idPrefix}
        label=<Stack
          spacing={1}
          direction="row"
          sx={{ alignItems: 'center' }}
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
          <span>{name}</span>
        </Stack>
      >
        {tables.map((table) => (
          <TreeItem
            key={table}
            itemId={idPrefix + table}
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
                  style: calMenuStyle(e, 200),
                });
              }}
            >
              <span> {table} </span>
            </Stack>
          ></TreeItem>
        ))}
      </TreeItem>
    );
  };

  let { setUpMenu } = useContext(ContextMenuContext);
  return (
    <>
      <TreeItem
        itemId={filterTreeID}
        label=<Stack
          direction="row"
          style={{ cursor: 'pointer' }}
          onClick={() => {
            setShowEditFilter(true);
          }}
        >
          {tables && tables.has_more ? (
            <Tooltip title={`result exceed ${max},some result are omited`}>
              <em style={{ color: 'red' }}> filter:{filter} </em>
            </Tooltip>
          ) : (
            <em> filter:{filter} </em>
          )}
          {loading && (
            <CircularProgress size={progressBarSizeSmall}></CircularProgress>
          )}
        </Stack>
      ></TreeItem>
      {tables &&
        makeOneKind(
          stringTableTreePrefix,
          tables.string_tables,
          RedisTableType.String
        )}
      {tables &&
        makeOneKind(
          listTableTreePrefix,
          tables.list_tables,
          RedisTableType.List
        )}
      {tables &&
        makeOneKind(setTableTreePrefix, tables.set_tables, RedisTableType.Set)}
      {tables &&
        makeOneKind(
          zsetTableTreePrefix,
          tables.zset_tables,
          RedisTableType.Zset
        )}
      {tables &&
        makeOneKind(
          hashTableTreePrefix,
          tables.hash_tables,
          RedisTableType.Hash
        )}
      {showEditFilter && (
        <Modal
          open={showEditFilter}
          onClose={(e: EscDownEvent, reason) => {
            if (reason === 'backdropClick') {
              return;
            }

            e.preventDefault();
            setShowEditFilter(false);
          }}
        >
          <Box sx={modalStyle} width="45%">
            <TextField
              size="small"
              value={filter}
              fullWidth
              label="filer"
              helperText="can be *key or key* etc..."
              onChange={(e) => {
                setFilter(e.target.value);
              }}
            ></TextField>
          </Box>
        </Modal>
      )}
    </>
  );
}

enum RedisTableType {
  String,
  List,
  Set,
  Zset,
  Hash,
}

export { TablesTree, TablesType, RedisTables };
