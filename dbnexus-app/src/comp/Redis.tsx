import React, {
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
} from 'react';
import {
  Button,
  Stack,
  Box,
  CircularProgress,
  TextField,
  MenuItem,
  FormControl,
  InputLabel,
  Select,
  Typography,
  Modal,
  Paper,
  MenuList,
  ListItemIcon,
  ListItemText,
} from '@mui/material';
import { v4 as uuidv4 } from 'uuid';
import ContentCopyIcon from '@mui/icons-material/ContentCopy';
import DeleteForeverIcon from '@mui/icons-material/DeleteForever';
import {
  modalStyle,
  progressBarSize,
  SettingsContext,
  reloadOK,
  VisiableContext,
  SimpleResizeEditor,
} from './Common.tsx';

import { types } from './ApiStruct.ts';
import { openUrl } from '@tauri-apps/plugin-opener';
import { NotificationContext } from './Notification';
import {
  DataGrid,
  GridPaginationModel,
  GridSlotsComponentsProps,
  GridSortModel,
} from '@mui/x-data-grid';
import HexEditor from 'react-hex-editor';
import oneDarkPro from 'react-hex-editor/themes/oneDarkPro';
import AddIcon from '@mui/icons-material/Add';

import {
  arrayToHex,
  calMenuStyle,
  ContextMenuContext,
  datagridContainerHeight,
  handleCopyToClipboard,
  hexTryToUtf8,
  stringToHex,
  stringToHexArray,
  uint8ArrayToUtf8,
} from './helper.ts';
import {
  datagridHeaderHeight,
  datagridRowHeight,
  pageSizeOptions,
} from './CommonComp.tsx';

class RedisZsetHashModifyReq {
  constructor(
    public oldKey: string,
    public newKey: string,
    public value: string
  ) {}
}

enum RedisEditorMode {
  Hex,
  Text,
  Json,
}

function RedisAddList(props: {
  lpushOP: (value: string) => Promise<void>;
  rpushOP: (value: string) => Promise<void>;
}) {
  const { lpushOP, rpushOP } = props;
  const [loading, setLoading] = useState(false);
  const [lOrR, setLOrR] = useState('lpush');
  const [key, setKey] = useState('');
  const { showNotification } = useContext(NotificationContext);
  const handleAdd = () => {
    // reset some state
    setLoading(true);
    (lOrR == 'lpush' ? lpushOP : rpushOP)(key)
      .then(() => {
        showNotification('success', 'Push OK');
      })
      .catch((e) => {
        showNotification('error', 'Push OK', e);
      })
      .finally(() => {
        setLoading(false);
      });
  };

  return (
    <Stack spacing={1}>
      <Box width="200px">
        <FormControl fullWidth>
          <InputLabel id="simple-select-label">Direction</InputLabel>
          <Select
            labelId="simple-select-label"
            id="simple-select"
            value={lOrR}
            onChange={(e) => {
              setLOrR(e.target.value);
            }}
            label=""
          >
            <MenuItem value="lpush">List head (LPUSH)</MenuItem>
            <MenuItem value="rpush">List tail (RPUSH)</MenuItem>
          </Select>
        </FormControl>
      </Box>
      <RedisValueEditor
        hexRows={17}
        textRows={18}
        handleValue={(e) => setKey(e)}
        jsonHeigth="60vh"
      ></RedisValueEditor>

      <Stack direction="row" spacing={1}>
        <Button
          loading={loading}
          color="primary"
          variant="contained"
          onClick={handleAdd}
        >
          push
        </Button>
      </Stack>
    </Stack>
  );
}

function RedisStringTable(props: {
  modifyOP: (content: string) => Promise<void>;
  content: string;
}) {
  const { modifyOP, content } = props;
  const [newContent, setNewConent] = useState(content);
  const [loading, setLoading] = useState(false);
  const { showNotification } = useContext(NotificationContext);
  const handleModify = () => {
    setLoading(true);
    modifyOP(newContent)
      .then(() => {
        showNotification('success', 'Modify OK');
      })
      .catch((e) => {
        showNotification('error', 'Modify failure', e);
      })
      .finally(() => {
        setLoading(false);
      });
  };

  return (
    <Stack spacing={1}>
      <RedisValueEditor
        oldValue={newContent}
        handleValue={(e) => {
          setNewConent(e);
        }}
        textRows={22}
        hexRows={0x17}
        jsonHeigth="60vh"
      ></RedisValueEditor>

      <Stack width="100px" direction="row" spacing={1}>
        <Button
          loading={loading}
          color="primary"
          variant="contained"
          onClick={() => {
            handleModify();
          }}
        >
          Set
        </Button>
      </Stack>
    </Stack>
  );
}

function RedisAddSet(props: { addOP: (key: string) => Promise<void> }) {
  let { addOP } = props;
  const [loading, setLoading] = useState(false);
  const [key, setKey] = useState('');
  const { showNotification } = useContext(NotificationContext);
  const handleAdd = () => {
    // reset some state
    setLoading(true);
    addOP(key)
      .then(() => {
        showNotification('success', 'Add ok');
      })
      .catch((e) => {
        showNotification('error', 'Add error', e);
      })
      .finally(() => {
        setLoading(false);
      });
  };

  return (
    <Stack spacing={1}>
      <RedisValueEditor
        handleValue={(e) => {
          setKey(e);
        }}
        hexRows={17}
        textRows={18}
        jsonHeigth="60vh"
      ></RedisValueEditor>

      <Stack direction="row" spacing={1}>
        <Button color="primary" variant="contained" onClick={handleAdd}>
          Add
        </Button>
        {loading && <CircularProgress size={progressBarSize} />}
      </Stack>
    </Stack>
  );
}

