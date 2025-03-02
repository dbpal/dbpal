import { useContext, useEffect, useRef, useState } from 'react';
import {
  modalStyle,
  progressBarSizeSmall,
  treeItemCommonStyle,
} from './Common';
import { TreeItem } from '@mui/x-tree-view/TreeItem';
import {
  Box,
  Button,
  CircularProgress,
  Divider,
  FormControlLabel,
  FormLabel,
  ListItemIcon,
  ListItemText,
  MenuItem,
  MenuList,
  Modal,
  Paper,
  Radio,
  RadioGroup,
  Stack,
  TextField,
  Tooltip,
  Typography,
} from '@mui/material';
import AddIcon from '@mui/icons-material/Add';
import types from './ApiStruct';
import { NotificationContext } from './Notification';
import { invoke } from '@tauri-apps/api/core';
import EditIcon from '@mui/icons-material/Edit';
import DeleteForeverIcon from '@mui/icons-material/DeleteForever';
import {
  calMenuStyle,
  ConnTypeContext,
  ContextMenuContext,
  DataSourceKind,
  secondConfirm,
  UuidContext,
} from './helper';
import { MenuRefresh } from './CommonComp';
import { MongoBsonEditor } from './MongoDB';
import { BSON } from 'bson';

function Menu(props: {
  reload;
  uuid: string;
  table: string;
  kind: DataSourceKind;
}) {
  const { reload, uuid, table, kind } = props;
  const [showAdd, setShowAdd] = useState(false);

  let addIndex;

  switch (kind) {
    case DataSourceKind.Mongo:
      addIndex = (
        <MongoAddIndex
          uuid={uuid}
          table={table}
          handleClose={() => {
            setShowAdd(false);
          }}
          realod={reload}
        ></MongoAddIndex>
      );
      break;

    default:
      addIndex = (
        <AddIndex
          uuid={uuid}
          table={table}
          handleClose={() => {
            setShowAdd(false);
          }}
          kind={kind}
          realod={reload}
        ></AddIndex>
      );
  }

  return (
    <Paper>
      <MenuList>
        <MenuItem
          onClick={() => {
            setShowAdd(true);
          }}
        >
          <ListItemIcon>
            <AddIcon></AddIcon>
          </ListItemIcon>
          <ListItemText> add </ListItemText>
        </MenuItem>
        <MenuRefresh reload={reload}></MenuRefresh>
      </MenuList>
      {showAdd && (
        <Modal
          open={showAdd}
          onClose={(_, reason) => {
            if (reason === 'backdropClick') {
              return;
            }
            setShowAdd(false);
          }}
        >
          <Box sx={modalStyle} width="900px">
            {addIndex}
          </Box>
        </Modal>
      )}
    </Paper>
  );
}

function MongoAddIndex(props: { uuid; table; handleClose; realod }) {
  const { uuid, table, handleClose, realod } = props;

  const indexTypes = ['normal', 'unique'];
  const [indexType, setIndexType] = useState(indexTypes[0]);
  const [indexName, setIndexName] = useState('');
  const indexNameDom = useRef<HTMLInputElement>(null);

  const { showNotification } = useContext(NotificationContext);
  const handleAdd = async (keys) => {
    try {
      if (!indexName) {
        showNotification('error', "index name can't be none");
        indexNameDom.current.focus();
        return;
      }
      await invoke('mongo_create_index', {
        name: uuid,
        tableName: table,
        keys: BSON.serialize(keys),
        unique: indexType == 'unique',
        indexName: indexName ? indexName : null,
      });
      showNotification('success', 'create index ok');
      handleClose();
      realod();
    } catch (e) {
      showNotification('error', 'create index failed', e);
    }
  };

  return (
    <Stack spacing={1}>
      <TextField
        value={indexName}
        onChange={(e) => {
          setIndexName(e.target.value);
        }}
        inputRef={indexNameDom}
        label="index name"
        size="small"
        required
      ></TextField>

      <RadioGroup
        row
        onChange={(e) => {
          setIndexType(e.target.value);
        }}
        value={indexType}
      >
        {indexTypes.map((e) => (
          <FormControlLabel key={e} value={e} control={<Radio />} label={e} />
        ))}
      </RadioGroup>
      <MongoBsonEditor
        doc={{}}
        height="250px"
        buttons={[
          {
            buttonText: 'create index',
            callBack: (keys) => {
              handleAdd(keys);
            },
          },
        ]}
      ></MongoBsonEditor>
    </Stack>
  );
}

