import React, { useContext, useEffect, useState } from 'react';
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
  CloseSpace,
  hexToUint8Array,
  hexToUtf8,
  hexTryToUtf8,
  modalStyle,
  stringToHex,
  arrayToHex,
} from './common';

import { types } from './ApiStruct.ts';

import { NotificationContext } from './Notification';
import { DataGrid, GridSortModel } from '@mui/x-data-grid';
import HexEditor from 'react-hex-editor';
import oneDarkPro from 'react-hex-editor/themes/oneDarkPro';
import AddIcon from '@mui/icons-material/Add';
import { secondConfirm } from './Confirm.tsx';

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
  handleClose: () => void;
}) {
  const { lpushOP, rpushOP, handleClose } = props;
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
        showNotification('success', 'Push OK', '');
      })
      .catch((e) => {
        setLoading(false);
        showNotification('error', 'Push OK', e);
      });
  };

  return (
    <Stack spacing={2}>
      <CloseSpace handleClose={handleClose} />

      {loading && (
        <Box sx={{ display: 'flex' }}>
          <CircularProgress />
        </Box>
      )}
      <Box width='200px'>
        <FormControl fullWidth>
          <InputLabel id='simple-select-label'>Direction</InputLabel>
          <Select
            labelId='simple-select-label'
            id='simple-select'
            value={lOrR}
            onChange={(e) => {
              setLOrR(e.target.value);
            }}
            label=''
          >
            <MenuItem value='lpush'>List head (LPUSH)</MenuItem>
            <MenuItem value='rpush'>List tail (RPUSH)</MenuItem>
          </Select>
        </FormControl>
      </Box>
      <RedisEditHexOrText handleValue={(e) => setKey(e)}></RedisEditHexOrText>

      <Stack direction='row' spacing={1}>
        <Button
          variant='outlined'
          onClick={() => {
            handleClose();
          }}
        >
          Cancel
        </Button>
        <Button color='primary' variant='contained' onClick={handleAdd}>
          PUSH
        </Button>
      </Stack>
    </Stack>
  );

  return <></>;
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
        oldValue={content}
        handleValue={(e) => {
          setNewConent(e);
        }}
        textRows={10}
        hexColumns={0x16}
        hexRows={0x10}
      ></RedisEditHexOrText>

      <Stack width='100px' direction='row' spacing={1}>
        <Button
          color='primary'
          variant='contained'
          onClick={() => {
            handleModify();
          }}
        >
          Set
        </Button>
        {loading && (
          <Box sx={{ display: 'flex' }}>
            <CircularProgress />
          </Box>
        )}
      </Stack>
    </Stack>
  );
}

function RedisAddSet(props: {
  addOP: (key: string) => Promise<void>;
  handleClose: () => void;
}) {
  var { addOP, handleClose } = props;
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
      {loading && (
        <Box sx={{ display: 'flex' }}>
          <CircularProgress />
        </Box>
      )}
      <CloseSpace handleClose={handleClose}></CloseSpace>
      <RedisEditHexOrText
        handleValue={(e) => {
          setKey(e);
        }}
      ></RedisEditHexOrText>

      <Stack direction='row' spacing={1}>
        <Button
          variant='outlined'
          onClick={() => {
            handleClose();
          }}
        >
          Cancel
        </Button>
        <Button color='primary' variant='contained' onClick={handleAdd}>
          Add
        </Button>
      </Stack>
    </Stack>
  );
}

