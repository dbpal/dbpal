import React, { Profiler, useContext, useEffect, useState } from 'react';
import {
  Button,
  Stack,
  Alert,
  Box,
  CircularProgress,
  TextField,
  MenuItem,
  FormControl,
  InputLabel,
  Select,
  AlertColor,
  Typography,
  Modal,
} from '@mui/material';
import { v4 as uuidv4 } from 'uuid';
import ReactDOM from 'react-dom/client';

import {
  CloseSpace,
  errOkHoldTime,
  hexToUint8Array,
  hexToUtf8,
  hexTryToUtf8,
  modalStyle,
  stringToHex,
  arrayToHex,
  SecondConfirm,
} from './common';
import { testExampleRedisList, testExampleRedisSet } from './exampleTest';
import { types } from './ApiStruct.ts';
import { voidTestFunction } from './helper';
import { NotificationContext } from './Notification';
import { DataGrid } from '@mui/x-data-grid';
import HexEditor from 'react-hex-editor';
import oneDarkPro from 'react-hex-editor/themes/oneDarkPro';
import AddIcon from '@mui/icons-material/Add';
class RedisZsetHashModifyReq {
  oldKey: string;
  newKey: string;
  value: string;
  constructor(oldKey: string, newKey: string, value: string) {
    this.oldKey = oldKey;
    this.newKey = newKey;
    this.value = value;
  }
}

enum RedisEditMode {
  Hex,
  Text,
}

