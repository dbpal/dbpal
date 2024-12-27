import { ReactNode, useContext, useEffect, useState } from 'react';
import {
  DataSourceKind,
  MenuContext,
  MenuRefresh,
  modalStyle,
  UuidContext,
  progressBarSizeSmall,
  calMenuStyle,
  tabsHasId,
  TabsSetContext,
  ConnTypeContext,
  progressBarSize,
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
import RefreshIcon from '@mui/icons-material/Refresh';
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
import { secondConfirm } from './Confirm';
import { TabContent } from './Tabs';
import { SqlTableView } from './SqlTableView';
import { TriggersTree } from './LeftTriggers';
import { ExportTableView } from './Export';
import AddIcon from '@mui/icons-material/Add';

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
        direction='row'
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
  const { kind: conn_type } = useContext(ConnTypeContext);
  let { setUpMenu } = useContext(MenuContext);
  return (
    <TreeItem
      itemId={uuid + '-table-' + table}
      label={
        <Stack
          direction='row'
          spacing={1}
          justifyContent='space-between'
          alignContent='center'
          onContextMenu={(e) => {
            setUpMenu({
              content: (
                <TableMenu
                  kind={conn_type}
                  reload={reloadTables}
                  table={table}
                  uuid={uuid}
                />
              ),
              style: calMenuStyle(e, 200),
            });
          }}
        >
          <Stack direction='row'>
            <TableViewIcon> </TableViewIcon> {table}
          </Stack>
          <span> {size} </span>
        </Stack>
      }
    >
      <TableColumnsTree table={table}></TableColumnsTree>
      <TriggersTree table={table}></TriggersTree>
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
    <Stack direction='row' style={{ flexWrap: 'wrap' }}>
      <Button onClick={toggleShow}>
        {show && <CloseIcon></CloseIcon>} all types
      </Button>
      {show &&
        types.map((t) => (
          <Button
            style={{ padding: '3px' }}
            variant='outlined'
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
        label='New Name'
        value={newName}
        onChange={(e) => {
          setNewName(e.target.value);
        }}
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
              name='myCheckbox'
              color='primary'
            />
          }
          label='modify TYPE'
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

          <Stack direction='row' spacing={1}>
            <TextField
              label='Type'
              value={newTyp}
              onChange={(e) => {
                setNewTyp(e.target.value);
              }}
            ></TextField>
            <ColumnNullness
              nullness={nullness}
              handleNullness={setNullness}
            ></ColumnNullness>
          </Stack>
        </Stack>
      )}
      <Stack direction='row' spacing={1}>
        <Button color='error' variant='contained' onClick={handleModify}>
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
}) {
  let { reload, table, uuid, fields, kind } = props;

  const [showAdd, setShowAdd] = useState(false);

  return (
    <Paper>
      <MenuList>
        <MenuRefresh reload={reload}></MenuRefresh>

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
      </MenuList>
      {showAdd && (
        <Modal
          open={showAdd}
          onClose={() => {
            setShowAdd(false);
          }}
          aria-labelledby='add column'
        >
          <Box sx={modalStyle} width='600px'>
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
        label='column name'
      ></TextField>
      <TableTypesButton
        uuid={uuid}
        handleChange={(e) => {
          setTyp(e);
        }}
      ></TableTypesButton>
      {/*   TODO some mariadb require a length parameter    */}
      <Stack direction='row'>
        <TextField
          value={typ}
          onChange={(e) => setTyp(e.target.value)}
          label='type'
        ></TextField>
        <ColumnNullness
          nullness={nullness}
          handleNullness={(e) => {
            setNullness(e);
          }}
        ></ColumnNullness>
        {supportAfter && (
          <FormControl style={{ minWidth: '180px' }}>
            <InputLabel id='dropdown-label'>Select After column </InputLabel>
            <Select
              labelId='dropdown-label'
              value={after}
              onChange={(e) => {
                setAfter(e.target.value);
              }}
            >
              <MenuItem value=''>none</MenuItem>
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
          label='default value'
        ></TextField>
      </Stack>
      <Divider></Divider>
      <Stack direction='row'>
        <Button variant='contained' color='primary' onClick={handleAdd}>
          add
        </Button>
      </Stack>
    </Stack>
  );
}

function ColumnNullness({ nullness, handleNullness }) {
  return (
    <FormControl style={{ minWidth: '180px' }}>
      <InputLabel id='dropdown-label'>select nullness </InputLabel>
      <Select
        labelId='dropdown-label'
        value={nullness}
        onChange={(e) => {
          handleNullness(e.target.value);
        }}
      >
        <MenuItem value=''>none</MenuItem>
        <MenuItem value='null'>null</MenuItem>
        <MenuItem value='not null'>not null</MenuItem>
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
    invoke<types.Field[]>('show_columns', { name: uuid, tableName: table })
      .then((fs) => {
        setLoading(false);
        setFields(fs);
      })
      .catch((e) => {
        setLoading(false);
        showNotification('error', 'list column failed', e);
      });
  };

  const support_modify_column =
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
        direction='row'
        onContextMenu={
          support_modify_column
            ? (e) => {
                setUpMenu({
                  content: (
                    <TableColumnsMenu
                      table={table}
                      uuid={uuid}
                      reload={reload}
                      kind={kind}
                      fields={fields}
                    ></TableColumnsMenu>
                  ),
                  style: calMenuStyle(e, 200),
                });
              }
            : undefined
        }
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
            support_modify_column
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
          label=<Stack direction='row' spacing={1}>
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
          label='CASCADE'
        />
      )}
      <Stack direction='row' spacing={1}>
        <Button
          variant='outlined'
          onClick={() => {
            handleClose();
          }}
        >
          Cancel
        </Button>
        <Button color='error' variant='contained' onClick={handleDrop}>
          Drop
        </Button>
        {loading && <CircularProgress size={progressBarSizeSmall} />}
      </Stack>
    </Stack>
  );
}

function TableMenu(props: {
  table: string;
  uuid: string;
  reload;
  kind: DataSourceKind;
}) {
  let { table, uuid, reload, kind } = props;

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

  const handleOpenTableList = () => {
    const id = uuid + '-table-' + table;
    let index = tabsHasId(tabs, id);
    if (typeof index == 'number') {
      setSelectIndex(index);
    } else {
      let tableList: ReactNode = null;
      switch (kind) {
        case DataSourceKind.MariaDB:
        case DataSourceKind.Mysql:
        case DataSourceKind.MsSql:
        case DataSourceKind.Sqlite:
        case DataSourceKind.Postgres:
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
          throw 'impl this';
        case DataSourceKind.Mongo:
          throw 'impl this';
      }

      setTabs((e) => {
        setSelectIndex(tabs.length);
        return [...e, new TabContent(id, table, tableList)];
      });
    }
  };

  return (
    <Paper>
      <MenuList>
        <MenuItem
          onClick={() => {
            handleOpenTableList();
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
            setShowDrop(true);
          }}
        >
          <ListItemIcon>
            <DeleteForeverIcon></DeleteForeverIcon>
          </ListItemIcon>
          <ListItemText>drop table</ListItemText>
        </MenuItem>
        <MenuItem
          sx={{ color: 'error.main' }}
          onClick={() => {
            handleTruncateTable();
          }}
        >
          <ListItemIcon>
            <PlaylistRemoveIcon></PlaylistRemoveIcon>
          </ListItemIcon>
          <ListItemText> truncate </ListItemText>
        </MenuItem>
      </MenuList>
      {showDup && (
        <Modal
          open={showDup}
          onClose={() => {
            setshowDup(false);
          }}
          aria-labelledby='set filter'
        >
          <Box sx={modalStyle} width='600px'>
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
          aria-labelledby='rename'
        >
          <Box sx={modalStyle} width='600px'>
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
          aria-labelledby='drop'
        >
          <Box sx={modalStyle} width='600px'>
            <ExportTableView
              uuid={uuid}
              table={table}
              handleClose={() => {
                setShowExport(false);
              }}
            ></ExportTableView>
          </Box>
        </Modal>
      )}

      {showDrop && (
        <Modal
          open={showDrop}
          onClose={() => {
            setShowDrop(false);
          }}
          aria-labelledby='drop'
        >
          <Box sx={modalStyle} width='600px'>
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
          <ListItemText color='error'> modify </ListItemText>
        </MenuItem>
        <MenuItem
          onClick={() => {
            handleDrop();
          }}
          sx={{ color: 'error.main' }}
        >
          <ListItemIcon>
            <DeleteForeverIcon> </DeleteForeverIcon>
          </ListItemIcon>
          <ListItemText color='error'> drop </ListItemText>
        </MenuItem>
      </MenuList>
      {showModify && (
        <Modal
          open={showModify}
          onClose={() => {
            setShowModify(false);
          }}
          aria-labelledby='set filter'
        >
          <Box sx={modalStyle} width='600px'>
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
        label='Duplicate Name'
        onChange={(e) => {
          setName(e.target.value);
        }}
      ></TextField>
      <Stack direction='row' spacing={1}>
        <Button
          variant='outlined'
          onClick={() => {
            handleClose();
          }}
        >
          Cancel
        </Button>
        <Button color='primary' variant='contained' onClick={handleRename}>
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
        label='New Name'
        onChange={(e) => {
          setNewName(e.target.value);
        }}
      ></TextField>
      <Stack direction='row' spacing={1}>
        <Button
          variant='outlined'
          onClick={() => {
            handleClose();
          }}
        >
          Cancel
        </Button>
        <Button color='primary' variant='contained' onClick={handleRename}>
          Rename
        </Button>
        {loading && <CircularProgress size={progressBarSizeSmall} />}
      </Stack>
    </Stack>
  );
}

export { TablesTree };