function RedisAddHash(props: {
  addOP: (key: string, value: string) => Promise<void>;
  handleClose: () => void;
}) {
  var { addOP, handleClose } = props;
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
      {loading && (
        <Box sx={{ display: 'flex' }}>
          <CircularProgress />
        </Box>
      )}
      <Typography variant='h5'> key </Typography>
      <RedisEditHexOrText
        oldValue={key}
        handleValue={(e) => {
          setKey(e);
        }}
      ></RedisEditHexOrText>
      <Typography variant='h5'> value </Typography>
      <RedisEditHexOrText
        oldValue={value}
        handleValue={(e) => {
          setValue(e);
        }}
      ></RedisEditHexOrText>

      <Stack direction='row' spacing={1}>
        <Button
          variant='outlined'
          onClick={() => {
            handleClose();
          }}
        >
          Cancel
        </Button>
        <Button color='primary' variant='contained' onClick={handleAdd}>
          Add
        </Button>
      </Stack>
    </Stack>
  );
}
function RedisAddZset(props: {
  addOP: (key: string, value: string) => Promise<void>;
  handleClose: () => void;
}) {
  var { addOP, handleClose } = props;
  const [loading, setLoading] = useState(false);
  const [score, setScore] = useState('');
  const [key, setKey] = useState('');
  const [scoreErr, setScoreErr] = useState(false);

  const { showNotification } = useContext(NotificationContext);

  const handleAdd = () => {
    // reset some state
    setLoading(true);

    addOP(key, score)
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
      {loading && (
        <Box sx={{ display: 'flex' }}>
          <CircularProgress />
        </Box>
      )}
      <TextField
        value={score}
        label='score'
        error={scoreErr}
        helperText='put a valid float64 value'
        onChange={(event) => {
          setScore(event.target.value);
          setScoreErr(isNaN(parseFloat(event.target.value)));
        }}
      ></TextField>

      <RedisEditHexOrText
        handleValue={(e) => {
          setKey(e);
        }}
      ></RedisEditHexOrText>

      <Stack direction='row' spacing={1}>
        <Button
          variant='outlined'
          onClick={() => {
            handleClose();
          }}
        >
          Cancel
        </Button>
        <Button color='primary' variant='contained' onClick={handleAdd}>
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
  handleClose: () => void;
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
      })
      .catch((e) => {
        showNotification('success', 'Modify failure', e);
        setLoading(false);
      });
  };

  return (
    <Stack spacing={1}>
      {loading && (
        <Box sx={{ display: 'flex' }}>
          <CircularProgress />
        </Box>
      )}
      <Typography variant='h5'>key</Typography>
      <RedisEditHexOrText
        oldValue={newKey}
        handleValue={(e) => {
          setNewKey(e);
        }}
      ></RedisEditHexOrText>
      <Typography variant='h5'>value</Typography>
      <RedisEditHexOrText
        oldValue={newValue}
        handleValue={(e) => {
          setNewValue(e);
        }}
      ></RedisEditHexOrText>

      <Stack direction='row' spacing={1}>
        <Button
          variant='outlined'
          onClick={() => {
            handleClose();
          }}
        >
          Cancel
        </Button>
        <Button color='primary' variant='contained' onClick={handleModify}>
          Modify
        </Button>
      </Stack>
    </Stack>
  );
}