function RedisAddHash(props: {
  addOP: (key: string, value: string) => Promise<void>;
}) {
  let { addOP } = props;
  const [loading, setLoading] = useState(false);
  const [value, setValue] = useState('');
  const [key, setKey] = useState('');

  const { showNotification } = useContext(NotificationContext);
  const handleAdd = () => {
    // reset some state
    setLoading(true);
    addOP(key, value)
      .then(() => {
        showNotification('success', 'Add OK');
      })
      .catch((e) => {
        showNotification('success', 'Add Error', e);
      })
      .finally(() => {
        setLoading(false);
      });
  };

  return (
    <Stack spacing={1}>
      <Typography variant="h5"> key </Typography>
      <RedisValueEditor
        oldValue={key}
        handleValue={(e) => {
          setKey(e);
        }}
        hexRows={10}
        textRows={10}
        jsonHeigth="40vh"
      ></RedisValueEditor>
      <Typography variant="h5"> value </Typography>
      <RedisValueEditor
        oldValue={value}
        handleValue={(e) => {
          setValue(e);
        }}
        hexRows={10}
        textRows={10}
        jsonHeigth="40vh"
      ></RedisValueEditor>

      <Stack direction="row" spacing={1}>
        <Button
          loading={loading}
          color="primary"
          variant="contained"
          onClick={handleAdd}
        >
          Add
        </Button>
      </Stack>
    </Stack>
  );
}
function RedisAddZset(props: {
  addOP: (key: string, value: number) => Promise<void>;
}) {
  let { addOP } = props;
  const [loading, setLoading] = useState(false);
  const [score, setScore] = useState('');
  const [key, setKey] = useState('');
  const [scoreErr, setScoreErr] = useState(false);

  const { showNotification } = useContext(NotificationContext);

  const handleAdd = () => {
    // reset some state
    setLoading(true);
    addOP(key, parseFloat(score))
      .then(() => {
        showNotification('success', 'Add OK');
      })
      .catch((e) => {
        showNotification('success', 'Add Failure', e);
      })
      .finally(() => {
        setLoading(false);
      });
  };

  return (
    <Stack spacing={1}>
      <TextField
        size="small"
        value={score}
        label="score"
        error={scoreErr}
        helperText="put a valid float64 value"
        onChange={(event) => {
          setScore(event.target.value);
          setScoreErr(isNaN(parseFloat(event.target.value)));
        }}
      ></TextField>

      <RedisValueEditor
        handleValue={(e) => {
          setKey(e);
        }}
        hexRows={17}
        textRows={18}
        jsonHeigth="60vh"
      ></RedisValueEditor>

      <Stack direction="row" spacing={1}>
        <Button
          loading={loading}
          color="primary"
          variant="contained"
          onClick={handleAdd}
        >
          Add
        </Button>
      </Stack>
    </Stack>
  );
}

function RedisEditHash(props: {
  modifyOP: (req: RedisZsetHashModifyReq) => Promise<void>;
  oldKey: string;
  oldValue: string;
  handleClose;
}) {
  const { oldKey, oldValue, modifyOP, handleClose } = props;
  const [loading, setLoading] = useState(false);
  const [newKey, setNewKey] = useState(oldKey);
  const [newValue, setNewValue] = useState(oldValue);
  const { showNotification } = useContext(NotificationContext);

  const handleModify = () => {
    // reset some state
    setLoading(true);
    modifyOP(new RedisZsetHashModifyReq(oldKey, newKey, newValue))
      .then(() => {
        showNotification('success', 'Modify OK');

        handleClose();
      })
      .catch((e) => {
        showNotification('success', 'Modify failure', e);
      })
      .finally(() => {
        setLoading(false);
      });
  };

  return (
    <Stack spacing={1}>
      <Typography variant="h5">key</Typography>
      <RedisValueEditor
        oldValue={newKey}
        handleValue={(e) => {
          setNewKey(e);
        }}
      ></RedisValueEditor>
      <Typography variant="h5">value</Typography>
      <RedisValueEditor
        oldValue={newValue}
        handleValue={(e) => {
          setNewValue(e);
        }}
      ></RedisValueEditor>

      <Stack direction="row" spacing={1}>
        <Button
          loading={loading}
          color="primary"
          variant="contained"
          onClick={handleModify}
        >
          modify
        </Button>
      </Stack>
    </Stack>
  );
}

function RedisEditZset(props: {
  modifyOP: (oldkey: string, newkey: string, value: number) => Promise<void>;
  oldKey: string;
  oldValue: string;
  handleClose: () => void;
}) {
  const { oldKey, oldValue, modifyOP, handleClose } = props;
  const [loading, setLoading] = useState(false);

  const [newKey, setNewKey] = useState(oldKey);
  const [newValue, setNewValue] = useState(oldValue);
  const [newValueErr, setNewValueErr] = useState(false);
  const { showNotification } = useContext(NotificationContext);

  const handleModify = () => {
    if (oldKey == newKey && oldValue == newValue) {
      return;
    }
    // reset some state
    setLoading(true);

    modifyOP(oldKey, newKey, parseInt(newValue))
      .then(() => {
        showNotification('success', 'Modify OK');
        handleClose();
      })
      .catch((e) => {
        showNotification('error', 'Modify Failure', e);
      })
      .finally(() => {
        setLoading(false);
      });
  };

  return (
    <Stack spacing={2}>
      <TextField
        size="small"
        value={newValue}
        label="score"
        error={newValueErr}
        helperText="put a valid float64 value"
        onChange={(event) => {
          setNewValue(event.target.value);
          if (isNaN(parseFloat(event.target.value))) {
            setNewValueErr(true);
          } else {
            setNewValueErr(false);
          }
        }}
      ></TextField>
      <RedisValueEditor
        oldValue={oldKey}
        handleValue={(e) => {
          setNewKey(e);
        }}
      ></RedisValueEditor>

      <Stack direction="row" spacing={1}>
        <Button
          loading={loading}
          color="primary"
          variant="contained"
          onClick={handleModify}
        >
          modify
        </Button>
      </Stack>
    </Stack>
  );
}