function AddIndex(props: {
  uuid;
  table;
  handleClose;
  realod;
  kind: DataSourceKind;
}) {
  let { uuid, table, handleClose, realod, kind } = props;

  const [fields, setFields] = useState<types.Field[]>([]);
  const [includes, setIncludes] = useState<string[]>([]);
  const { showNotification } = useContext(NotificationContext);
  const indexTypes = ['normal', 'unique'];
  const [indexType, setIndexType] = useState(indexTypes[0]);
  const [indexName, setIndexName] = useState('');
  const indexNameDom = useRef<HTMLInputElement>(null);
  const supportUnique = kind != DataSourceKind.Cassandra;

  useEffect(() => {
    invoke<types.Field[]>('show_table_columns', {
      name: uuid,
      tableName: table,
    })
      .then((e) => {
        setFields(e);
        e;
      })
      .catch((e) => {
        showNotification('error', 'list fields failed', e);
      });
  }, []);
  const handleAdd = async () => {
    if (!indexName) {
      showNotification('error', "index name can't be none");
      indexNameDom.current.focus();
      return;
    }
    if (includes.length == 0) {
      showNotification('error', 'select at least one column');
      return;
    }
    try {
      await invoke('create_index', {
        name: uuid,
        tableName: table,
        fields: includes,
        indexType,
        indexName: indexName ? indexName : null,
      });
      showNotification('success', 'create index ok');
      handleClose();
      realod();
    } catch (e) {
      showNotification('error', 'create index failed', e);
    }
  };

  return (
    <Stack spacing={1}>
      <TextField
        value={indexName}
        onChange={(e) => {
          setIndexName(e.target.value);
        }}
        inputRef={indexNameDom}
        label="index name"
        size="small"
        required
      ></TextField>
      <Stack direction="row" style={{ flexWrap: 'wrap', alignItems: 'center' }}>
        <Typography> includes: </Typography>

        {includes.map((x) => (
          <Button
            onClick={() => {
              setIncludes((e) => e.filter((e) => e != x));
            }}
            key={x}
            color="primary"
            variant="outlined"
          >
            {x}
          </Button>
        ))}
      </Stack>

      <Stack direction="row" style={{ flexWrap: 'wrap', alignItems: 'center' }}>
        <Typography> candidates: </Typography>
        {fields
          .filter((f) => includes.find((e) => e == f.field) == undefined)
          .map((x) => (
            <Button
              key={x.field}
              onClick={() => {
                setIncludes((e) => [...e, x.field]);
              }}
              color="primary"
            >
              {x.field}
            </Button>
          ))}
      </Stack>

      {supportUnique && (
        <>
          <FormLabel component="legend">select a type</FormLabel>
          <RadioGroup
            row
            onChange={(e) => {
              setIndexType(e.target.value);
            }}
            value={indexType}
          >
            {indexTypes.map((e) => (
              <FormControlLabel
                key={e}
                value={e}
                control={<Radio />}
                label={e}
              />
            ))}
          </RadioGroup>
        </>
      )}
      <Divider></Divider>
      <Stack direction="row">
        <Button onClick={handleAdd} variant="contained" color="primary">
          add
        </Button>
      </Stack>
    </Stack>
  );
}

function LeftIndexes(props: { table: string }) {
  const { table } = props;
  const { uuid } = useContext(UuidContext);
  const [loading, setLoading] = useState(false);
  const idPrefix = uuid + '-table-' + table + '-indexes-';
  const [indexes, setIndexes] = useState<types.Index[]>([]);
  const { showNotification } = useContext(NotificationContext);
  let { setUpMenu } = useContext(ContextMenuContext);
  const { kind } = useContext(ConnTypeContext);
  const reload = () => {
    setLoading(true);
    invoke<types.Index[]>('show_indexes', { name: uuid, tableName: table })
      .then((indexes) => {
        setIndexes(indexes);
      })
      .catch((e) => {
        showNotification('error', 'list indexes failed', e);
      })
      .finally(() => {
        setLoading(false);
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
              <Menu
                reload={reload}
                uuid={uuid}
                table={table}
                kind={kind}
              ></Menu>
            ),
            style: calMenuStyle(e, 70),
          });
        }}
        style={{ ...treeItemCommonStyle }}
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
              spacing={1}
              onContextMenu={
                index.name
                  ? (e) => {
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
                    }
                  : undefined
              }
              style={{ ...treeItemCommonStyle }}
            >
              <Tooltip title={index.kind}>
                <span>{index.name}</span>
              </Tooltip>
            </Stack>
          >
            {index.on.map((e) => (
              <TreeItem
                key={e}
                itemId={idPrefix + '-index-' + index.name + '-col-' + e}
                label=<span style={{ ...treeItemCommonStyle }}>{e}</span>
              ></TreeItem>
            ))}
          </TreeItem>
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
        reload();
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
          onClose={(_, reason) => {
            if (reason === 'backdropClick') {
              return;
            }
            setShowRename(false);
          }}
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
        showNotification('success', 'Rename OK');
        reload();
        handleClose();
      })
      .catch((e) => {
        showNotification('error', 'Rename Failure', e);
      })
      .finally(() => {
        setLoading(false);
      });
  };
  return (
    <Stack spacing={1}>
      <TextField
        size="small"
        label="new name"
        value={newName}
        onChange={(e) => {
          setNewname(e.target.value);
        }}
      ></TextField>
      <Stack direction="row" spacing={1}>
        <Button
          color="primary"
          variant="contained"
          loading={loading}
          onClick={handleRename}
        >
          rename
        </Button>
      </Stack>
    </Stack>
  );
}

export { LeftIndexes };
