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
} from '@mui/material';
import { v4 as uuidv4 } from 'uuid';

import DeleteForeverIcon from '@mui/icons-material/DeleteForever';
import {
  modalStyle,
  pageSizeOptions,
  datagridRowHeight,
  datagridHeaderHeight,
  progressBarSize,
  SettingsContext,
  reloadOK,
  VisiableContext,
} from './Common.tsx';

import { types } from './ApiStruct.ts';

import { NotificationContext } from './Notification';
import {
  DataGrid,
  GridPaginationModel,
  GridRowSelectionModel,
  GridSortModel,
} from '@mui/x-data-grid';
import HexEditor from 'react-hex-editor';
import oneDarkPro from 'react-hex-editor/themes/oneDarkPro';
import AddIcon from '@mui/icons-material/Add';
import { secondConfirm } from './Confirm.tsx';
import {
  arrayToHex,
  hexToUint8Array,
  hexToUtf8,
  hexTryToUtf8,
  stringToHex,
} from './helper.ts';

class RedisZsetHashModifyReq {
  constructor(
    public oldKey: string,
    public newKey: string,
    public value: string
  ) {}
}

enum RedisEditMode {
  Hex,
  Text,
}

function RedisListAdd(props: {
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
        setLoading(false);
        showNotification('success', 'Push OK');
      })
      .catch((e) => {
        setLoading(false);
        showNotification('error', 'Push OK', e);
      });
  };

  return (
    <Stack spacing={1}>
      {loading && <CircularProgress size={progressBarSize} />}
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
      <RedisEditHexOrText handleValue={(e) => setKey(e)}></RedisEditHexOrText>

      <Stack direction="row" spacing={1}>
        <Button
          size="small"
          color="primary"
          variant="contained"
          onClick={handleAdd}
        >
          PUSH
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
        setLoading(false);
        showNotification('success', 'Modify OK');
      })
      .catch((e) => {
        setLoading(false);
        showNotification('error', 'Modify failure', e);
      });
  };

  return (
    <Stack spacing={1}>
      <RedisEditHexOrText
        oldValue={newContent}
        handleValue={(e) => {
          setNewConent(e);
        }}
        textRows={10}
        hexRows={0x10}
      ></RedisEditHexOrText>

      <Stack width="100px" direction="row" spacing={1}>
        <Button
          size="small"
          color="primary"
          variant="contained"
          onClick={() => {
            handleModify();
          }}
        >
          Set
        </Button>
        {loading && <CircularProgress size={progressBarSize} />}
      </Stack>
    </Stack>
  );
}