function RedisEditModeSelect(props: {
  mode: RedisEditorMode;
  handleChange: (mode: RedisEditorMode) => void;
}) {
  const { mode, handleChange } = props;

  return (
    <Box>
      <FormControl fullWidth>
        <InputLabel id="simple-select-label">viewer</InputLabel>
        <Select
          labelId="simple-select-label"
          id="simple-select"
          value={mode}
          onChange={(e) => {
            handleChange(e.target.value as RedisEditorMode);
          }}
          label="viewer"
        >
          <MenuItem value={RedisEditorMode.Text}>text</MenuItem>
          <MenuItem value={RedisEditorMode.Json}>json</MenuItem>
          <MenuItem value={RedisEditorMode.Hex}>hex</MenuItem>
        </Select>
      </FormControl>
    </Box>
  );
}

function RedisValueEditor(props: {
  oldValue?: string;
  handleValue: (a: string) => void;
  hexRows?: number;
  textRows?: number;
  jsonHeigth?: string;
}) {
  let { oldValue, handleValue, hexRows, textRows, jsonHeigth } = props;
  const [newValue, setNewValue] = useState<string | Uint8Array>('');
  const [editModel, setEditModel] = useState<RedisEditorMode>(
    RedisEditorMode.Text
  );
  useEffect(() => {
    let value = hexTryToUtf8(oldValue);
    if (typeof value === 'string') {
      let isJson = false;
      try {
        isJson = typeof JSON.parse(value) == 'object';
      } catch (e) {}
      if (isJson) {
        setEditModel(RedisEditorMode.Json);
      } else {
        setEditModel(RedisEditorMode.Text);
      }
    } else {
      setEditModel(RedisEditorMode.Hex);
    }
    setNewValue(value);
  }, []);

  useEffect(() => {
    if (editModel == RedisEditorMode.Hex && typeof newValue == 'string') {
      setNewValue((e) => stringToHexArray(e as string));
    }
    if (
      (editModel == RedisEditorMode.Text ||
        editModel == RedisEditorMode.Json) &&
      typeof newValue == 'object'
    ) {
      setNewValue((e) => {
        return uint8ArrayToUtf8(e);
      });
    }
  }, [editModel]);

  const [addLength, setAddLength] = useState('1');
  const [addLengthErr, setAddLengthErr] = useState(false);

  const [nonce, setNonce] = useState(0);
  // The callback facilitates updates to the source data.
  const handleSetValue = React.useCallback(
    (offset, value) => {
      //@ts-ignore
      newValue[offset] = value;
      setNonce((v) => v + 1);
      handleValue(arrayToHex(newValue as Uint8Array));
    },
    [newValue]
  );

  const handleAddLength = () => {
    let length = parseInt(addLength);
    if (isNaN(length)) {
      setAddLengthErr(true);
      return;
    } else {
      setAddLengthErr(false);
    }
    let array = new Uint8Array(newValue.length + length);
    array.set(newValue as Uint8Array);
    setNewValue(array);
    handleValue(arrayToHex(array));
  };

  if (!hexRows) {
    hexRows = 5;
  }
  if (!textRows) {
    textRows = 4;
  }
  if (!jsonHeigth) {
    jsonHeigth = '20vh';
  }
  return (
    <Stack spacing={1}>
      <RedisEditModeSelect
        mode={editModel}
        handleChange={(mode) => {
          setEditModel(mode);
        }}
      ></RedisEditModeSelect>
      {editModel == RedisEditorMode.Text && (
        <TextField
          size="small"
          value={newValue}
          label="key"
          multiline
          rows={textRows}
          onChange={(event) => {
            setNewValue(event.target.value);
            handleValue(stringToHex(event.target.value));
          }}
        ></TextField>
      )}

      {editModel == RedisEditorMode.Json && (
        <SimpleResizeEditor
          language="json"
          // in some times newvalue is not change as a string
          value={typeof newValue == 'object' ? '' : newValue}
          onMount={(editor) => {
            editor.onDidChangeModelContent(() => {
              const value = editor.getValue();
              setNewValue(value);
              handleValue(stringToHex(value));
            });
          }}
          style={{ height: jsonHeigth }}
        ></SimpleResizeEditor>
      )}

      {editModel == RedisEditorMode.Hex && (
        <Stack spacing={1}>
          <HexEditor
            rows={hexRows}
            data={newValue}
            nonce={nonce}
            onSetValue={handleSetValue}
            theme={{ hexEditor: oneDarkPro }}
            showAscii={true}
            showRowLabels={true}
            showColumnLabels={true}
            style={{ width: '100%', height: '100%' }}
          ></HexEditor>

          <Stack direction="row" spacing={1} alignItems="flex-start">
            <TextField
              size="small"
              error={addLengthErr}
              value={addLength}
              helperText="length add the hex"
              onChange={(e) => {
                setAddLength(e.target.value);
              }}
            ></TextField>

            <Button
              color="primary"
              variant="contained"
              onClick={handleAddLength}
            >
              <AddIcon></AddIcon>
            </Button>
          </Stack>
        </Stack>
      )}
    </Stack>
  );
}