function RedisEditZset(props: {
  modifyOP: (req: RedisZsetHashModifyReq) => Promise<void>;
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

    modifyOP(new RedisZsetHashModifyReq(oldKey, newKey, newValue))
      .then(() => {
        setLoading(false);
        showNotification('success', 'Modify OK');
      })
      .catch((e) => {
        setLoading(false);
        showNotification('error', 'Modify Failure', e);
      });
  };

  return (
    <Stack spacing={2}>
      {loading && (
        <Box sx={{ display: 'flex' }}>
          <CircularProgress />
        </Box>
      )}
      <TextField
        value={newValue}
        label='score'
        error={newValueErr}
        helperText='put a valid float64 value'
        onChange={(event) => {
          setNewValue(event.target.value);
          if (isNaN(parseFloat(event.target.value))) {
            setNewValueErr(true);
          } else {
            setNewValueErr(false);
          }
        }}
      ></TextField>
      <RedisEditHexOrText
        oldValue={oldKey}
        handleValue={(e) => {
          setNewKey(e);
        }}
      ></RedisEditHexOrText>

      <Stack direction='row' spacing={1}>
        <Button
          variant='outlined'
          onClick={() => {
            handleClose();
          }}
        >
          Cancel
        </Button>
        <Button color='primary' variant='contained' onClick={handleModify}>
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
        <InputLabel id='simple-select-label'>viewer</InputLabel>
        <Select
          labelId='simple-select-label'
          id='simple-select'
          value={mode}
          onChange={(e) => {
            handleChange(e.target.value as RedisEditMode);
          }}
          label='viewer'
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
  hexColumns?: number;
  hexRows?: number;
  textRows?: number;
}) {
  let { oldValue, handleValue, hexColumns, hexRows, textRows } = props;
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
      // @ts-ignore
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

  if (!hexColumns) {
    hexColumns = 0x11;
  }
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
          label='key'
          multiline
          rows={textRows}
          onChange={(event) => {
            setNewValue(event.target.value);
            handleValue(stringToHex(event.target.value));
          }}
        ></TextField>
      )}

      {editModel == RedisEditMode.Hex && (
        <Stack spacing={1}>
          <HexEditor
            columns={hexColumns}
            rows={hexRows}
            data={newValue}
            nonce={nonce}
            onSetValue={handleSetValue}
            theme={{ hexEditor: oneDarkPro }}
            showAscii={true}
            showRowLabels={true}
            showColumnLabels={true}
          ></HexEditor>
          <Stack direction='row' spacing={1} alignItems='flex-start'>
            <TextField
              error={addLengthErr}
              value={addLength}
              helperText='length add the hex'
              onChange={(e) => {
                setAddLength(e.target.value);
              }}
              size='small'
            ></TextField>

            <Button
              color='primary'
              variant='contained'
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
      })
      .catch((e) => {
        setLoading(false);
        showNotification('success', 'Modify Failure', e);
      });
  };

  return (
    <Stack spacing={1}>
      {loading && (
        <Box sx={{ display: 'flex' }}>
          <CircularProgress />
        </Box>
      )}
      <RedisEditHexOrText
        oldValue={oldValue}
        handleValue={(e) => {
          setNewValue(e);
        }}
      ></RedisEditHexOrText>
      <Stack direction='row' spacing={1}>
        <Button
          variant='outlined'
          onClick={() => {
            handleClose();
          }}
        >
          Cancel
        </Button>
        <Button
          color='primary'
          variant='contained'
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
      })
      .catch((e) => {
        setLoading(false);
        showNotification('success', 'Modify Failure', e);
      });
  };

  return (
    <Stack spacing={1}>
      {loading && (
        <Box sx={{ display: 'flex' }}>
          <CircularProgress />
        </Box>
      )}
      <RedisEditHexOrText
        oldValue={oldValue}
        handleValue={(e) => {
          setNewValue(e);
        }}
      ></RedisEditHexOrText>
      <Stack direction='row' spacing={1}>
        <Button
          variant='outlined'
          onClick={() => {
            handleClose();
          }}
        >
          Cancel
        </Button>
        <Button
          color='primary'
          variant='contained'
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
  const [selectID, setSelectID] = useState('');
  const [showAdd, setShowAdd] = useState(false);
  const [selectionModel, setSelectionModel] = useState([]);
  const handleReload = () => {
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
      })
      .catch((e) => {
        showNotification('error', 'load data error', e);
      });
  };

  useEffect(() => {
    handleReload();
  }, []);

  const getIDIndex = (selectID: string): number => {
    let ret;
    resp.data.data.forEach((e, index) => {
      if (e.id === selectID) {
        ret = index;
      }
    });
    return ret;
  };

  const handleDel = async () => {
    for (let i = 0; i < selectionModel.length; i++) {
      let id = selectionModel[i];
      let index = getIDIndex(id);
      let value = resp.data.data[index]['value'];
      try {
        await delOP(value);
        showNotification('success', 'Delete OK');
      } catch (e) {
        showNotification('error', 'delete failure', e);
      }
    }
    handleReload();
  };
  return (
    <Stack spacing={1}>
      <Stack direction='row' spacing={1} alignItems='flex-start'>
        <TextField
          label='key match could be *key,key*,*key*'
          variant='outlined' // 可以是 'outlined', 'filled', or 'standard'
          value={keyMatch}
          onChange={(event) => {
            setKeyMatch(event.target.value);
          }}
          size='small'
        />
        <Button
          variant='contained' // 可以是 'text', 'outlined', 或 'contained'
          color='primary' // 主题颜色
          onClick={handleReload} // 点击事件处理
        >
          Reload
        </Button>
      </Stack>

      {resp && (
        <Stack spacing={1}>
          <Stack direction='row' spacing={1}>
            <Button
              variant='contained'
              color='error'
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
              <DeleteForeverIcon></DeleteForeverIcon>
            </Button>
          </Stack>

          {/* actual data grid */}
          <DataGrid
            rows={resp.data.data}
            checkboxSelection
            editMode='row'
            onRowDoubleClick={(e) => {
              setSelectID(e.id as string);
            }}
            columns={resp.data.fields}
            disableColumnFilter
            onRowSelectionModelChange={(rows) => {
              setSelectionModel(rows as string[]);
            }}
          ></DataGrid>
          {selectID && (
            <Modal
              open={!!selectID}
              onClose={() => {
                setSelectID(null);
              }}
              aria-labelledby='modify set'
            >
              <Box sx={modalStyle} width='600px'>
                <RedisEditSet
                  modifyOP={async (oldValue, newValue) => {
                    try {
                      await modifyOP(oldValue, newValue);
                    } catch (e) {
                      throw e;
                    }
                    resp.data.data[getIDIndex(selectID)].value = newValue;
                  }}
                  key={selectID}
                  oldValue={resp.data.data[getIDIndex(selectID)].value}
                  handleClose={() => {
                    setSelectID(null);
                  }}
                ></RedisEditSet>
              </Box>
            </Modal>
          )}
        </Stack>
      )}
      <Stack direction='row' spacing={1}>
        <Button
          variant='contained'
          color='primary'
          onClick={() => {
            setShowAdd(true);
          }}
        >
          <AddIcon></AddIcon>
        </Button>
      </Stack>

      <Modal
        open={showAdd}
        onClose={() => {
          setSelectID(null);
        }}
        aria-labelledby='modify set'
      >
        <Box sx={modalStyle} width='600px'>
          <RedisAddSet
            addOP={async (value) => {
              try {
                await addOP(value);
              } catch (e) {
                throw e;
              }
              handleReload();
            }}
            handleClose={() => {
              setShowAdd(false);
            }}
          ></RedisAddSet>
        </Box>
      </Modal>

      {resp && (
        <Typography variant='h3'>
          Result {resp.data.data.length} of {resp.total}
        </Typography>
      )}
    </Stack>
  );
}