function RedisAddSet(props: { addOP: (key: string) => Promise<void> }) {
  var { addOP } = props;
  const [loading, setLoading] = useState(false);
  const [key, setKey] = useState('');
  const { showNotification } = useContext(NotificationContext);
  const handleAdd = () => {
    // reset some state
    setLoading(true);
    addOP(key)
      .then(() => {
        showNotification('success', 'Add ok');
        setLoading(false);
      })
      .catch((e) => {
        showNotification('error', 'Add error', e);
        setLoading(false);
      });
  };

  return (
    <Stack spacing={1}>
      <RedisEditHexOrText
        handleValue={(e) => {
          setKey(e);
        }}
      ></RedisEditHexOrText>

      <Stack direction="row" spacing={1}>
        <Button
          size="small"
          color="primary"
          variant="contained"
          onClick={handleAdd}
        >
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
  var { addOP } = props;
  const [loading, setLoading] = useState(false);
  const [value, setValue] = useState('');
  const [key, setKey] = useState('');

  const { showNotification } = useContext(NotificationContext);
  const handleAdd = () => {
    // reset some state
    setLoading(true);
    addOP(key, value)
      .then(() => {
        setLoading(false);
        showNotification('success', 'Add OK');
      })
      .catch((e) => {
        setLoading(false);
        showNotification('success', 'Add Error', e);
      });
  };

  return (
    <Stack spacing={1}>
      {loading && <CircularProgress size={progressBarSize} />}
      <Typography variant="h5"> key </Typography>
      <RedisEditHexOrText
        oldValue={key}
        handleValue={(e) => {
          setKey(e);
        }}
      ></RedisEditHexOrText>
      <Typography variant="h5"> value </Typography>
      <RedisEditHexOrText
        oldValue={value}
        handleValue={(e) => {
          setValue(e);
        }}
      ></RedisEditHexOrText>

      <Stack direction="row" spacing={1}>
        <Button
          size="small"
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
  handleClose: () => void;
}) {
  var { addOP } = props;
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
        setLoading(false);
        showNotification('success', 'Add OK');
      })
      .catch((e) => {
        setLoading(false);
        showNotification('success', 'Add Failure', e);
      });
  };

  return (
    <Stack spacing={1}>
      {loading && <CircularProgress size={progressBarSize} />}
      <TextField
        value={score}
        label="score"
        error={scoreErr}
        helperText="put a valid float64 value"
        onChange={(event) => {
          setScore(event.target.value);
          setScoreErr(isNaN(parseFloat(event.target.value)));
        }}
        size="small"
      ></TextField>

      <RedisEditHexOrText
        handleValue={(e) => {
          setKey(e);
        }}
      ></RedisEditHexOrText>

      <Stack direction="row" spacing={1}>
        <Button
          size="small"
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
        setLoading(false);
        handleClose();
      })
      .catch((e) => {
        showNotification('success', 'Modify failure', e);
        setLoading(false);
      });
  };

  return (
    <Stack spacing={1}>
      {loading && <CircularProgress size={progressBarSize} />}
      <Typography variant="h5">key</Typography>
      <RedisEditHexOrText
        oldValue={newKey}
        handleValue={(e) => {
          setNewKey(e);
        }}
      ></RedisEditHexOrText>
      <Typography variant="h5">value</Typography>
      <RedisEditHexOrText
        oldValue={newValue}
        handleValue={(e) => {
          setNewValue(e);
        }}
      ></RedisEditHexOrText>

      <Stack direction="row" spacing={1}>
        <Button
          size="small"
          color="primary"
          variant="contained"
          onClick={handleModify}
        >
          Modify
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
        setLoading(false);
        showNotification('success', 'Modify OK');
        handleClose();
      })
      .catch((e) => {
        setLoading(false);
        showNotification('error', 'Modify Failure', e);
      });
  };

  return (
    <Stack spacing={2}>
      {loading && <CircularProgress size={progressBarSize} />}
      <TextField
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
        size="small"
      ></TextField>
      <RedisEditHexOrText
        oldValue={oldKey}
        handleValue={(e) => {
          setNewKey(e);
        }}
      ></RedisEditHexOrText>

      <Stack direction="row" spacing={1}>
        <Button
          size="small"
          color="primary"
          variant="contained"
          onClick={handleModify}
        >
          Modify
        </Button>
      </Stack>
    </Stack>
  );
}

function RedisEditModeSelect(props: {
  mode: RedisEditMode;
  handleChange: (mode: RedisEditMode) => void;
}) {
  const { mode, handleChange } = props;

  return (
    <Box sx={{ minWidth: 100 }}>
      <FormControl fullWidth>
        <InputLabel id="simple-select-label">viewer</InputLabel>
        <Select
          labelId="simple-select-label"
          id="simple-select"
          value={mode}
          onChange={(e) => {
            handleChange(e.target.value as RedisEditMode);
          }}
          label="viewer"
        >
          <MenuItem value={RedisEditMode.Hex}>hex</MenuItem>
          <MenuItem value={RedisEditMode.Text}>text</MenuItem>
        </Select>
      </FormControl>
    </Box>
  );
}