function RedisEditList(props: {
  modifyOP: (index: number, newValue: string) => Promise<void>;
  oldValue: string;
  index: number;
  handleClose: () => void;
}) {
  let { oldValue, modifyOP, handleClose, index } = props;
  const [loading, setLoading] = useState(false);
  const [newValue, setNewValue] = useState('');
  const { showNotification } = useContext(NotificationContext);

  // really modify the content.
  const handleModify = () => {
    setLoading(true);
    modifyOP(index, newValue)
      .then(() => {
        setNewValue('');
        showNotification('success', 'Modify OK', '');
        handleClose();
      })
      .catch((e) => {
        showNotification('success', 'Modify Failure', e);
      })
      .finally(() => {
        setLoading(false);
      });
  };

  return (
    <Stack spacing={1}>
      <RedisValueEditor
        oldValue={oldValue}
        handleValue={(e) => {
          setNewValue(e);
        }}
      ></RedisValueEditor>
      <Stack direction="row" spacing={1}>
        <Button
          loading={loading}
          color="primary"
          variant="contained"
          disabled={newValue == ''}
          onClick={handleModify}
        >
          modify
        </Button>
      </Stack>
    </Stack>
  );
}

function RedisEditSet(props: {
  modifyOP: (oldValue: string, newValue: string) => Promise<void>;
  oldValue: string;
  handleClose: () => void;
}) {
  let { oldValue, modifyOP, handleClose } = props;
  const [loading, setLoading] = useState(false);
  const [newValue, setNewValue] = useState('');
  const { showNotification } = useContext(NotificationContext);

  // really modify the content.
  const handleModify = () => {
    setLoading(true);
    modifyOP(oldValue, newValue)
      .then(() => {
        setNewValue('');
        showNotification('success', 'Modify OK');
        handleClose();
      })
      .catch((e) => {
        showNotification('success', 'Modify Failure', e);
      })
      .finally(() => {
        setLoading(false);
      });
  };

  return (
    <Stack spacing={1}>
      <RedisValueEditor
        oldValue={oldValue}
        handleValue={(e) => {
          setNewValue(e);
        }}
      ></RedisValueEditor>
      <Stack direction="row" spacing={1}>
        <Button
          loading={loading}
          color="primary"
          variant="contained"
          disabled={newValue == ''}
          onClick={handleModify}
        >
          modify
        </Button>
      </Stack>
    </Stack>
  );
}

function redisRenderCell(p: any) {
  let value = hexTryToUtf8(p);
  if (typeof value === 'string') {
    return value;
  } else {
    return 'BINARY HEX ' + p;
  }
}

function RedisSetTableContextMenu(props: { delOP: () => void; content }) {
  const { delOP, content } = props;
  const { showNotification } = useContext(NotificationContext);
  return (
    <Paper>
      <MenuList>
        <MenuItem
          onClick={() => {
            handleCopyToClipboard(content, showNotification);
          }}
        >
          <ListItemIcon>
            <ContentCopyIcon></ContentCopyIcon>
          </ListItemIcon>
          <ListItemText> copy </ListItemText>
        </MenuItem>

        <MenuItem
          onClick={() => {
            delOP();
          }}
        >
          <ListItemIcon>
            <DeleteForeverIcon></DeleteForeverIcon>
          </ListItemIcon>
          <ListItemText> delete </ListItemText>
        </MenuItem>
      </MenuList>
    </Paper>
  );
}

