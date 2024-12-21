import { useContext, useEffect, useState } from 'react';
import {
  DataSourceKind,
  MenuContext,
  MenuRefresh,
  modalStyle,
  NameAndTypeContext,
  progressBarSize,
  setUpMenu,
  tabsHasId,
  TabsSetContext,
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
import {
  Box,
  Button,
  CircularProgress,
  FormControlLabel,
  ListItem,
  MenuItem,
  MenuList,
  Modal,
  Stack,
  Switch,
  TextField,
  Typography,
} from '@mui/material';
import { secondConfirm } from './Confirm';
import { TabContent } from './Tabs';
import { SqlTableView } from './SqlTableView';

function TablesTree() {
  const { uuid } = useContext(NameAndTypeContext);
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
  let { setMenuContent } = useContext(MenuContext);
  return (
    <TreeItem
      itemId={uuid + '-tables'}
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
        tables
      </div>
    >
      {loading && (
        <TreeItem
          itemId={uuid + '-tables' + 'loading'}
          label={<CircularProgress size={progressBarSize}></CircularProgress>}
        ></TreeItem>
      )}
      {tables.map((e) => (
        <TableTree
          key={e.name}
          reloadTables={handleReload}
          table={e.name}
        ></TableTree>
      ))}
    </TreeItem>
  );
}

function TableTree(props: { table: string; reloadTables }) {
  let { table, reloadTables } = props;
  const { uuid, conn_type } = useContext(NameAndTypeContext);
  let { setMenuContent } = useContext(MenuContext);
  return (
    <TreeItem
      itemId={uuid + '-' + table}
      onContextMenu={(e) => {
        setMenuContent(
          <TableMenu
            kind={conn_type}
            reloadTables={reloadTables}
            table={table}
            uuid={uuid}
          />
        );
        setUpMenu(e, 200);
      }}
      label={
        <>
          <TableViewIcon> </TableViewIcon> {table}
        </>
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
        {loading && <CircularProgress size={progressBarSize} />}
      </Stack>
    </Stack>
  );
}

function TableMenu(props: {
  table: string;
  uuid: string;
  reloadTables;
  kind: DataSourceKind;
}) {
  let { table, uuid, reloadTables, kind } = props;
  const [showDup, setshowDup] = useState(false);
  const [showRename, setShowRename] = useState(false);
  const [showDrop, setShowDrop] = useState(false);

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
      const reloadOP = async () => {
        invoke('show_table');
      };
      setTabs((e) => {
        setSelectIndex(tabs.length);
        return [
          ...e,
          new TabContent(
            id,
            table,

            (
              <SqlTableView
                reloadOP={async (
                  where: string,
                  orderBy: string,
                  page: number,
                  pageSize: number
                ) => {
                  console.log('111');
                  let where_null = where ? where : null;
                  let orderBy_null = orderBy ? orderBy : null;
                  return invoke('show_table', {
                    name: uuid,
                    tableName: table,
                    whereStr: where_null,
                    orderBy: orderBy_null,
                    page,
                    pageSize,
                  });
                }}
              >
                table content
              </SqlTableView>
            )
          ),
        ];
      });
    }
  };

  return (
    <Stack>
      <MenuList>
        <MenuItem
          onClick={() => {
            handleOpenTableList();
          }}
        >
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
          onClick={() => {
            setshowDup(true);
          }}
        >
          <ListItem>
            <ContentCopyIcon></ContentCopyIcon>
          </ListItem>
          <ListItem> duplicate table </ListItem>
          <Typography
            variant='body2'
            sx={{ color: 'text.secondary' }}
          ></Typography>
        </MenuItem>
        <MenuItem
          onClick={() => {
            setShowRename(true);
          }}
        >
          <ListItem>
            <EditIcon></EditIcon>
          </ListItem>
          <ListItem> rename table</ListItem>
          <Typography
            variant='body2'
            sx={{ color: 'text.secondary' }}
          ></Typography>
        </MenuItem>
        <MenuItem
          sx={{ color: 'error.main' }}
          onClick={() => {
            setShowDrop(true);
          }}
        >
          <ListItem>
            <DeleteForeverIcon></DeleteForeverIcon>
          </ListItem>
          <ListItem>drop table</ListItem>
          <Typography
            variant='body2'
            sx={{ color: 'text.secondary' }}
          ></Typography>
        </MenuItem>
        <MenuItem
          sx={{ color: 'error.main' }}
          onClick={() => {
            handleTruncateTable();
          }}
        >
          <ListItem>
            <PlaylistRemoveIcon></PlaylistRemoveIcon>
          </ListItem>
          <ListItem> truncate table </ListItem>
          <Typography
            variant='body2'
            sx={{ color: 'text.secondary' }}
          ></Typography>
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
              reloadTables={reloadTables}
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
              reloadTables={reloadTables}
              handleClose={() => {
                setShowRename(false);
              }}
            ></RenameTable>
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
              reloadTables={reloadTables}
              handleClose={() => {
                setShowDrop(false);
              }}
              kind={kind}
            ></DropTable>
          </Box>
        </Modal>
      )}
    </Stack>
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
        showNotification('success', 'Rename OK');
        reloadTables();
      })
      .catch((e) => {
        showNotification('error', 'Rename failure', e);
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
        {loading && <CircularProgress size={progressBarSize} />}
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
      })
      .catch((e) => {
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
        {loading && <CircularProgress size={progressBarSize} />}
      </Stack>
    </Stack>
  );
}

export { TablesTree };