function RedisEditHexOrText(props: {
  oldValue?: string;
  handleValue: (a: string) => void;
  hexRows?: number;
  textRows?: number;
}) {
  let { oldValue, handleValue, hexRows, textRows } = props;
  const [newValue, setNewValue] = useState<String | Uint8Array>('');
  const [editModel, setEditModel] = useState<RedisEditMode>(RedisEditMode.Text);
  useEffect(() => {
    let value = hexTryToUtf8(oldValue);
    if (typeof value === 'string') {
      setEditModel(RedisEditMode.Text);
    } else {
      setEditModel(RedisEditMode.Hex);
    }
    setNewValue(value);
  }, []);

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

  useEffect(() => {
    switch (editModel) {
      case RedisEditMode.Text:
        let v = hexToUtf8(oldValue);
        setNewValue(v);
        break;
      case RedisEditMode.Hex:
        setNewValue(hexToUint8Array(oldValue));
        break;
    }
  }, [editModel]);

  if (!hexRows) {
    hexRows = 0x5;
  }
  if (!textRows) {
    textRows = 4;
  }
  return (
    <Stack spacing={1}>
      <RedisEditModeSelect
        mode={editModel}
        handleChange={(mode) => {
          setEditModel(mode);
        }}
      ></RedisEditModeSelect>
      {editModel == RedisEditMode.Text && (
        <TextField
          value={newValue}
          label="key"
          multiline
          rows={textRows}
          onChange={(event) => {
            setNewValue(event.target.value);
            handleValue(stringToHex(event.target.value));
          }}
          size="small"
        ></TextField>
      )}

      {editModel == RedisEditMode.Hex && (
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
              error={addLengthErr}
              value={addLength}
              helperText="length add the hex"
              onChange={(e) => {
                setAddLength(e.target.value);
              }}
              size="small"
            ></TextField>

            <Button
              color="primary"
              size="small"
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
        setLoading(false);
        setNewValue('');
        showNotification('success', 'Modify OK', '');
        handleClose();
      })
      .catch((e) => {
        setLoading(false);
        showNotification('success', 'Modify Failure', e);
      });
  };

  return (
    <Stack spacing={1}>
      {loading && <CircularProgress size={progressBarSize} />}
      <RedisEditHexOrText
        oldValue={oldValue}
        handleValue={(e) => {
          setNewValue(e);
        }}
      ></RedisEditHexOrText>
      <Stack direction="row" spacing={1}>
        <Button
          size="small"
          color="primary"
          variant="contained"
          disabled={newValue == ''}
          onClick={handleModify}
        >
          Modify
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
        setLoading(false);
        setNewValue('');
        showNotification('success', 'Modify OK');
        handleClose();
      })
      .catch((e) => {
        setLoading(false);
        showNotification('success', 'Modify Failure', e);
      });
  };

  return (
    <Stack spacing={1}>
      {loading && <CircularProgress size={progressBarSize} />}
      <RedisEditHexOrText
        oldValue={oldValue}
        handleValue={(e) => {
          setNewValue(e);
        }}
      ></RedisEditHexOrText>
      <Stack direction="row" spacing={1}>
        <Button
          size="small"
          color="primary"
          variant="contained"
          disabled={newValue == ''}
          onClick={handleModify}
        >
          Modify
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

function RedisSetTable(props: {
  reloadOP: (keyMatch: string) => Promise<types.MuiDataTableListResp>;
  modifyOP: (oldvalue: string, newValue: string) => Promise<void>;
  addOP: (value: string) => Promise<void>;
  delOP: (value: string) => Promise<void>;
}) {
  const { reloadOP, modifyOP, addOP, delOP } = props;
  const [keyMatch, setKeyMatch] = useState('');
  const [resp, setResp] = useState<types.MuiDataTableListResp | null>(null);
  const { showNotification } = useContext(NotificationContext);
  const [selectRow, setSelectRow] = useState(null);
  const [showAdd, setShowAdd] = useState(false);
  const [selectionModel, setSelectionModel] = useState([]);
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

  const getIDIndex = (id: string): number => {
    let ret;
    resp.data.data.forEach((e, index) => {
      if (e.id === id) {
        ret = index;
      }
    });
    return ret;
  };

  useEffect(() => {
    handleReload(false);
  }, []);

  const handleDel = async () => {
    for (let i = 0; i < selectionModel.length; i++) {
      let index = getIDIndex(selectionModel[i]);
      let value = resp.data.data[index]['value'];
      try {
        await delOP(value);
        showNotification('success', 'Delete OK');
      } catch (e) {
        showNotification('error', 'delete failure', e);
      }
    }
    handleReload(false);
  };
  const onRowDoubleClick = useCallback((e) => {
    setSelectRow(e.row);
  }, []);
  const onRowSelectionModelChange = useCallback(
    (rows: GridRowSelectionModel) => {
      setSelectionModel(rows as string[]);
    },
    []
  );
  const vis = useContext(VisiableContext);
  return (
    <Stack spacing={1}>
      <Stack direction="row" spacing={1} alignItems="flex-start">
        <TextField
          label="key match could be *key,key*,*key*"
          variant="outlined" // 可以是 'outlined', 'filled', or 'standard'
          value={keyMatch}
          onChange={(event) => {
            setKeyMatch(event.target.value);
          }}
          size="small"
        />
        <Button
          size="small"
          variant="contained" // 可以是 'text', 'outlined', 或 'contained'
          color="primary" // 主题颜色
          onClick={() => {
            handleReload(true);
          }} // 点击事件处理
        >
          Reload
        </Button>
      </Stack>

      {resp && (
        <Stack spacing={1}>
          <Stack direction="row" spacing={1}>
            <Button
              size="small"
              variant="contained"
              color="error"
              disabled={selectionModel.length == 0}
              onClick={() => {
                secondConfirm({
                  title: 'Are u sure',
                  content: 'Are you sure to delete those data',
                  confirmText: 'Delete',
                }).then((confirm) => {
                  if (confirm) {
                    handleDel();
                  }
                });
              }}
            >
              <DeleteForeverIcon color="error"></DeleteForeverIcon>
            </Button>
            <Button
              size="small"
              variant="contained"
              color="primary"
              onClick={() => {
                setShowAdd(true);
              }}
            >
              <AddIcon></AddIcon>
            </Button>
          </Stack>

          {/* actual data grid */}
          {vis && (
            <div style={{ display: 'grid' }}>
              <DataGrid
                rowHeight={datagridRowHeight}
                columnHeaderHeight={datagridHeaderHeight}
                rows={resp.data.data}
                checkboxSelection
                editMode="row"
                onRowDoubleClick={onRowDoubleClick}
                columns={resp.data.fields}
                disableColumnFilter
                onRowSelectionModelChange={onRowSelectionModelChange}
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
              <Box sx={modalStyle} width="800px">
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
      )}
      {showAdd && (
        <Modal
          open={showAdd}
          onClose={() => {
            setShowAdd(false);
          }}
          aria-labelledby="modify set"
        >
          <Box sx={modalStyle} width="800px">
            <RedisAddSet
              addOP={async (value) => {
                try {
                  await addOP(value);
                } catch (e) {
                  throw e;
                }
                handleReload(false);
              }}
            ></RedisAddSet>
          </Box>
        </Modal>
      )}
    </Stack>
  );
}

function RedisListTable(props: {
  reloadOP: () => Promise<types.MuiDataTableListResp>;
  modifyOP: (index: number, newValue: string) => Promise<void>;
  lpushOP: (value: string) => Promise<void>;
  rpushOP: (value: string) => Promise<void>;
  delOP: (count: number, value: string) => Promise<void>;
}) {
  const { reloadOP, modifyOP, lpushOP, delOP, rpushOP } = props;
  const [resp, setResp] = useState<types.MuiDataTableListResp | null>(null);
  const [resp2, setResp2] = useState<types.MuiDataTableListResp | null>(null);

  const [selectRow, setSelectRow] = useState<any>(null);
  const [showAdd, setShowAdd] = useState(false);
  const [selectionModel, setSelectionModel] = useState('');

  const { showNotification } = useContext(NotificationContext);
  const [deleteCount, setDeleteCunnt] = useState<string | number>(0);
  const [deleteCountErr, setDeleteCountErr] = useState(false);
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

  const getIDIndex = (id: string): number => {
    let ret;
    resp.data.data.forEach((e, index) => {
      if (e.id === id) {
        ret = index;
      }
    });
    return ret;
  };

  const handleDel = async () => {
    let value = resp.data.data[getIDIndex(selectionModel)]['value'];
    try {
      await delOP(
        typeof deleteCount === 'string' ? parseInt(deleteCount) : deleteCount,
        value
      );
      showNotification('success', 'Delete OK');
    } catch (e) {
      showNotification('error', 'Delete failure', e);
    }
    handleReload(false);
  };
  const onRowDoubleClick = useCallback((e) => {
    setSelectRow(e.row);
  }, []);
  const onRowSelectionModelChange = useCallback(
    (rows: GridRowSelectionModel) => {
      setSelectionModel(rows.length > 0 ? (rows[0] as string) : '');
    },
    []
  );
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
  return (
    <Stack spacing={1}>
      <Stack direction="row" spacing={1}>
        <Button
          size="small"
          variant="contained" // 可以是 'text', 'outlined', 或 'contained'
          color="primary" // 主题颜色
          onClick={() => {
            handleReload(true);
          }} // 点击事件处理
        >
          Reload
        </Button>
      </Stack>
      {resp && (
        <Stack spacing={1}>
          {/* delete Button  */}
          <Stack direction="row" spacing={1} alignItems="flex-start">
            <TextField
              size="small"
              value={deleteCount}
              error={deleteCountErr}
              helperText="put a valid integer"
              onChange={(e) => {
                setDeleteCountErr(isNaN(parseInt(e.target.value)));
                setDeleteCunnt(e.target.value);
              }}
              inputProps={{ type: 'number' }}
            />
            <Button
              size="small"
              variant="contained"
              color="error"
              disabled={selectionModel === '' || deleteCountErr}
              onClick={() => {
                secondConfirm({
                  title: 'Delete',
                  content: 'Are u sure to Delete',
                  confirmText: 'Delete',
                }).then((confirm) => {
                  if (confirm) {
                    handleDel();
                  }
                });
              }}
            >
              <DeleteForeverIcon color="error"></DeleteForeverIcon>
            </Button>
            <Button
              size="small"
              variant="contained"
              color="primary"
              onClick={() => {
                setShowAdd(true);
              }}
            >
              <AddIcon></AddIcon>
            </Button>
          </Stack>

          {/* actual data grid */}
          {vis && (
            <div style={{ display: 'grid' }}>
              <DataGrid
                rowHeight={datagridRowHeight}
                columnHeaderHeight={datagridHeaderHeight}
                checkboxSelection
                disableMultipleRowSelection
                disableColumnFilter
                rows={resp.data.data}
                columns={resp.data.fields}
                paginationModel={paginationModel}
                onRowDoubleClick={onRowDoubleClick}
                onRowSelectionModelChange={onRowSelectionModelChange}
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
              <Box sx={modalStyle} width="800px">
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
      )}

      {/* // add modal */}
      {showAdd && (
        <Modal
          open={showAdd}
          onClose={() => {
            setShowAdd(false);
          }}
          aria-labelledby="modify list"
        >
          <Box sx={modalStyle} width="800px">
            <RedisListAdd
              lpushOP={async (value) => {
                try {
                  await lpushOP(value);
                } catch (e) {
                  throw e;
                }
                handleReload(false);
              }}
              rpushOP={async (value) => {
                try {
                  await rpushOP(value);
                } catch (e) {
                  throw e;
                }
                handleReload(false);
              }}
            ></RedisListAdd>
          </Box>
        </Modal>
      )}
    </Stack>
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
  addOP: (key: string, value: number) => Promise<void>;
  delOP: (key: string) => Promise<void>;
}) {
  const { reloadOP, modifyOP, addOP, delOP } = props;
  const [keyMatch, setKeyMatch] = useState('');
  const [resp, setResp] = useState<types.MuiDataTableListResp | null>(null);
  const [selectRow, setSelectRow] = useState<any>();
  const [showAdd, setShowAdd] = useState(false);
  const [min, setMin] = useState('');
  const [max, setMax] = useState('');
  const [minErr, setMinErr] = useState(false);
  const [maxErr, setMaxErr] = useState(false);

  const [selectionModel, setSelectionModel] = useState([]);

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
  const getIDIndex = (id: string): number => {
    let ret;
    resp.data.data.forEach((e, index) => {
      if (e.id === id) {
        ret = index;
      }
    });
    return ret;
  };

  const handleDel = async () => {
    for (let i = 0; i < selectionModel.length; i++) {
      let index = getIDIndex(selectionModel[i]);
      let value = resp.data.data[index]['key'];
      try {
        await delOP(value);
        showNotification('success', 'Delete OK');
      } catch (e) {
        showNotification('error', 'delete failure', e);
      }
    }
    handleReload(false);
  };
  const onRowDoubleClick = useCallback((e) => {
    setSelectRow(e.row);
  }, []);
  const onRowSelectionModelChange = useCallback(
    (rows: GridRowSelectionModel) => {
      setSelectionModel(rows as string[]);
    },
    []
  );
  const vis = useContext(VisiableContext);
  return (
    <Stack spacing={1}>
      <Stack direction="row" spacing={1} alignItems="flex-start">
        <TextField
          label="key match could be *key,key*,*key*"
          variant="outlined"
          value={keyMatch}
          onChange={(event) => {
            setKeyMatch(event.target.value);
          }}
          size="small"
        />
        <TextField
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
          size="small"
        />

        <TextField
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
          size="small"
        />
        <Button
          size="small"
          variant="contained"
          color="primary"
          onClick={() => {
            handleReload(true);
          }}
        >
          Reload
        </Button>
      </Stack>

      {resp && (
        <Stack spacing={1}>
          <Stack direction="row" spacing={1}>
            <Button
              size="small"
              variant="contained"
              color="error"
              disabled={selectionModel.length == 0}
              onClick={() => {
                secondConfirm({
                  title: 'Are u sure',
                  content: 'Are you sure to delete those data',
                  confirmText: 'Delete',
                }).then((confirm) => {
                  if (confirm) {
                    handleDel();
                  }
                });
              }}
            >
              <DeleteForeverIcon color="error"></DeleteForeverIcon>
            </Button>
            <Button
              size="small"
              variant="contained"
              color="primary"
              onClick={() => {
                setShowAdd(true);
              }}
            >
              <AddIcon />
            </Button>
          </Stack>

          {/* actual data grid */}
          {vis && (
            <div style={{ display: 'grid' }}>
              <DataGrid
                rowHeight={datagridRowHeight}
                columnHeaderHeight={datagridHeaderHeight}
                rows={resp.data.data}
                checkboxSelection
                editMode="row"
                onRowDoubleClick={onRowDoubleClick}
                columns={resp.data.fields}
                disableColumnFilter
                onRowSelectionModelChange={onRowSelectionModelChange}
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
              <Box sx={modalStyle} width="800px">
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
      )}

      {/* // add modal */}
      {showAdd && (
        <Modal
          open={showAdd}
          onClose={() => {
            setShowAdd(false);
          }}
          aria-labelledby="modify zset"
        >
          <Box sx={modalStyle} width="800px">
            <RedisAddZset
              addOP={async (key, value) => {
                try {
                  await addOP(key, value);
                } catch (e) {
                  throw e;
                }
                handleReload(false);
              }}
              handleClose={() => {
                setShowAdd(false);
              }}
            ></RedisAddZset>
          </Box>
        </Modal>
      )}
    </Stack>
  );
}

function RedisHashTable(props: {
  reloadOP: (keyMatch: string) => Promise<types.MuiDataTableListResp>;
  modifyOP: (
    oldMember: string,
    newMember: string,
    newValue: string
  ) => Promise<void>;
  addOP: (key: string, value: string) => Promise<void>;
  delOP: (key: string) => Promise<void>;
}) {
  const { reloadOP, modifyOP, addOP, delOP } = props;
  const [keyMatch, setKeyMatch] = useState('');
  const [resp, setResp] = useState<types.MuiDataTableListResp | null>(null);
  const { showNotification } = useContext(NotificationContext);
  const [selectRow, setSelectRow] = useState<any>(null);
  const [showAdd, setShowAdd] = useState(false);
  const [selectionModel, setSelectionModel] = useState([]);
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

  const getIDIndex = (id: string): number => {
    let ret;
    resp.data.data.forEach((e, index) => {
      if (e.id === id) {
        ret = index;
      }
    });
    return ret;
  };

  const handleDel = async () => {
    for (let i = 0; i < selectionModel.length; i++) {
      let id = selectionModel[i];
      let index = getIDIndex(id);
      let value = resp.data.data[index]['key'];
      try {
        await delOP(value);
        showNotification('success', 'Delete OK');
      } catch (e) {
        showNotification('error', 'delete failure', e);
      }
    }
    handleReload(false);
  };
  const onRowDoubleClick = useCallback((e) => {
    setSelectRow(e.row);
  }, []);
  const onRowSelectionModelChange = useCallback(
    (rows: GridRowSelectionModel) => {
      setSelectionModel(rows as string[]);
    },
    []
  );
  const vis = useContext(VisiableContext);
  return (
    <Stack spacing={1}>
      <Stack direction="row" spacing={1} alignItems="flex-start">
        <TextField
          label="key match could be *key,key*,*key*"
          variant="outlined" // 可以是 'outlined', 'filled', or 'standard'
          value={keyMatch}
          onChange={(event) => {
            setKeyMatch(event.target.value);
          }}
          size="small"
        />
        <Button
          size="small"
          variant="contained" // 可以是 'text', 'outlined', 或 'contained'
          color="primary" // 主题颜色
          onClick={() => {
            handleReload(true);
          }} // 点击事件处理
        >
          Reload
        </Button>
      </Stack>

      {resp && (
        <Stack spacing={1}>
          <Stack direction="row" spacing={1}>
            <Button
              size="small"
              variant="contained"
              color="error"
              disabled={selectionModel.length == 0}
              onClick={() => {
                secondConfirm({
                  title: 'Are u sure',
                  content: 'Are you sure to delete those data',
                  confirmText: 'Delete',
                }).then((confirm) => {
                  if (confirm) {
                    handleDel();
                  }
                });
              }}
            >
              <DeleteForeverIcon color="error"></DeleteForeverIcon>
            </Button>
            <Button
              size="small"
              variant="contained"
              color="primary"
              onClick={() => {
                setShowAdd(true);
              }}
            >
              <AddIcon></AddIcon>
            </Button>
          </Stack>

          {/* actual data grid */}
          {vis && (
            <div style={{ display: 'grid' }}>
              <DataGrid
                rowHeight={datagridRowHeight}
                columnHeaderHeight={datagridHeaderHeight}
                rows={resp.data.data}
                checkboxSelection
                editMode="row"
                onRowDoubleClick={onRowDoubleClick}
                columns={resp.data.fields}
                disableColumnFilter
                onRowSelectionModelChange={onRowSelectionModelChange}
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
              <Box sx={modalStyle} width="800px">
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
      )}

      {showAdd && (
        <Modal
          open={showAdd}
          onClose={() => {
            setShowAdd(false);
          }}
          aria-labelledby="modify hash"
        >
          <Box sx={modalStyle} width="800px">
            <RedisAddHash
              addOP={async (key, value) => {
                try {
                  await addOP(key, value);
                } catch (e) {
                  throw e;
                }
                handleReload(false);
              }}
            ></RedisAddHash>
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
        setLoading(false);
        showNotification('success', 'Modify OK');
      })
      .catch((e) => {
        setLoading(false);
        showNotification('error', 'Modify failure', e);
      });
  };

  return (
    <Stack spacing={1}>
      <Stack direction="row">
        <Typography variant="subtitle1"> New name: </Typography>
        {loading && <CircularProgress size={progressBarSize} />}
      </Stack>

      <TextField
        value={newName}
        onChange={(e) => {
          setNewName(e.target.value);
        }}
        label="name"
        size="small"
      ></TextField>
      <Stack direction="row" spacing={1}>
        <Button
          size="small"
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
  RedisEditHexOrText,
  RedisModifyKeyName,
};