function RedisSetTable(props: {
  reloadOP: (keyMatch: string) => Promise<types.MuiDataTableListResp>;
  modifyOP: (oldvalue: string, newValue: string) => Promise<void>;

  delOP: (value: string) => Promise<void>;
}) {
  const { reloadOP, modifyOP, delOP } = props;
  const [keyMatch, setKeyMatch] = useState('');
  const [resp, setResp] = useState<types.MuiDataTableListResp | null>(null);
  const { showNotification } = useContext(NotificationContext);
  const [selectRow, setSelectRow] = useState(null);

  const handleReload = (showOK) => {
    reloadOP(keyMatch)
      .then((resp) => {
        resp.data.data.forEach((e) => {
          e.id = uuidv4();
        });
        resp.data.fields[0]['flex'] = 1;
        resp.data.fields[0]['renderCell'] = (p) => {
          return redisRenderCell(p.row.value);
        };
        setResp(resp);
        if (showOK) {
          reloadOK(showNotification);
        }
      })
      .catch((e) => {
        showNotification('error', 'load data error', e);
      });
  };

  useEffect(() => {
    handleReload(false);
  }, []);

  const handleDel = async (id: string) => {
    let del = resp.data.data.find((e) => e['id'] == id);
    try {
      await delOP(del.value);
      showNotification('success', 'Delete OK');
    } catch (e) {
      showNotification('error', 'delete failure', e);
    }

    handleReload(false);
  };
  const onRowDoubleClick = useCallback((e) => {
    setSelectRow(e.row);
  }, []);

  const vis = useContext(VisiableContext);
  const { setUpMenu } = useContext(ContextMenuContext);
  const slotProps = useMemo(() => {
    return {
      row: {
        onContextMenu: (e) => {
          const { currentTarget, target } = e;
          let content = (target as HTMLDivElement).innerHTML;
          let id = (currentTarget as HTMLDivElement).getAttribute('data-id');
          setUpMenu({
            content: (
              <RedisSetTableContextMenu
                content={content}
                delOP={() => {
                  handleDel(id);
                }}
              ></RedisSetTableContextMenu>
            ),
            style: calMenuStyle(e, 200),
          });
        },
        style: { cursor: 'context-menu' },
      },
    } as GridSlotsComponentsProps;
  }, [resp]);

  return (
    <Stack spacing={1}>
      <Stack direction="row" spacing={1} alignItems="flex-start">
        <TextField
          size="small"
          label="key match could be *key,key*,*key*"
          variant="outlined"
          value={keyMatch}
          onChange={(event) => {
            setKeyMatch(event.target.value);
          }}
        />
        <Button
          variant="contained"
          color="primary"
          onClick={() => {
            handleReload(true);
          }}
        >
          reload
        </Button>
      </Stack>

      {resp && vis && (
        <div
          style={{
            display: 'grid',
            height: datagridContainerHeight(resp.data.data.length),
          }}
        >
          <DataGrid
            showCellVerticalBorder
            showColumnVerticalBorder
            slotProps={slotProps}
            rowHeight={datagridRowHeight}
            columnHeaderHeight={datagridHeaderHeight}
            rows={resp.data.data}
            editMode="row"
            checkboxSelection={false}
            onRowDoubleClick={onRowDoubleClick}
            columns={resp.data.fields}
            disableColumnFilter
          ></DataGrid>
        </div>
      )}

      {selectRow && (
        <Modal
          open={!!selectRow}
          onClose={() => {
            setSelectRow(null);
          }}
          aria-labelledby="modify set"
        >
          <Box sx={modalStyle} width="950px">
            <RedisEditSet
              modifyOP={async (oldValue, newValue) => {
                try {
                  await modifyOP(oldValue, newValue);
                } catch (e) {
                  throw e;
                }
                //@ts-ignore
                selectRow.value = newValue;
                handleReload(false);
              }}
              //@ts-ignore
              key={selectRow.id}
              //@ts-ignore
              oldValue={selectRow.value}
              handleClose={() => {
                setSelectRow(null);
              }}
            ></RedisEditSet>
          </Box>
        </Modal>
      )}
    </Stack>
  );
}

function RedisListTableContextMenu(props: {
  delOP: (count: number) => Promise<void>;
  content;
}) {
  const { delOP, content } = props;

  const [count, setCount] = useState('1');
  const [showDel, setShowDel] = useState(false);
  const { showNotification } = useContext(NotificationContext);
  return (
    <Paper>
      <MenuList>
        <MenuItem
          onClick={() => {
            handleCopyToClipboard(content, showNotification);
          }}
        >
          <ListItemIcon>
            <ContentCopyIcon></ContentCopyIcon>
          </ListItemIcon>
          <ListItemText> copy </ListItemText>
        </MenuItem>

        <MenuItem
          onClick={() => {
            setShowDel(true);
          }}
        >
          <ListItemIcon>
            <DeleteForeverIcon></DeleteForeverIcon>
          </ListItemIcon>
          <ListItemText> delete </ListItemText>
        </MenuItem>
      </MenuList>

      {showDel && (
        <Modal
          open={!!showDel}
          onClose={() => {
            setShowDel(null);
          }}
          aria-labelledby="modify list"
        >
          <Box sx={modalStyle} width="950px">
            <Stack spacing={1}>
              <TextField
                size="small"
                value={count}
                label="delete count"
                onChange={(e) => {
                  setCount(e.target.value);
                }}
                helperText=<span
                  style={{ cursor: 'pointer' }}
                  onClick={() => {
                    openUrl('https://redis.io/docs/latest/commands/lrem/');
                  }}
                >
                  count???
                </span>
                type="number"
              ></TextField>
              <Stack direction="row">
                <Button
                  onClick={async () => {
                    await delOP(parseInt(count));
                    setShowDel(false);
                  }}
                >
                  delete
                </Button>
              </Stack>
            </Stack>
          </Box>
        </Modal>
      )}
    </Paper>
  );
}

