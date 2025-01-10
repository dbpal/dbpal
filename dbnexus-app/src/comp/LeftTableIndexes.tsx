import { useContext, useEffect, useState } from 'react';
import {
  calMenuStyle,
  MenuContext,
  MenuRefresh,
  modalStyle,
  progressBarSize,
  progressBarSizeSmall,
  UuidContext,
} from './Common';
import { TreeItem } from '@mui/x-tree-view/TreeItem';
import {
  Box,
  Button,
  CircularProgress,
  ListItemIcon,
  ListItemText,
  MenuItem,
  MenuList,
  Modal,
  Paper,
  Stack,
  TextField,
} from '@mui/material';
import types from './ApiStruct';
import { NotificationContext } from './Notification';
import { invoke } from '@tauri-apps/api/core';
import EditIcon from '@mui/icons-material/Edit';
import DeleteForeverIcon from '@mui/icons-material/DeleteForever';
import { secondConfirm } from './Confirm';

function LeftIndexes(props: { table: string }) {
  const { table } = props;

  const { uuid } = useContext(UuidContext);
  const [loading, setLoading] = useState(false);
  const idPrefix = uuid + '-table-' + table + '-indexes-';
  const [indexes, setIndexes] = useState<types.Index[]>([]);
  const { showNotification } = useContext(NotificationContext);
  let { setUpMenu } = useContext(MenuContext);

  const reload = () => {
    setLoading(true);
    invoke<types.Index[]>('show_indexes', { name: uuid, tableName: table })
      .then((indexes) => {
        setLoading(false);
        setIndexes(indexes);
      })
      .catch((e) => {
        setLoading(false);
        showNotification('error', 'list indexes failed', e);
      });
  };

  useEffect(() => {
    reload();
  }, []);
  return (
    <TreeItem
      itemId={idPrefix}
      label=<Stack
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
            style: calMenuStyle(e, 70),
          });
        }}
      >
        <span>indexes </span>
        {loading && <CircularProgress size={progressBarSizeSmall} />}
      </Stack>
    >
      {indexes.map((index) => {
        return (
          <TreeItem
            itemId={idPrefix + '-index-' + index.name}
            key={index.name}
            label=<Stack
              direction="row"
              onContextMenu={(e) => {
                setUpMenu({
                  content: (
                    <IndexMenu
                      uuid={uuid}
                      table={table}
                      indexName={index.name}
                      reload={reload}
                    ></IndexMenu>
                  ),
                  style: calMenuStyle(e, 70),
                });
              }}
            >
              <span> {index.name} </span>
            </Stack>
          ></TreeItem>
        );
      })}
    </TreeItem>
  );
}

function IndexMenu(props: {
  uuid: string;
  table: string;
  indexName: string;
  reload;
}) {
  const { uuid, table, indexName, reload } = props;
  const [showRename, setShowRename] = useState(false);
  const { showNotification } = useContext(NotificationContext);

  const handleDrop = async () => {
    let confirm = await secondConfirm({
      title: `drop index '${indexName}'`,
      content: 'Are u sure to drop the index',
      confirmText: 'drop',
    });
    if (!confirm) {
      return;
    }
    invoke('drop_index', {
      name: uuid,
      tableName: table,
      index: indexName,
    })
      .then(() => {
        showNotification('success', 'Drop OK');
      })
      .catch((e) => {
        showNotification('error', 'Drop index failure', e);
      });
  };
  return (
    <Paper>
      <MenuList>
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
            handleDrop();
          }}
        >
          <ListItemIcon>
            <DeleteForeverIcon color="error"></DeleteForeverIcon>
          </ListItemIcon>
          <ListItemText>drop</ListItemText>
        </MenuItem>
      </MenuList>

      {showRename && (
        <Modal
          open={showRename}
          onClose={() => {
            setShowRename(false);
          }}
          aria-labelledby="rename"
        >
          <Box sx={modalStyle} width="800px">
            <RenameIndex
              uuid={uuid}
              table={table}
              reload={reload}
              handleClose={() => {
                setShowRename(false);
              }}
              indexName={indexName}
            ></RenameIndex>
          </Box>
        </Modal>
      )}
    </Paper>
  );
}

function RenameIndex(props: {
  uuid: string;
  table: string;
  reload;
  handleClose;
  indexName;
}) {
  const { uuid, table, reload, handleClose, indexName } = props;
  const [loading, setLoading] = useState(false);
  const [newName, setNewname] = useState(indexName);
  const { showNotification } = useContext(NotificationContext);
  const handleRename = () => {
    if (newName == indexName) {
      showNotification('error', 'name not change');
      return;
    }
    setLoading(true);

    invoke('rename_index', {
      name: uuid,
      tableName: table,
      oldName: indexName,
      newName: newName,
    })
      .then(() => {
        setLoading(false);
        showNotification('success', 'Rename OK');
        reload();
        handleClose();
      })
      .catch((e) => {
        setLoading(false);
        showNotification('error', 'Rename Failure', e);
      });
  };
  return (
    <Stack spacing={1}>
      {loading && <CircularProgress size={progressBarSize} />}
      <TextField
        label="new name"
        value={newName}
        onChange={(e) => {
          setNewname(e.target.value);
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
      </Stack>
    </Stack>
  );
}

export { LeftIndexes };