function RedisListAdd(props: {
  lpush: (value: string) => Promise<void>;
  rpush: (value: string) => Promise<void>;
  handleClose: () => void;
}) {
  const { lpush, rpush, handleClose } = props;
  const [loading, setLoading] = useState(false);

  const [lOrR, setLOrR] = useState('lpush');
  const [key, setKey] = useState('');
  const { showNotification } = useContext(NotificationContext);
  const handleAdd = () => {
    // reset some state
    setLoading(true);
    (lOrR == 'lpush' ? lpush : rpush)(key)
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
          <InputLabel id='simple-select-label'>LPUSH OR RPUSH</InputLabel>
          <Select
            labelId='simple-select-label'
            id='simple-select'
            value={lOrR}
            onChange={(e) => {
              setLOrR(e.target.value);
            }}
            label=''
          >
            <MenuItem value='lpush'>LPUSH</MenuItem>
            <MenuItem value='rpush'>RPUSH</MenuItem>
          </Select>
        </FormControl>
      </Box>
      <RedisEditHexOrText handleValue={(e) => setKey(e)}></RedisEditHexOrText>

      <Stack direction='row'>
        <Button color='primary' variant='contained' onClick={handleAdd}>
          PUSH
        </Button>
        &nbsp;
        <Button
          variant='outlined'
          onClick={() => {
            handleClose();
          }}
        >
          Cancel
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

  const [err, setErr] = useState<string>('');
  const [loading, setLoading] = useState(false);
  const [okMsg, setOkMsg] = useState('');
  const handleModify = () => {
    setErr('');
    setOkMsg('');
    setLoading(true);
    modifyOP(newContent)
      .then(() => {
        setLoading(false);
        setOkMsg('Modify OK');
        setTimeout(() => {
          setOkMsg('');
        }, errOkHoldTime);
      })
      .catch((e) => {
        setLoading(false);
        setErr(e.toString());
        setTimeout(() => {
          setErr('');
        }, errOkHoldTime);
      });
  };

  return (
    <Stack>
      <TextField
        id='outlined-basic'
        label='value'
        variant='outlined'
        value={newContent}
        multiline
        rows={13}
        onChange={(e) => {
          setNewConent(e.target.value);
        }}
      ></TextField>
      {err && <Alert severity='error'>{err}</Alert>}
      {okMsg && <Alert>{okMsg}</Alert>}

      <Stack width='100px' direction='row'>
        <Button
          color='primary'
          variant='contained'
          onClick={() => {
            handleModify();
          }}
        >
          Modify
        </Button>
        &nbsp;
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
    console.log('Add score', key);
    addOP(key)
      .then(() => {
        showNotification('success', 'Add ok', '');
        setLoading(false);
      })
      .catch((e) => {
        showNotification('error', 'Add error', e);
        setLoading(false);
      });
  };

  return (
    <Stack spacing={2}>
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

      <Stack direction='row'>
        <Button color='primary' variant='contained' onClick={handleAdd}>
          Add
        </Button>
        &nbsp;
        <Button
          variant='outlined'
          onClick={() => {
            handleClose();
          }}
        >
          Cancel
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

    console.log('Add score', key, value);
    addOP(key, value)
      .then(() => {
        setLoading(false);
        showNotification('success', 'Add OK', '');
      })
      .catch((e) => {
        setLoading(false);
        showNotification('success', 'Add Error', e);
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
        value={key}
        label='key'
        multiline
        rows={4}
        onChange={(event) => {
          setKey(event.target.value);
        }}
      ></TextField>

      <TextField
        value={value}
        label='value'
        multiline
        rows={4}
        onChange={(event) => {
          setValue(event.target.value);
        }}
      ></TextField>

      <Stack direction='row'>
        <Button color='primary' variant='contained' onClick={handleAdd}>
          Add
        </Button>
        &nbsp;
        <Button
          variant='outlined'
          onClick={() => {
            handleClose();
          }}
        >
          Cancel
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
  const [err, setErr] = useState<string>('');
  const [okMsg, setOkMsg] = useState('');
  const [score, setScore] = useState('');
  const [key, setKey] = useState('');
  const [scoreErr, setScoreErr] = useState(false);
  const handleAdd = () => {
    // reset some state
    setLoading(true);
    setErr('');
    setOkMsg('');
    console.log('Add score', key, score);
    addOP(key, score)
      .then(() => {
        setLoading(false);
        setOkMsg('Add OK');
        setTimeout(() => {
          setOkMsg('');
        }, errOkHoldTime);
      })
      .catch((e) => {
        setLoading(false);
        setErr(e.toString());
        setTimeout(() => {
          setErr('');
        }, errOkHoldTime);
      });
  };

  return (
    <Stack spacing={2}>
      {err && <Alert severity='error'>{err}</Alert>}
      {okMsg && <Alert>{okMsg}</Alert>}
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
          if (isNaN(parseFloat(event.target.value))) {
            setScoreErr(true);
          } else {
            setScoreErr(false);
          }
        }}
      ></TextField>
      <TextField
        value={key}
        label='key'
        multiline
        rows={4}
        onChange={(event) => {
          setKey(event.target.value);
        }}
      ></TextField>

      <Stack direction='row'>
        <Button color='primary' variant='contained' onClick={handleAdd}>
          Add
        </Button>
        &nbsp;
        <Button
          variant='outlined'
          onClick={() => {
            handleClose();
          }}
        >
          Cancel
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
  const [err, setErr] = useState<string>('');
  const [okMsg, setOkMsg] = useState('');
  const [newKey, setNewKey] = useState(oldKey);
  const [newValue, setNewValue] = useState(oldValue);
  const handleModify = () => {
    if (oldKey == newKey && oldValue == newValue) {
      return;
    }
    // reset some state
    setLoading(true);
    setErr('');
    setOkMsg('');

    modifyOP(new RedisZsetHashModifyReq(oldKey, newKey, newValue))
      .then(() => {
        setLoading(false);
        setOkMsg('Modify OK');
        setTimeout(() => {
          setOkMsg('');
        }, errOkHoldTime);
      })
      .catch((e) => {
        setLoading(false);
        setErr(e.toString());
        setTimeout(() => {
          setErr('');
        }, errOkHoldTime);
      });
  };

  return (
    <Stack spacing={2}>
      {err && <Alert severity='error'>{err}</Alert>}
      {okMsg && <Alert>{okMsg}</Alert>}
      {loading && (
        <Box sx={{ display: 'flex' }}>
          <CircularProgress />
        </Box>
      )}

      <TextField
        value={newKey}
        label='key'
        multiline
        rows={4}
        onChange={(event) => {
          setNewKey(event.target.value);
        }}
      ></TextField>

      <TextField
        value={newValue}
        label='value'
        multiline
        rows={4}
        helperText='put a valid float64 value'
        onChange={(event) => {
          setNewValue(event.target.value);
        }}
      ></TextField>

      <Stack direction='row'>
        <Button color='primary' variant='contained' onClick={handleModify}>
          Modify
        </Button>
        &nbsp;
        <Button
          variant='outlined'
          onClick={() => {
            handleClose();
          }}
        >
          Cancel
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
  const [err, setErr] = useState<string>('');
  const [okMsg, setOkMsg] = useState('');
  const [newKey, setNewKey] = useState(oldKey);
  const [newValue, setNewValue] = useState(oldValue);
  const [newValueErr, setNewValueErr] = useState(false);
  const handleModify = () => {
    if (oldKey == newKey && oldValue == newValue) {
      return;
    }
    // reset some state
    setLoading(true);
    setErr('');
    setOkMsg('');

    modifyOP(new RedisZsetHashModifyReq(oldKey, newKey, newValue))
      .then(() => {
        setLoading(false);
        setOkMsg('Modify OK');
        setTimeout(() => {
          setOkMsg('');
        }, errOkHoldTime);
      })
      .catch((e) => {
        setLoading(false);
        setErr(e.toString());
        setTimeout(() => {
          setErr('');
        }, errOkHoldTime);
      });
  };

  return (
    <Stack spacing={2}>
      {err && <Alert severity='error'>{err}</Alert>}
      {okMsg && <Alert>{okMsg}</Alert>}
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
      <TextField
        value={newKey}
        label='key'
        multiline
        rows={4}
        onChange={(event) => {
          setNewKey(event.target.value);
        }}
      ></TextField>

      <Stack direction='row'>
        <Button color='primary' variant='contained' onClick={handleModify}>
          Modify
        </Button>
        &nbsp;
        <Button
          variant='outlined'
          onClick={() => {
            handleClose();
          }}
        >
          Cancel
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
  handleValue: (a: any) => void;
}) {
  let { oldValue, handleValue } = props;
  const [newValue, setNewValue] = useState<String | Uint8Array>(oldValue);
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
    let value = [...newValue];
    for (let i = 0; i < length; i++) {
      value.push(0);
    }
    setNewValue(value);
  };

  useEffect(() => {
    switch (editModel) {
      case RedisEditMode.Text:
        setNewValue(hexToUtf8(oldValue));
        break;
      case RedisEditMode.Hex:
        setNewValue(hexToUint8Array(oldValue));
        break;
    }
  }, [editModel]);

  return (
    <Stack spacing={2}>
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
          rows={4}
          onChange={(event) => {
            setNewValue(event.target.value);
            handleValue(stringToHex(event.target.value));
          }}
        ></TextField>
      )}

      {editModel == RedisEditMode.Hex && (
        <Stack>
          <HexEditor
            columns={0x10}
            rows={0x10}
            data={newValue}
            nonce={nonce}
            onSetValue={handleSetValue}
            theme={{ hexEditor: oneDarkPro }}
            showAscii={true}
            showRowLabels={true}
            showColumnLabels={true}
          ></HexEditor>
          <Stack direction='row'>
            <TextField
              error={addLengthErr}
              value={addLength}
              helperText='length add the hex'
              onChange={(e) => {
                setAddLength(e.target.value);
              }}
            ></TextField>
            <Button
              color='primary'
              variant='contained'
              onClick={handleAddLength}
            >
              +
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
    <Stack spacing={2}>
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
      <Stack direction='row'>
        <Button
          color='primary'
          variant='contained'
          disabled={newValue == ''}
          onClick={handleModify}
        >
          Modify
        </Button>
        &nbsp;
        <Button
          variant='outlined'
          onClick={() => {
            handleClose();
          }}
        >
          Cancel
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
        showNotification('success', 'Modify OK', '');
      })
      .catch((e) => {
        setLoading(false);
        showNotification('success', 'Modify Failure', e);
      });
  };

  return (
    <Stack spacing={2}>
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
      <Stack direction='row'>
        <Button
          color='primary'
          variant='contained'
          disabled={newValue == ''}
          onClick={handleModify}
        >
          Modify
        </Button>
        &nbsp;
        <Button
          variant='outlined'
          onClick={() => {
            handleClose();
          }}
        >
          Cancel
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
  reload: (keyMatch: string) => Promise<types.MuiDataTableListResp>;
  modifyOP: (oldvalue: string, newValue: string) => Promise<void>;
  addOP: (value: string) => Promise<void>;
  delOP: (value: string) => Promise<void>;
}) {
  const { reload, modifyOP, addOP, delOP } = props;
  const [keyMatch, setKeyMatch] = useState('');
  const [resp, setResp] = useState<types.MuiDataTableListResp | null>(null);
  const { showNotification } = useContext(NotificationContext);
  const [selectID, setSelectID] = useState<string | null>(null);
  const [showAdd, setShowAdd] = useState(false);
  const [selectionModel, setSelectionModel] = useState([]);
  const [showDel, setShowDel] = useState(false);
  const handleReload = () => {
    reload(keyMatch)
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
        showNotification('success', 'Delete OK', '');
      } catch (e) {
        showNotification('error', 'delete failure', e);
      }
    }
    handleReload();
  };
  return (
    <Stack>
      <Stack direction='row'>
        <TextField
          label='key match could be *key,key*,*key*'
          variant='outlined' // 可以是 'outlined', 'filled', or 'standard'
          value={keyMatch}
          onChange={(event) => {
            setKeyMatch(event.target.value);
          }}
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
        <Stack>
          <Button
            variant='contained'
            color='error'
            disabled={selectionModel.length == 0}
            onClick={() => {
              setShowDel(true);
            }}
          >
            Delete
          </Button>
          {/* // delete modal  */}
          <Modal
            open={showDel}
            onClose={() => {
              setShowDel(false);
            }}
            aria-labelledby='modify set'
          >
            <Box sx={modalStyle} width='600px'>
              <SecondConfirm
                buttonName='Delete'
                handleClose={() => {
                  setShowDel(false);
                }}
                msg='Are u really going to delete those key??'
                operationOP={async () => {
                  await handleDel();
                  return true;
                }}
              ></SecondConfirm>
            </Box>
          </Modal>
          {/* actual data grid */}
          <DataGrid
            rows={resp.data.data}
            checkboxSelection
            editMode='row'
            onRowDoubleClick={(e) => {
              setSelectID(e.id);
            }}
            columns={resp.data.fields}
            disableColumnFilter
            onRowSelectionModelChange={(rows) => {
              setSelectionModel(rows);
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

      {resp && (
        <Typography variant='h3'>
          Result {resp.data.data.length} of {resp.total}
        </Typography>
      )}
      <Button
        variant='contained'
        color='primary'
        startIcon={<AddIcon />}
        onClick={() => {
          setShowAdd(true);
        }}
      ></Button>
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
    </Stack>
  );
}

function RedisListTable(props: {
  reload: () => Promise<types.MuiDataTableListResp>;
  modifyOP: (index: number, newValue: string) => Promise<void>;
  lpushOP: (value: string) => Promise<void>;
  rpushOP: (value: string) => Promise<void>;
  delOP: (index: number) => Promise<void>;
}) {
  const { reload, modifyOP, lpushOP, delOP, rpushOP } = props;
  const [resp, setResp] = useState<types.MuiDataTableListResp | null>(null);
  const { showNotification } = useContext(NotificationContext);
  const [selectID, setSelectID] = useState<string | null>(null);
  const [showAdd, setShowAdd] = useState(false);
  const [selectionModel, setSelectionModel] = useState('');
  const [showDel, setShowDel] = useState(false);
  const handleReload = () => {
    reload()
      .then((resp) => {
        resp.data.data.forEach((e) => {
          e.id = uuidv4();
        });
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
    let index = getIDIndex(selectionModel);
    let value = parseInt(resp.data.data[index]['seq']);
    try {
      await delOP(value);
      showNotification('success', 'Delete OK', '');
    } catch (e) {
      showNotification('error', 'delete failure', e);
    }
    handleReload();
  };
  return (
    <Stack>
      <Stack direction='row'>
        <Button
          variant='contained' // 可以是 'text', 'outlined', 或 'contained'
          color='primary' // 主题颜色
          onClick={handleReload} // 点击事件处理
        >
          Reload
        </Button>
      </Stack>

      {resp && (
        <Stack>
          {/* delete Button  */}
          <Button
            variant='contained'
            color='error'
            disabled={selectionModel === ''}
            onClick={() => {
              setShowDel(true);
            }}
          >
            Delete
          </Button>
          {/* // delete modal  */}
          <Modal
            open={showDel}
            onClose={() => {
              setShowDel(false);
            }}
            aria-labelledby='modify set'
          >
            <Box sx={modalStyle} width='600px'>
              <SecondConfirm
                buttonName='Delete'
                handleClose={() => {
                  setShowDel(false);
                }}
                msg={`Are u really going to delete key at ${getIDIndex(
                  selectionModel
                )}`}
                operationOP={async () => {
                  await handleDel();
                  return true;
                }}
              ></SecondConfirm>
            </Box>
          </Modal>

          {/* actual data grid */}
          <DataGrid
            rows={resp.data.data}
            checkboxSelection
            editMode='row'
            onRowDoubleClick={(e) => {
              setSelectID(e.id);
            }}
            columns={resp.data.fields}
            disableColumnFilter
            disableMultipleRowSelection
            onRowSelectionModelChange={(rows) => {
              setSelectionModel(rows.length > 0 ? rows[0] : '');
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
                  handleClose={() => {
                    setSelectID(null);
                  }}
                ></RedisEditList>
              </Box>
            </Modal>
          )}
        </Stack>
      )}

      {resp && (
        <Typography variant='h3'>
          Result {resp.data.data.length} of {resp.total}
        </Typography>
      )}
      <Button
        variant='contained'
        color='primary'
        startIcon={<AddIcon />}
        onClick={() => {
          setShowAdd(true);
        }}
      ></Button>
      <Modal
        open={showAdd}
        onClose={() => {
          setSelectID(null);
        }}
        aria-labelledby='modify set'
      >
        <Box sx={modalStyle} width='600px'>
          <RedisListAdd
            lpush={async (value) => {
              try {
                await lpushOP(value);
              } catch (e) {
                throw e;
              }
              handleReload();
            }}
            rpush={async (value) => {
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
    </Stack>
  );
}

const root = ReactDOM.createRoot(document.getElementById('root'));

root.render(
  <NotificationContext.Provider
    value={{
      showNotification: (
        severity: AlertColor,
        title: string,
        message: string
      ) => {
        console.log(`${severity} ${title} ${message}`);
        // alert(`${severity} ${title} ${message}`);
      },
    }}
  >
    <React.StrictMode>
      {/* <RedisEditHexOrText
        oldValue='aaaaaabb'
        handleValue={(e) => {
          console.log(e);
        }}
      ></RedisEditHexOrText>
      <hr></hr> */}

      <RedisListTable
        reload={async (_: string) => {
          try {
            await voidTestFunction();
          } catch (e) {
            throw e;
          }
          return testExampleRedisList(10);
        }}
        modifyOP={voidTestFunction}
        lpushOP={voidTestFunction}
        rpushOP={voidTestFunction}
        delOP={voidTestFunction}
      ></RedisListTable>

      {/* <RedisSetTable
        reload={async (_: string) => {
          try {
            await voidTestFunction();
          } catch (e) {
            throw e;
          }
          return testExampleRedisSet(10);
        }}
        modifyOP={voidTestFunction}
        addOP={voidTestFunction}
        delOP={async () => {}}
      ></RedisSetTable> */}
    </React.StrictMode>
  </NotificationContext.Provider>
);

export {};