function RedisListTable(props: {
  reloadOP: () => Promise<types.MuiDataTableListResp>;
  modifyOP: (index: number, newValue: string) => Promise<void>;
  delOP: (count: number, value: string) => Promise<void>;
}) {
  const { reloadOP, modifyOP, delOP } = props;
  const [resp, setResp] = useState<types.MuiDataTableListResp | null>(null);
  const [resp2, setResp2] = useState<types.MuiDataTableListResp | null>(null);

  const [selectRow, setSelectRow] = useState<any>(null);

  const { showNotification } = useContext(NotificationContext);
  const [pageSize, setPageSize] = useState(
    useContext(SettingsContext).pageSize
  );
  const [page, setPage] = useState(0);

  const handleReload = (showReloadOK: boolean) => {
    reloadOP()
      .then((resp) => {
        resp.data.data.forEach((e) => {
          e.id = uuidv4();
        });
        resp.data.fields[1]['sortable'] = false;
        resp.data.fields[1]['flex'] = 1;
        resp.data.fields[0]['flex'] = 1;
        resp.data.fields[1]['renderCell'] = (p) => {
          return redisRenderCell(p.row.value);
        };
        setResp(resp);
        setResp2(resp);
        if (showReloadOK) {
          reloadOK(showNotification);
        }
      })
      .catch((e) => {
        showNotification('error', 'load data error', e);
      });
  };

  useEffect(() => {
    handleReload(true);
  }, []);

  const handleDel = async (id, deleteCount: number) => {
    let del = resp.data.data.find((e) => e.id == id);
    try {
      await delOP(deleteCount, del.value);
      showNotification('success', 'Delete OK');
    } catch (e) {
      showNotification('error', 'Delete failure', e);
    }
    handleReload(false);
  };

  const onRowDoubleClick = useCallback((e) => {
    setSelectRow(e.row);
  }, []);

  const onPaginationModelChange = useCallback((x: GridPaginationModel) => {
    setPage(x.page);
    setPageSize(x.pageSize);
  }, []);
  const paginationModel = useMemo(() => {
    return { page, pageSize };
  }, [page, pageSize]);
  const onSortModelChange = useCallback(
    (c: GridSortModel) => {
      if (c.length == 0) {
        setResp(resp2);
        return;
      }
      let s = c[0];
      if (!s.sort) {
        setResp(resp2);
        return;
      }
      if (s.sort == 'asc') {
        setResp(resp2);
        return;
      }
      setResp({
        sql: resp2.sql,
        total: resp2.total,
        data: {
          fields: resp2.data.fields,
          data: resp2.data.data.slice().reverse(),
        },
      } as types.MuiDataTableListResp);
    },
    [resp2]
  );
  const vis = useContext(VisiableContext);
  const { setUpMenu } = useContext(ContextMenuContext);
  const slotProps = useMemo(() => {
    return {
      row: {
        onContextMenu: (e) => {
          const { currentTarget, target } = e;
          let content = (target as HTMLDivElement).innerHTML;
          let id = (currentTarget as HTMLDivElement).getAttribute('data-id');
          setUpMenu({
            content: (
              <RedisListTableContextMenu
                delOP={async (count) => {
                  handleDel(id, count);
                }}
                content={content}
              ></RedisListTableContextMenu>
            ),
            style: calMenuStyle(e, 200),
          });
        },
        style: { cursor: 'context-menu' },
      },
    } as GridSlotsComponentsProps;
  }, [resp]);
  return (
    <Stack spacing={1}>
      <Stack direction="row" spacing={1}>
        <Button
          variant="contained" // 可以是 'text', 'outlined', 或 'contained'
          color="primary" // 主题颜色
          onClick={() => {
            handleReload(true);
          }} // 点击事件处理
        >
          reload
        </Button>
      </Stack>
      {resp && vis && (
        <div
          style={{
            display: 'grid',
            height: datagridContainerHeight(resp.data.data.length),
          }}
        >
          <DataGrid
            showCellVerticalBorder
            showColumnVerticalBorder
            slotProps={slotProps}
            rowHeight={datagridRowHeight}
            columnHeaderHeight={datagridHeaderHeight}
            checkboxSelection={false}
            disableMultipleRowSelection
            disableColumnFilter
            rows={resp.data.data}
            columns={resp.data.fields}
            paginationModel={paginationModel}
            onRowDoubleClick={onRowDoubleClick}
            pageSizeOptions={pageSizeOptions}
            onPaginationModelChange={onPaginationModelChange}
            sortingMode="server"
            paginationMode="client"
            onSortModelChange={onSortModelChange}
          ></DataGrid>
        </div>
      )}

      {selectRow && (
        <Modal
          open={!!selectRow}
          onClose={() => {
            setSelectRow(null);
          }}
          aria-labelledby="modify list"
        >
          <Box sx={modalStyle} width="950px">
            <RedisEditList
              modifyOP={async (index, newValue) => {
                try {
                  await modifyOP(index, newValue);
                } catch (e) {
                  throw e;
                }
                handleReload(false);
                selectRow.value = newValue;
              }}
              key={selectRow.id}
              oldValue={selectRow.value}
              index={parseInt(selectRow.seq)}
              handleClose={() => {
                setSelectRow(null);
              }}
            ></RedisEditList>
          </Box>
        </Modal>
      )}
    </Stack>
  );
}

function RedisZSetTableContextMenu(props: { delOP: () => void; content }) {
  const { delOP, content } = props;
  const { showNotification } = useContext(NotificationContext);
  return (
    <Paper>
      <MenuList>
        <MenuItem
          onClick={() => {
            handleCopyToClipboard(content, showNotification);
          }}
        >
          <ListItemIcon>
            <ContentCopyIcon></ContentCopyIcon>
          </ListItemIcon>
          <ListItemText> copy </ListItemText>
        </MenuItem>

        <MenuItem
          onClick={() => {
            delOP();
          }}
        >
          <ListItemIcon>
            <DeleteForeverIcon></DeleteForeverIcon>
          </ListItemIcon>
          <ListItemText> delete </ListItemText>
        </MenuItem>
      </MenuList>
    </Paper>
  );
}