function RedisListTable(props: {
  reloadOP: (
    page: number,
    pageSize: number,
    asc: boolean
  ) => Promise<types.MuiDataTableListResp>;
  modifyOP: (index: number, newValue: string) => Promise<void>;
  lpushOP: (value: string) => Promise<void>;
  rpushOP: (value: string) => Promise<void>;
  delOP: (index: number) => Promise<void>;
}) {
  const { reloadOP, modifyOP, lpushOP, delOP, rpushOP } = props;
  const [resp, setResp] = useState<types.MuiDataTableListResp | null>(null);
  const [selectID, setSelectID] = useState('');
  const [showAdd, setShowAdd] = useState(false);
  const [selectionModel, setSelectionModel] = useState('');
  const [sortModel, setSortModel] = useState<GridSortModel>([]);
  const [page, setPage] = useState(0);
  const [pageSize, setPageSize] = useState(10);
  const { showNotification } = useContext(NotificationContext);

  const getSortOrder = () => {
    if (sortModel.length == 0) {
      return true;
    }
    if (sortModel[0].sort == 'desc') {
      return false;
    }
    return true;
  };

  const handleReload = () => {
    reloadOP(page + 1, pageSize, getSortOrder())
      .then((resp) => {
        resp.data.data.forEach((e) => {
          e.id = uuidv4();
        });
        resp.data.fields[1]['sortable'] = false;
        resp.data.fields[1]['flex'] = 1;
        resp.data.fields[1]['renderCell'] = (p) => {
          return redisRenderCell(p.row.value);
        };
        setResp(resp);
      })
      .catch((e) => {
        showNotification('error', 'load data error', e);
      });
  };

  useEffect(() => {
    handleReload();
  }, [page, pageSize, sortModel]);

  const getIDIndex = (selectID: string): number => {
    let ret;
    resp.data.data.forEach((e, index) => {
      if (e.id === selectID) {
        ret = index;
      }
    });
    return ret;
  };

  const handleDel = async () => {
    let index = getIDIndex(selectionModel);
    let value = parseInt(resp.data.data[index]['seq']);
    try {
      await delOP(value);
      showNotification('success', 'Delete OK');
    } catch (e) {
      showNotification('error', 'Delete failure', e);
    }
    handleReload();
  };
  return (
    <Stack spacing={1}>
      <Stack direction='row' spacing={1}>
        <Button
          variant='contained' // 可以是 'text', 'outlined', 或 'contained'
          color='primary' // 主题颜色
          onClick={handleReload} // 点击事件处理
        >
          Reload
        </Button>
      </Stack>
      {resp && (
        <Stack spacing={1}>
          {/* delete Button  */}
          <Stack direction='row' spacing={1}>
            <Button
              variant='contained'
              color='error'
              disabled={selectionModel === ''}
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
              <DeleteForeverIcon></DeleteForeverIcon>
            </Button>
          </Stack>

          {/* actual data grid */}
          <DataGrid
            rows={resp.data.data}
            checkboxSelection
            editMode='row'
            onRowDoubleClick={(e) => {
              setSelectID(e.id as string);
            }}
            columns={resp.data.fields}
            disableColumnFilter
            disableMultipleRowSelection
            onRowSelectionModelChange={(rows) => {
              setSelectionModel(rows.length > 0 ? (rows[0] as string) : '');
            }}
            sortModel={sortModel}
            onSortModelChange={(e) => {
              setSortModel(e);
            }}
            autoPageSize={false}
            pageSizeOptions={[10]}
            paginationMeta={{ hasNextPage: true }}
            paginationMode='server'
            paginationModel={{ page: page, pageSize: pageSize }}
            rowCount={resp.total}
            onPaginationModelChange={(model) => {
              setPage(model.page);
              setPageSize(model.pageSize);
            }}
            sortingMode='server'
          ></DataGrid>
          {selectID && (
            <Modal
              open={!!selectID}
              onClose={() => {
                setSelectID(null);
              }}
              aria-labelledby='modify set'
            >
              <Box sx={modalStyle} width='600px'>
                <RedisEditList
                  modifyOP={async (index, newValue) => {
                    try {
                      await modifyOP(index, newValue);
                    } catch (e) {
                      throw e;
                    }
                    resp.data.data[getIDIndex(selectID)].value = newValue;
                  }}
                  key={selectID}
                  oldValue={resp.data.data[getIDIndex(selectID)].value}
                  index={getIDIndex(selectID)}
                  handleClose={() => {
                    setSelectID(null);
                  }}
                ></RedisEditList>
              </Box>
            </Modal>
          )}
        </Stack>
      )}
      <Stack direction='row' spacing={1}>
        <Button
          variant='contained'
          color='primary'
          onClick={() => {
            setShowAdd(true);
          }}
        >
          <AddIcon></AddIcon>
        </Button>
      </Stack>
      {/* // add modal */}
      <Modal
        open={showAdd}
        onClose={() => {
          setSelectID(null);
        }}
        aria-labelledby='modify set'
      >
        <Box sx={modalStyle} width='600px'>
          <RedisListAdd
            lpushOP={async (value) => {
              try {
                await lpushOP(value);
              } catch (e) {
                throw e;
              }
              handleReload();
            }}
            rpushOP={async (value) => {
              try {
                await rpushOP(value);
              } catch (e) {
                throw e;
              }
              handleReload();
            }}
            handleClose={() => {
              setShowAdd(false);
            }}
          ></RedisListAdd>
        </Box>
      </Modal>
      {resp && (
        <Typography variant='h3'>
          Result {resp.data.data.length} of {resp.total}
        </Typography>
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

  modifyOP: (oldkey: string, newKey: string, newValue: string) => Promise<void>;
  addOP: (key: string, value: string) => Promise<void>;
  delOP: (key: string) => Promise<void>;
}) {
  const { reloadOP, modifyOP, addOP, delOP } = props;
  const [keyMatch, setKeyMatch] = useState('');
  const [resp, setResp] = useState<types.MuiDataTableListResp | null>(null);
  const [selectID, setSelectID] = useState('');
  const [showAdd, setShowAdd] = useState(false);
  const [min, setMin] = useState('');
  const [max, setMax] = useState('');
  const [minErr, setMinErr] = useState(false);
  const [maxErr, setMaxErr] = useState(false);

  const [selectionModel, setSelectionModel] = useState([]);
  const { showNotification } = useContext(NotificationContext);
  const handleReload = () => {
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
        setResp(resp);
      })
      .catch((e) => {
        showNotification('error', 'load data error', e);
      });
  };

  useEffect(() => {
    handleReload();
  }, []);

  const getIDIndex = (selectID: string): number => {
    let ret;
    resp.data.data.forEach((e, index) => {
      if (e.id === selectID) {
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
    handleReload();
  };

  return (
    <Stack spacing={1}>
      <Stack direction='row' spacing={1} alignItems='flex-start'>
        <TextField
          label='key match could be *key,key*,*key*'
          variant='outlined'
          value={keyMatch}
          onChange={(event) => {
            setKeyMatch(event.target.value);
          }}
          size='small'
        />
        <TextField
          label='minimum value'
          variant='outlined'
          value={min}
          error={minErr}
          helperText='put a valid float64'
          onChange={(event) => {
            setMinErr(isNaN(parseFloat(event.target.value)));
            setMin(event.target.value);
          }}
          size='small'
        />

        <TextField
          label='maxiumn value'
          variant='outlined'
          value={max}
          error={maxErr}
          helperText='put a valid float64'
          onChange={(event) => {
            setMaxErr(isNaN(parseFloat(event.target.value)));
            setMax(event.target.value);
          }}
          size='small'
        />
        <Button variant='contained' color='primary' onClick={handleReload}>
          Reload
        </Button>
      </Stack>

      {resp && (
        <Stack spacing={1}>
          <Stack direction='row' spacing={1}>
            <Button
              variant='contained'
              color='error'
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
              <DeleteForeverIcon></DeleteForeverIcon>
            </Button>
          </Stack>

          {/* actual data grid */}
          <DataGrid
            rows={resp.data.data}
            checkboxSelection
            editMode='row'
            onRowDoubleClick={(e) => {
              setSelectID(e.id as string);
            }}
            columns={resp.data.fields}
            disableColumnFilter
            onRowSelectionModelChange={(rows) => {
              setSelectionModel(rows as string[]);
            }}
          ></DataGrid>
          {selectID && (
            <Modal
              open={!!selectID}
              onClose={() => {
                setSelectID(null);
              }}
              aria-labelledby='modify set'
            >
              <Box sx={modalStyle} width='600px'>
                <RedisEditZset
                  modifyOP={async (req) => {
                    try {
                      await modifyOP(req.oldKey, req.newKey, req.value);
                    } catch (e) {
                      throw e;
                    }
                    let index = getIDIndex(selectID);
                    resp.data.data[index].key = req.newKey;
                    resp.data.data[index].score = req.value;
                  }}
                  key={selectID}
                  oldValue={resp.data.data[getIDIndex(selectID)].score}
                  oldKey={resp.data.data[getIDIndex(selectID)].key}
                  handleClose={() => {
                    setSelectID(null);
                  }}
                ></RedisEditZset>
              </Box>
            </Modal>
          )}
        </Stack>
      )}
      <Stack direction='row' spacing={1}>
        <Button
          variant='contained'
          color='primary'
          onClick={() => {
            setShowAdd(true);
          }}
        >
          <AddIcon />
        </Button>
      </Stack>
      {/* // add modal */}
      <Modal
        open={showAdd}
        onClose={() => {
          setSelectID(null);
        }}
        aria-labelledby='modify set'
      >
        <Box sx={modalStyle} width='600px'>
          <RedisAddZset
            addOP={async (key, value) => {
              try {
                await addOP(key, value);
              } catch (e) {
                throw e;
              }
              handleReload();
            }}
            handleClose={() => {
              setShowAdd(false);
            }}
          ></RedisAddZset>
        </Box>
      </Modal>

      {resp && (
        <Typography variant='h3'>
          Result {resp.data.data.length} of {resp.total}
        </Typography>
      )}
    </Stack>
  );
}

function RedisHashTable(props: {
  reloadOP: (keyMatch: string) => Promise<types.MuiDataTableListResp>;
  modifyOP: (oldkey: string, newKey: string, newValue: string) => Promise<void>;
  addOP: (key: string, value: string) => Promise<void>;
  delOP: (key: string) => Promise<void>;
}) {
  const { reloadOP, modifyOP, addOP, delOP } = props;
  const [keyMatch, setKeyMatch] = useState('');
  const [resp, setResp] = useState<types.MuiDataTableListResp | null>(null);
  const { showNotification } = useContext(NotificationContext);
  const [selectID, setSelectID] = useState('');
  const [showAdd, setShowAdd] = useState(false);
  const [selectionModel, setSelectionModel] = useState([]);
  const handleReload = () => {
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
      })
      .catch((e) => {
        showNotification('error', 'load data error', e);
      });
  };

  useEffect(() => {
    handleReload();
  }, []);

  const getIDIndex = (selectID: string): number => {
    let ret;
    resp.data.data.forEach((e, index) => {
      if (e.id === selectID) {
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
    handleReload();
  };

  return (
    <Stack spacing={1}>
      <Stack direction='row' spacing={1} alignItems='flex-start'>
        <TextField
          label='key match could be *key,key*,*key*'
          variant='outlined' // 可以是 'outlined', 'filled', or 'standard'
          value={keyMatch}
          onChange={(event) => {
            setKeyMatch(event.target.value);
          }}
          size='small'
        />
        <Button
          variant='contained' // 可以是 'text', 'outlined', 或 'contained'
          color='primary' // 主题颜色
          onClick={handleReload} // 点击事件处理
        >
          Reload
        </Button>
      </Stack>

      {resp && (
        <Stack spacing={1}>
          <Stack direction='row'>
            <Button
              variant='contained'
              color='error'
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
              <DeleteForeverIcon></DeleteForeverIcon>
            </Button>
          </Stack>

          {/* actual data grid */}
          <DataGrid
            rows={resp.data.data}
            checkboxSelection
            editMode='row'
            onRowDoubleClick={(e) => {
              setSelectID(e.id as string);
            }}
            columns={resp.data.fields}
            disableColumnFilter
            onRowSelectionModelChange={(rows) => {
              setSelectionModel(rows as string[]);
            }}
          ></DataGrid>
          {selectID && (
            <Modal
              open={!!selectID}
              onClose={() => {
                setSelectID(null);
              }}
              aria-labelledby='modify set'
            >
              <Box sx={modalStyle} width='600px'>
                <RedisEditHash
                  modifyOP={async (req) => {
                    try {
                      await modifyOP(req.oldKey, req.newKey, req.value);
                    } catch (e) {
                      throw e;
                    }
                    let index = getIDIndex(selectID);
                    resp.data.data[index].key = req.newKey;
                    resp.data.data[index].value = req.value;
                  }}
                  key={selectID}
                  oldValue={resp.data.data[getIDIndex(selectID)].value}
                  oldKey={resp.data.data[getIDIndex(selectID)].key}
                  handleClose={() => {
                    setSelectID(null);
                  }}
                ></RedisEditHash>
              </Box>
            </Modal>
          )}
        </Stack>
      )}
      <Stack direction='row' spacing={1}>
        <Button
          variant='contained'
          color='primary'
          onClick={() => {
            setShowAdd(true);
          }}
        >
          <AddIcon></AddIcon>
        </Button>
      </Stack>
      <Modal
        open={showAdd}
        onClose={() => {
          setSelectID(null);
        }}
        aria-labelledby='modify set'
      >
        <Box sx={modalStyle} width='600px'>
          <RedisAddHash
            addOP={async (key, value) => {
              try {
                await addOP(key, value);
              } catch (e) {
                throw e;
              }
              handleReload();
            }}
            handleClose={() => {
              setShowAdd(false);
            }}
          ></RedisAddHash>
        </Box>
      </Modal>
      {resp && (
        <Typography variant='h3'>
          Result {resp.data.data.length} of {resp.total}
        </Typography>
      )}
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
};
