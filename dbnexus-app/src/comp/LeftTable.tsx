import { ReactNode, useContext, useEffect, useState } from 'react';
import {
  MenuContext,
  MenuRefresh,
  modalStyle,
  UuidContext,
  progressBarSizeSmall,
  tabsHasId,
  TabsSetContext,
  ConnTypeContext,
  progressBarSize,
  DocsAndCount,
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
import CloseIcon from '@mui/icons-material/Close';
import { FaListOl } from 'react-icons/fa';
import TextFieldsIcon from '@mui/icons-material/TextFields';
import {
  Box,
  Button,
  Checkbox,
  CircularProgress,
  Divider,
  FormControl,
  FormControlLabel,
  InputLabel,
  ListItemIcon,
  ListItemText,
  MenuItem,
  MenuList,
  Modal,
  Paper,
  Select,
  Stack,
  Switch,
  TextField,
} from '@mui/material';

import { TabContent } from './Tabs';
import { SqlTableView } from './SqlTableView';
import { TriggersTree } from './LeftTriggers';
import { ExportTableOrView } from './Export';
import AddIcon from '@mui/icons-material/Add';
import { LeftIndexes } from './LeftTableIndexes';
import {
  RedisHashTable,
  RedisListTable,
  RedisSetTable,
  RedisStringTable,
  RedisZsetTable,
} from './Redis';
import { MongoTable } from './MongoDB';
import { BSON, EJSON } from 'bson';
import { calMenuStyle, DataSourceKind, secondConfirm } from './helper';

function TablesTree() {
  const { uuid } = useContext(UuidContext);
  const [tables, setTables] = useState<types.ListTable[]>([]);
  const { showNotification } = useContext(NotificationContext);
  const [loading, setLoading] = useState(false);

  const handleReload = () => {
    setLoading(true);
    invoke<types.ListTable[]>('list_table', { name: uuid })
      .then((ts) => {
        setLoading(false);
        setTables(ts);
      })
      .catch((e) => {
        setLoading(false);
        showNotification('error', 'list table failed', e);
      });
  };
  useEffect(() => {
    handleReload();
  }, []);

  let { setUpMenu } = useContext(MenuContext);
  return (
    <TreeItem
      itemId={uuid + '-tables'}
      label=<Stack
        spacing={1}
        direction="row"
        sx={{ alignItems: 'flex-start' }}
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
        <span> tables </span>
        {loading && (
          <CircularProgress size={progressBarSizeSmall}></CircularProgress>
        )}
      </Stack>
    >
      {tables.map((e) => {
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
  let { setUpMenu } = useContext(MenuContext);
  let support_trigger =
    kind == DataSourceKind.MariaDB ||
    kind == DataSourceKind.MsSql ||
    kind == DataSourceKind.Mysql ||
    kind == DataSourceKind.Sqlite ||
    kind == DataSourceKind.Postgres ||
    kind == DataSourceKind.Oracle;

  return (
    <TreeItem
      itemId={uuid + '-table-' + table}
      label={
        <Stack
          direction="row"
          spacing={1}
          justifyContent="space-between"
          alignContent="center"
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
    >
      <TableColumnsTree table={table}></TableColumnsTree>
      {support_trigger && <TriggersTree table={table}></TriggersTree>}
      <LeftIndexes table={table}></LeftIndexes>
    </TreeItem>
  );
}

function TableTypesButton(props: { handleChange; uuid }) {
  let { handleChange, uuid } = props;
  const [types, setTypes] = useState([]);
  const [show, setShow] = useState(false);
  useEffect(() => {
    invoke<string[]>('list_data_types', { name: uuid })
      .then((ts) => {
        setTypes(ts);
      })
      .catch((e) => {
        console.log('should not return a error', e);
      });
  }, []);
  const toggleShow = () => {
    setShow((e) => !e);
  };
  return (
    <Stack direction="row" spacing={1} style={{ flexWrap: 'wrap' }}>
      <Button size="small" onClick={toggleShow}>
        {show && <CloseIcon></CloseIcon>} all types
      </Button>
      {show &&
        types.map((t) => (
          <Button
            key={t}
            size="small"
            style={{ padding: '3px' }}
            onClick={() => {
              handleChange(t);
            }}
          >
            {t}
          </Button>
        ))}
    </Stack>
  );
}

function ModifyTableColumn(props: {
  uuid: string;
  table: string;
  col: string;
  handleClose;
  reload;
  typ: string;
}) {
  let { uuid, table, col, handleClose, reload, typ } = props;
  const [loading, setLoading] = useState(false);
  const [newName, setNewName] = useState(col);
  const [newTyp, setNewTyp] = useState(typ);
  const { showNotification } = useContext(NotificationContext);
  const [modifyType, setModifyType] = useState(false);

  const [nullness, setNullness] = useState('');

  const handleModify = () => {
    if (newName == col && !newTyp) {
      showNotification('error', 'nothing is changed');
      return;
    }
    if (!newName) {
      showNotification('error', "name can't empty");
      return;
    }
    setLoading(true);
    let typePara = null;
    if (typ != newTyp) {
      typePara = { typ: newTyp, null: nullness ? nullness : nullness };
    }
    invoke('modify_table_column', {
      name: uuid,
      tableName: table,
      oldName: col,
      newName: newName,
      typ: typePara,
    })
      .then((_) => {
        setLoading(false);
        showNotification('success', 'Modify column OK');
        reload();
        handleClose();
      })
      .catch((e) => {
        setLoading(false);
        showNotification('error', 'Modify column Failure', e);
      });
  };

  return (
    <Stack spacing={1}>
      <TextField
        fullWidth
        label="New Name"
        value={newName}
        onChange={(e) => {
          setNewName(e.target.value);
        }}
        size="small"
        required
        error={!newName}
      ></TextField>
      {!modifyType && (
        <FormControlLabel
          control={
            <Checkbox
              checked={modifyType}
              onChange={(e) => {
                setModifyType(e.target.checked);
              }}
              name="myCheckbox"
              color="primary"
            />
          }
          label="modify TYPE"
        />
      )}
      {modifyType && (
        <Stack spacing={1}>
          <TableTypesButton
            uuid={uuid}
            handleChange={(e) => {
              setNewTyp(e);
            }}
          ></TableTypesButton>

          <Stack direction="row" spacing={1}>
            <TextField
              label="Type"
              value={newTyp}
              onChange={(e) => {
                setNewTyp(e.target.value);
              }}
              size="small"
            ></TextField>
            <ColumnNullness
              nullness={nullness}
              handleNullness={setNullness}
            ></ColumnNullness>
          </Stack>
        </Stack>
      )}
      <Stack direction="row" spacing={1}>
        <Button
          size="small"
          color="error"
          variant="contained"
          onClick={handleModify}
        >
          Modify
        </Button>
        {loading && <CircularProgress size={progressBarSizeSmall} />}
      </Stack>
    </Stack>
  );
}

function TableColumnsMenu(props: {
  reload;
  table;
  uuid;
  fields;
  kind: DataSourceKind;
  supportModifyColumn;
}) {
  let { reload, table, uuid, fields, kind, supportModifyColumn } = props;

  const [showAdd, setShowAdd] = useState(false);

  return (
    <Paper>
      <MenuList>
        <MenuRefresh reload={reload}></MenuRefresh>
        {supportModifyColumn && (
          <MenuItem
            onClick={() => {
              setShowAdd(true);
            }}
          >
            <ListItemIcon>
              <AddIcon></AddIcon>
            </ListItemIcon>
            <ListItemText> add column </ListItemText>
          </MenuItem>
        )}
      </MenuList>
      {showAdd && (
        <Modal
          open={showAdd}
          onClose={() => {
            setShowAdd(false);
          }}
          aria-labelledby="add column"
        >
          <Box sx={modalStyle} width="800px">
            <TableAddColumn
              table={table}
              uuid={uuid}
              handleClose={() => {
                setShowAdd(false);
              }}
              kind={kind}
              reload={reload}
              fields={fields}
            ></TableAddColumn>
          </Box>
        </Modal>
      )}
    </Paper>
  );
}
function TableAddColumn(props: {
  table: string;
  uuid;
  handleClose;
  reload;
  kind: DataSourceKind;
  fields: types.Field[];
}) {
  let { table, uuid, handleClose, reload, kind, fields } = props;
  const [name, setName] = useState('');
  const [nullness, setNullness] = useState('');
  const [typ, setTyp] = useState('');
  const { showNotification } = useContext(NotificationContext);
  const [after, setAfter] = useState('');
  const [defaultValue, setDefaultValue] = useState('');
  const supportAfter =
    kind == DataSourceKind.Mysql || kind == DataSourceKind.MariaDB;

  const [loading, setLoading] = useState(false);
  const handleAdd = () => {
    if (!name) {
      showNotification('error', 'column name is empty');
      return;
    }
    if (!typ) {
      showNotification('error', 'type name is empty');
      return;
    }
    setLoading(true);
    invoke('add_table_column', {
      name: uuid,
      tableName: table,
      colName: name,
      typ: {
        typ: typ,
        nullness: nullness ? nullness : null,
        after: after ? after : null,
        defaultValue: defaultValue ? defaultValue : null,
      },
    })
      .then(() => {
        setLoading(false);
        showNotification('success', 'Add Column OK');
        handleClose();
        reload();
      })
      .catch((e) => {
        setLoading(false);
        showNotification('error', 'Add Column failure', e);
      });
  };

  return (
    <Stack spacing={1}>
      {loading && <CircularProgress size={progressBarSize} />}
      <TextField
        value={name}
        required
        error={!name}
        onChange={(e) => {
          setName(e.target.value);
        }}
        label="column name"
        size="small"
      ></TextField>
      <TableTypesButton
        uuid={uuid}
        handleChange={(e) => {
          setTyp(e);
        }}
      ></TableTypesButton>
      {/*   TODO some mariadb require a length parameter    */}
      <Stack direction="row">
        <TextField
          value={typ}
          onChange={(e) => setTyp(e.target.value)}
          label="type"
          size="small"
        ></TextField>
        <ColumnNullness
          nullness={nullness}
          handleNullness={(e) => {
            setNullness(e);
          }}
        ></ColumnNullness>
        {supportAfter && (
          <FormControl style={{ minWidth: '180px' }}>
            <InputLabel id="dropdown-label">Select After column </InputLabel>
            <Select
              labelId="dropdown-label"
              value={after}
              onChange={(e) => {
                setAfter(e.target.value);
              }}
            >
              <MenuItem value="">none</MenuItem>
              {fields.map((e) => (
                <MenuItem value={e.field}> {e.field} </MenuItem>
              ))}
            </Select>
          </FormControl>
        )}
      </Stack>
      <Stack>
        <TextField
          value={defaultValue}
          onChange={(e) => setDefaultValue(e.target.value)}
          label="default value"
          size="small"
        ></TextField>
      </Stack>
      <Divider></Divider>
      <Stack direction="row">
        <Button
          size="small"
          variant="contained"
          color="primary"
          onClick={handleAdd}
        >
          add
        </Button>
      </Stack>
    </Stack>
  );
}

function ColumnNullness({ nullness, handleNullness }) {
  return (
    <FormControl style={{ minWidth: '180px' }}>
      <InputLabel id="dropdown-label">select nullness </InputLabel>
      <Select
        labelId="dropdown-label"
        value={nullness}
        onChange={(e) => {
          handleNullness(e.target.value);
        }}
      >
        <MenuItem value="">none</MenuItem>
        <MenuItem value="null">null</MenuItem>
        <MenuItem value="not null">not null</MenuItem>
      </Select>
    </FormControl>
  );
}

function TableColumnsTree(props: { table: string }) {
  let { table } = props;
  let { setUpMenu } = useContext(MenuContext);
  const [fields, setFields] = useState<types.Field[]>([]);
  const { showNotification } = useContext(NotificationContext);
  const { uuid } = useContext(UuidContext);
  const { kind } = useContext(ConnTypeContext);
  const [loading, setLoading] = useState(false);

  const reload = () => {
    setLoading(true);
    invoke<types.Field[]>('show_table_columns', {
      name: uuid,
      tableName: table,
    })
      .then((fs) => {
        setLoading(false);
        setFields(fs);
      })
      .catch((e) => {
        setLoading(false);
        showNotification('error', 'list column failed', e);
      });
  };

  const supportModifyColumn =
    kind == DataSourceKind.MariaDB ||
    kind == DataSourceKind.MsSql ||
    kind == DataSourceKind.Mysql ||
    kind == DataSourceKind.Postgres;

  useEffect(() => {
    reload();
  }, []);

  let idPrefix = uuid + '-table-' + table + 'columns';
  return (
    <TreeItem
      itemId={uuid + '-table-' + table + 'columns'}
      label=<Stack
        spacing={1}
        direction="row"
        onContextMenu={(e) => {
          setUpMenu({
            content: (
              <TableColumnsMenu
                table={table}
                uuid={uuid}
                reload={reload}
                kind={kind}
                fields={fields}
                supportModifyColumn={supportModifyColumn}
              ></TableColumnsMenu>
            ),
            style: calMenuStyle(e, 200),
          });
        }}
      >
        <span> columns</span>
        {loading && (
          <CircularProgress size={progressBarSizeSmall}></CircularProgress>
        )}
      </Stack>
    >
      {fields.map((f) => (
        <TreeItem
          key={f.field}
          onContextMenu={
            supportModifyColumn
              ? (e) => {
                  setUpMenu({
                    content: (
                      <TableColumnMenu
                        table={table}
                        uuid={uuid}
                        col={f.field}
                        reload={reload}
                        typ={f.typ}
                      ></TableColumnMenu>
                    ),
                    style: calMenuStyle(e, 70),
                  });
                }
              : undefined
          }
          itemId={idPrefix + '-' + f.field}
          label=<Stack direction="row" spacing={1}>
            {f.field} <em>({f.typ}) </em>
          </Stack>
        ></TreeItem>
      ))}
    </TreeItem>
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

  const handleDrop = () => {
    setLoading(true);
    invoke('drop_table', { name: uuid, tableName: table, cascade })
      .then(() => {
        setLoading(false);
        showNotification('success', 'Drop table OK');
        reloadTables();
        handleClose();
      })
      .catch((e) => {
        setLoading(false);
        showNotification('error', 'Drop table failure', e);
      });
  };

  const suportCascade = (): boolean => {
    switch (kind) {
      case DataSourceKind.Mysql:
      case DataSourceKind.MariaDB:
      case DataSourceKind.Postgres:
      case DataSourceKind.Sqlite:
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
          variant="outlined"
          size="small"
          onClick={() => {
            handleClose();
          }}
        >
          Cancel
        </Button>
        <Button
          size="small"
          color="error"
          variant="contained"
          onClick={handleDrop}
        >
          Drop
        </Button>
        {loading && <CircularProgress size={progressBarSizeSmall} />}
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
      tableList = (
        <SqlTableView
          reloadOP={async (
            where: string,
            orderBy: string,
            page: number,
            pageSize: number
          ) => {
            let where_null = where ? where : null;
            let orderBy_null = orderBy ? orderBy : null;
            return await invoke<types.MuiDataTableListResp>('show_table', {
              name: uuid,
              tableName: table,
              whereStr: where_null,
              orderBy: orderBy_null,
              page,
              pageSize,
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
          addOP={async (values) => {
            return await invoke<types.SqlExecuteRet>('insert', {
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
                  let x = await invoke<types.MuiDataTableListResp>(
                    'show_table',
                    {
                      name: uuid,
                      tableName: table,
                      whereStr: null,
                      orderBy: null,
                      page: 0,
                      pageSize: 0,
                    }
                  );
                  return x.data.data[0]['value'];
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
              addOP={async (member) => {
                return await invoke('redis_set_add', {
                  name: uuid,
                  key: table,
                  member,
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
              addOP={async (member, value) => {
                return await invoke('redis_zset_add', {
                  name: uuid,
                  key: table,
                  member,
                  value,
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
              addOP={async (member, value) => {
                return await invoke('redis_hash_add', {
                  name: uuid,
                  key: table,
                  member,
                  value,
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
  const { tabs, setTabs, setSelectIndex } = useContext(TabsSetContext);

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
    const id = uuid + '-table-' + table;
    let index = tabsHasId(tabs, id);
    if (typeof index == 'number') {
      setSelectIndex(index);
    } else {
      let tableList: ReactNode = await makeViewTable(
        uuid,
        table,
        kind,
        redisTableType,
        showNotification
      );

      setTabs((e) => {
        setSelectIndex(tabs.length);
        return [...e, new TabContent(id, table, tableList)];
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
          <ListItemText> view data</ListItemText>
        </MenuItem>

        <MenuItem
          onClick={() => {
            setshowDup(true);
          }}
        >
          <ListItemIcon>
            <ContentCopyIcon></ContentCopyIcon>
          </ListItemIcon>
          <ListItemText> duplicate </ListItemText>
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
          onClick={() => {
            setShowRename(true);
          }}
        >
          <ListItemIcon>
            <EditIcon></EditIcon>
          </ListItemIcon>
          <ListItemText> rename </ListItemText>
        </MenuItem>
        <MenuItem
          sx={{ color: 'error.main' }}
          onClick={() => {
            handleTruncateTable();
          }}
        >
          <ListItemIcon>
            <PlaylistRemoveIcon color="error"></PlaylistRemoveIcon>
          </ListItemIcon>
          <ListItemText> truncate </ListItemText>
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
          <ListItemText>drop</ListItemText>
        </MenuItem>
      </MenuList>
      {showDup && (
        <Modal
          open={showDup}
          onClose={() => {
            setshowDup(false);
          }}
          aria-labelledby="set filter"
        >
          <Box sx={modalStyle} width="800px">
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
          onClose={() => {
            setShowRename(false);
          }}
          aria-labelledby="rename"
        >
          <Box sx={modalStyle} width="800px">
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
          onClose={() => {
            setShowExport(false);
          }}
          aria-labelledby="drop"
        >
          <Box sx={modalStyle} width="800px">
            <ExportTableOrView
              uuid={uuid}
              name={table}
              handleClose={() => {
                setShowExport(false);
              }}
              support_kind={getExportKind()}
              kind={kind}
            ></ExportTableOrView>
          </Box>
        </Modal>
      )}

      {showDrop && (
        <Modal
          open={showDrop}
          onClose={() => {
            setShowDrop(false);
          }}
          aria-labelledby="drop"
        >
          <Box sx={modalStyle} width="800px">
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

function TableColumnMenu(props: {
  uuid: string;
  table: string;
  col: string;
  reload;
  typ: string;
}) {
  const { table, col, reload, uuid, typ } = props;
  const { showNotification } = useContext(NotificationContext);
  const [showModify, setShowModify] = useState(false);
  const handleDrop = async () => {
    const confirm = await secondConfirm({
      title: `Drop Column '${col}'`,
      content: '',
      confirmText: 'DROP COLUMN',
    });
    if (!confirm) {
      return;
    }
    invoke('drop_table_column', {
      name: uuid,
      tableName: table,
      columnName: col,
    })
      .then(() => {
        showNotification('success', 'Drop column OK');
        reload();
      })
      .catch((e) => {
        showNotification('error', 'Drop column OK', e);
      });
  };

  return (
    <Paper>
      <MenuList>
        <MenuItem
          sx={{ color: 'error.main' }}
          onClick={() => {
            setShowModify(true);
          }}
        >
          <ListItemIcon>
            <EditIcon> </EditIcon>
          </ListItemIcon>
          <ListItemText> modify </ListItemText>
        </MenuItem>
        <MenuItem
          onClick={() => {
            handleDrop();
          }}
          sx={{ color: 'error.main' }}
        >
          <ListItemIcon>
            <DeleteForeverIcon color="error"> </DeleteForeverIcon>
          </ListItemIcon>
          <ListItemText color="error"> drop </ListItemText>
        </MenuItem>
      </MenuList>
      {showModify && (
        <Modal
          open={showModify}
          onClose={() => {
            setShowModify(false);
          }}
          aria-labelledby="set filter"
        >
          <Box sx={modalStyle} width="800px">
            <ModifyTableColumn
              table={table}
              uuid={uuid}
              col={col}
              handleClose={() => {
                setShowModify(false);
              }}
              typ={typ}
              reload={reload}
            ></ModifyTableColumn>
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
        setLoading(false);
        showNotification('success', 'Duplicate OK');
        reloadTables();
        handleClose();
      })
      .catch((e) => {
        showNotification('error', 'Duplicate failure', e);
      });
  };

  return (
    <Stack spacing={1}>
      <TextField
        value={name}
        label="Duplicate Name"
        onChange={(e) => {
          setName(e.target.value);
        }}
        size="small"
      ></TextField>
      <Stack direction="row" spacing={1}>
        <Button
          size="small"
          variant="outlined"
          onClick={() => {
            handleClose();
          }}
        >
          Cancel
        </Button>
        <Button
          size="small"
          color="primary"
          variant="contained"
          onClick={handleRename}
        >
          Duplicate
        </Button>
        {loading && <CircularProgress size={progressBarSizeSmall} />}
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
  const handleRename = () => {
    if (newName == table) {
      showNotification('error', 'name not changed');
      return;
    }
    setLoading(true);
    invoke('rename_table', { name: uuid, old: table, new: newName })
      .then(() => {
        setLoading(false);
        showNotification('success', 'Rename OK');
        reloadTables();
        handleClose();
      })
      .catch((e) => {
        setLoading(false);
        showNotification('error', 'Rename failure', e);
      });
  };

  return (
    <Stack spacing={1}>
      <TextField
        value={newName}
        label="New Name"
        onChange={(e) => {
          setNewName(e.target.value);
        }}
        size="small"
      ></TextField>
      <Stack direction="row" spacing={1}>
        <Button
          size="small"
          color="primary"
          variant="contained"
          onClick={handleRename}
        >
          Rename
        </Button>
        {loading && <CircularProgress size={progressBarSizeSmall} />}
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
  const handleReload = () => {
    setLoading(true);
    invoke<types.RedisListTable>('redis_list_table', {
      name: uuid,
      filter: filter ? filter : null,
    })
      .then((e) => {
        setTables(e);
        setLoading(false);
      })
      .catch((e) => {
        showNotification('error', 'list redis table failed', e);
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
    //@ts-ignore
    let icon;

    switch (typ) {
      case RedisTableType.String:
        name = 'string';
        icon = <TextFieldsIcon></TextFieldsIcon>;
        break;
      case RedisTableType.List:
        name = 'list';
        icon = <FaListOl></FaListOl>;
        break;
      case RedisTableType.Set:
        name = 'set';
        icon = <span>{'{...}'}</span>;
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
          sx={{ alignItems: 'flex-end' }}
        >
          {/* <span>{icon} </span> */}
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

  let { setUpMenu } = useContext(MenuContext);
  return (
    <TreeItem
      itemId={getPrefix()}
      label=<Stack
        spacing={1}
        direction="row"
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
        <TableViewIcon> </TableViewIcon>
        <span>keys</span>
        {loading && (
          <CircularProgress size={progressBarSizeSmall}></CircularProgress>
        )}
      </Stack>
    >
      <TreeItem
        itemId={filterTreeID}
        label=<Stack
          direction="row"
          style={{ cursor: 'pointer' }}
          onClick={() => {
            setShowEditFilter(true);
          }}
        >
          filter: <em> {filter} </em>
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
          onClose={() => {
            setShowEditFilter(false);
          }}
          aria-labelledby="add column"
        >
          <Box sx={modalStyle} width="800px">
            <TextField
              value={filter}
              fullWidth
              label="filer"
              helperText="can be *key or key* etc..."
              onChange={(e) => {
                setFilter(e.target.value);
              }}
              size="small"
            ></TextField>
          </Box>
        </Modal>
      )}
    </TreeItem>
  );
}

enum RedisTableType {
  String,
  List,
  Set,
  Zset,
  Hash,
}

export { TablesTree, RedisTables };