function RedisZsetTable(props: {
  reloadOP: (
    keyMatch: string,
    min: number | null,
    max: number | null
  ) => Promise<types.MuiDataTableListResp>;

  modifyOP: (
    oldMember: string,
    newKey: string,
    newValue: number
  ) => Promise<void>;

  delOP: (key: string) => Promise<void>;
}) {
  const { reloadOP, modifyOP, delOP } = props;
  const [keyMatch, setKeyMatch] = useState('');
  const [resp, setResp] = useState<types.MuiDataTableListResp | null>(null);
  const [selectRow, setSelectRow] = useState<any>();

  const [min, setMin] = useState('');
  const [max, setMax] = useState('');
  const [minErr, setMinErr] = useState(false);
  const [maxErr, setMaxErr] = useState(false);

  const { showNotification } = useContext(NotificationContext);
  const handleReload = (showOK: boolean) => {
    if (minErr || maxErr) {
      return;
    }
    let minValue = parseFloat(min);
    let maxValue = parseFloat(max);

    reloadOP(keyMatch, minValue, maxValue)
      .then((resp) => {
        resp.data.data.forEach((e) => {
          e.id = uuidv4();
        });
        resp.data.fields[0]['flex'] = 1;
        resp.data.fields[1]['flex'] = 1;
        resp.data.fields[0]['renderCell'] = (p) => {
          return redisRenderCell(p.row.key);
        };
        resp.data.data.forEach((e) => {
          //@ts-ignore rewrite to float value
          e.score = parseFloat(e['score']);
        });
        setResp(resp);
        if (showOK) {
          reloadOK(showNotification);
        }
      })
      .catch((e) => {
        showNotification('error', 'load data error', e);
      });
  };

  useEffect(() => {
    handleReload(false);
  }, []);

  const handleDel = async (id: string) => {
    let del = resp.data.data.find((e) => e['id'] == id);
    try {
      await delOP(del.key);
      showNotification('success', 'Delete OK');
    } catch (e) {
      showNotification('error', 'delete failure', e);
    }

    handleReload(false);
  };
  const onRowDoubleClick = useCallback((e) => {
    setSelectRow(e.row);
  }, []);

  const vis = useContext(VisiableContext);
  const { setUpMenu } = useContext(ContextMenuContext);
  const slotProps = useMemo(() => {
    return {
      row: {
        onContextMenu: (e) => {
          const { currentTarget, target } = e;
          let content = (target as HTMLDivElement).innerHTML;
          let id = (currentTarget as HTMLDivElement).getAttribute('data-id');
          setUpMenu({
            content: (
              <RedisZSetTableContextMenu
                content={content}
                delOP={() => {
                  handleDel(id);
                }}
              ></RedisZSetTableContextMenu>
            ),
            style: calMenuStyle(e, 200),
          });
        },
        style: { cursor: 'context-menu' },
      },
    } as GridSlotsComponentsProps;
  }, [resp]);

  return (
    <Stack spacing={1}>
      <Stack direction="row" spacing={1} alignItems="flex-start">
        <TextField
          size="small"
          label="key match could be *key,key*,*key*"
          variant="outlined"
          value={keyMatch}
          onChange={(event) => {
            setKeyMatch(event.target.value);
          }}
        />
        <TextField
          size="small"
          label="minimum value"
          variant="outlined"
          value={min}
          error={minErr}
          helperText="put a valid float64"
          onChange={(event) => {
            if (event.target.value) {
              setMinErr(isNaN(parseFloat(event.target.value)));
            } else {
              setMinErr(false);
            }
            setMin(event.target.value);
          }}
        />

        <TextField
          size="small"
          label="maxiumn value"
          variant="outlined"
          value={max}
          error={maxErr}
          helperText="put a valid float64"
          onChange={(event) => {
            if (event.target.value) {
              setMaxErr(isNaN(parseFloat(event.target.value)));
            } else {
              setMaxErr(false);
            }
            setMax(event.target.value);
          }}
        />
        <Button
          variant="contained"
          color="primary"
          onClick={() => {
            handleReload(true);
          }}
        >
          reload
        </Button>
      </Stack>

      {resp && vis && (
        <div
          style={{
            display: 'grid',
            height: datagridContainerHeight(resp.data.data.length),
          }}
        >
          <DataGrid
            showCellVerticalBorder
            showColumnVerticalBorder
            slotProps={slotProps}
            rowHeight={datagridRowHeight}
            columnHeaderHeight={datagridHeaderHeight}
            rows={resp.data.data}
            checkboxSelection={false}
            editMode="row"
            onRowDoubleClick={onRowDoubleClick}
            columns={resp.data.fields}
            disableColumnFilter
          ></DataGrid>
        </div>
      )}

      {selectRow && (
        <Modal
          open={!!selectRow}
          onClose={() => {
            setSelectRow(null);
          }}
          aria-labelledby="modify zset"
        >
          <Box sx={modalStyle} width="950px">
            <RedisEditZset
              modifyOP={async (oldKey, newKey, value) => {
                try {
                  await modifyOP(oldKey, newKey, value);
                } catch (e) {
                  throw e;
                }
                handleReload(false);
              }}
              key={selectRow}
              oldValue={selectRow.score}
              oldKey={selectRow.key}
              handleClose={() => {
                setSelectRow(null);
              }}
            ></RedisEditZset>
          </Box>
        </Modal>
      )}
    </Stack>
  );
}

function RedisHashTableContextMenu(props: { delOP: () => void; content }) {
  const { delOP, content } = props;

  const { showNotification } = useContext(NotificationContext);
  return (
    <Paper>
      <MenuList>
        <MenuItem
          onClick={() => {
            handleCopyToClipboard(content, showNotification);
          }}
        >
          <ListItemIcon>
            <ContentCopyIcon></ContentCopyIcon>
          </ListItemIcon>
          <ListItemText> copy </ListItemText>
        </MenuItem>

        <MenuItem
          onClick={() => {
            delOP();
          }}
        >
          <ListItemIcon>
            <DeleteForeverIcon></DeleteForeverIcon>
          </ListItemIcon>
          <ListItemText> delete </ListItemText>
        </MenuItem>
      </MenuList>
    </Paper>
  );
}

function RedisHashTable(props: {
  reloadOP: (keyMatch: string) => Promise<types.MuiDataTableListResp>;
  modifyOP: (
    oldMember: string,
    newMember: string,
    newValue: string
  ) => Promise<void>;

  delOP: (key: string) => Promise<void>;
}) {
  const { reloadOP, modifyOP, delOP } = props;
  const [keyMatch, setKeyMatch] = useState('');
  const [resp, setResp] = useState<types.MuiDataTableListResp | null>(null);
  const { showNotification } = useContext(NotificationContext);
  const [selectRow, setSelectRow] = useState<any>(null);

  const handleReload = (showOK) => {
    reloadOP(keyMatch)
      .then((resp) => {
        resp.data.data.forEach((e) => {
          e.id = uuidv4();
        });
        resp.data.fields[0]['flex'] = 1;
        resp.data.fields[1]['flex'] = 1;
        resp.data.fields[0]['renderCell'] = (p) => {
          return redisRenderCell(p.row.key);
        };
        resp.data.fields[1]['renderCell'] = (p) => {
          return redisRenderCell(p.row.value);
        };
        setResp(resp);
        if (showOK) {
          reloadOK(showNotification);
        }
      })
      .catch((e) => {
        showNotification('error', 'load data error', e);
      });
  };

  useEffect(() => {
    handleReload(false);
  }, []);

  const handleDel = async (id: string) => {
    let del = resp.data.data.find((e) => e['id'] == id);
    try {
      await delOP(del.key);
      showNotification('success', 'Delete OK');
    } catch (e) {
      showNotification('error', 'delete failure', e);
    }

    handleReload(false);
  };
  const onRowDoubleClick = useCallback((e) => {
    setSelectRow(e.row);
  }, []);

  const vis = useContext(VisiableContext);
  const { setUpMenu } = useContext(ContextMenuContext);
  const slotProps = useMemo(() => {
    return {
      row: {
        onContextMenu: (e) => {
          const { currentTarget, target } = e;
          let content = (target as HTMLDivElement).innerHTML;
          let id = (currentTarget as HTMLDivElement).getAttribute('data-id');
          setUpMenu({
            content: (
              <RedisHashTableContextMenu
                content={content}
                delOP={() => {
                  handleDel(id);
                }}
              ></RedisHashTableContextMenu>
            ),
            style: calMenuStyle(e, 200),
          });
        },
        style: { cursor: 'context-menu' },
      },
    } as GridSlotsComponentsProps;
  }, [resp]);

  return (
    <Stack spacing={1}>
      <Stack direction="row" spacing={1} alignItems="flex-start">
        <TextField
          size="small"
          label="key match could be *key,key*,*key*"
          variant="outlined" // 可以是 'outlined', 'filled', or 'standard'
          value={keyMatch}
          onChange={(event) => {
            setKeyMatch(event.target.value);
          }}
        />
        <Button
          variant="contained" // 可以是 'text', 'outlined', 或 'contained'
          color="primary" // 主题颜色
          onClick={() => {
            handleReload(true);
          }} // 点击事件处理
        >
          reload
        </Button>
      </Stack>

      {resp && vis && (
        <div
          style={{
            display: 'grid',
            height: datagridContainerHeight(resp.data.data.length),
          }}
        >
          <DataGrid
            showCellVerticalBorder
            showColumnVerticalBorder
            slotProps={slotProps}
            rowHeight={datagridRowHeight}
            columnHeaderHeight={datagridHeaderHeight}
            rows={resp.data.data}
            checkboxSelection={false}
            editMode="row"
            onRowDoubleClick={onRowDoubleClick}
            columns={resp.data.fields}
            disableColumnFilter
          ></DataGrid>
        </div>
      )}

      {selectRow && (
        <Modal
          open={!!selectRow}
          onClose={() => {
            setSelectRow(null);
          }}
          aria-labelledby="modify hash"
        >
          <Box sx={modalStyle} width="950px">
            <RedisEditHash
              modifyOP={async (req) => {
                try {
                  await modifyOP(req.oldKey, req.newKey, req.value);
                } catch (e) {
                  throw e;
                }
                selectRow.key = req.newKey;
                selectRow.value = req.value;
                handleReload(false);
              }}
              handleClose={() => {
                setSelectRow(null);
              }}
              key={selectRow.id}
              oldValue={selectRow.value}
              oldKey={selectRow.key}
            ></RedisEditHash>
          </Box>
        </Modal>
      )}
    </Stack>
  );
}

function RedisModifyKeyName(props: {
  name: string;
  modifyOP: (oldName: string, newName: string) => Promise<void>;
}) {
  let { name, modifyOP } = props;
  const [newName, setNewName] = useState(name);
  const { showNotification } = useContext(NotificationContext);
  const [loading, setLoading] = useState(false);

  const handleModify = () => {
    setLoading(true);
    modifyOP(newName, name)
      .then(() => {
        showNotification('success', 'Modify OK');
      })
      .catch((e) => {
        showNotification('error', 'Modify failure', e);
      })
      .finally(() => {
        setLoading(false);
      });
  };

  return (
    <Stack spacing={1}>
      <Stack direction="row">
        <Typography variant="subtitle1"> New name: </Typography>
      </Stack>

      <TextField
        size="small"
        value={newName}
        onChange={(e) => {
          setNewName(e.target.value);
        }}
        label="name"
      ></TextField>
      <Stack direction="row" spacing={1}>
        <Button
          loading={loading}
          disabled={name == newName}
          color="primary"
          variant="contained"
          onClick={handleModify}
        >
          Save
        </Button>
      </Stack>
    </Stack>
  );
}

export {
  RedisListTable,
  RedisStringTable,
  RedisHashTable,
  RedisZsetTable,
  RedisSetTable,
  RedisValueEditor,
  RedisModifyKeyName,
  // add
  RedisAddSet,
  RedisAddHash,
  RedisAddList,
  RedisAddZset,
};
