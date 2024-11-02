import React, { useState } from 'react';
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
} from '@mui/material';

import ReactDOM from 'react-dom/client';

import { CloseSpace, errOkHoldTime } from './common';

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

function RedisListAdd(props: {
  lpush: (value: string) => Promise<void>;
  rpush: (value: string) => Promise<void>;
  handleClose: () => void;
}) {
  const { lpush, rpush, handleClose } = props;
  const [loading, setLoading] = useState(false);
  const [err, setErr] = useState<string>('');
  const [okMsg, setOkMsg] = useState('');
  const [key, setKey] = useState('');
  const [lOrR, setLOrR] = useState('lpush');

  const handleAdd = () => {
    // reset some state
    setLoading(true);
    setErr('');
    setOkMsg('');

    (lOrR == 'lpush' ? lpush : rpush)(key)
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
      <CloseSpace handleClose={handleClose} />
      {err && <Alert severity='error'>{err}</Alert>}
      {okMsg && <Alert>{okMsg}</Alert>}
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
      <TextField
        value={key}
        label='key'
        multiline
        rows={4}
        required
        onChange={(event) => {
          setKey(event.target.value);
        }}
      ></TextField>

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

function RedisDelKey(props: {
  delOP: (key: string) => Promise<void>;
  rediskey: string;
  handleClose: () => void;
}) {
  const { rediskey, delOP, handleClose } = props;
  const [loading, setLoading] = useState(false);
  const [err, setErr] = useState<string>('');
  const [okMsg, setOkMsg] = useState('');
  const [disableDel, setDisableDel] = useState(false);
  const handleDel = () => {
    setLoading(true);
    setErr('');
    setOkMsg('');
    delOP(rediskey)
      .then(() => {
        setLoading(false);
        setOkMsg('Delete ok,Please close this.');
        setDisableDel(true);
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
      <CloseSpace handleClose={handleClose} />

      {err && <Alert severity='error'>{err}</Alert>}
      {okMsg && <Alert>{okMsg}</Alert>}
      {loading && (
        <Box sx={{ display: 'flex' }}>
          <CircularProgress />
        </Box>
      )}

      {!okMsg && (
        <Alert severity='error'>Are you sure to Delete key '{rediskey}'.</Alert>
      )}
      <Stack direction='row'>
        <Button
          variant='contained'
          color='error'
          onClick={handleDel}
          disabled={disableDel}
        >
          Delete
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

function RedisAddSet(props: {
  addOP: (key: string) => Promise<void>;
  handleClose: () => void;
}) {
  var { addOP, handleClose } = props;
  const [loading, setLoading] = useState(false);
  const [err, setErr] = useState<string>('');
  const [okMsg, setOkMsg] = useState('');
  const [key, setKey] = useState('');
  const handleAdd = () => {
    // reset some state
    setLoading(true);
    setErr('');
    setOkMsg('');
    console.log('Add score', key);
    addOP(key)
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

function RedisAddHash(props: {
  addOP: (key: string, value: string) => Promise<void>;
  handleClose: () => void;
}) {
  var { addOP, handleClose } = props;
  const [loading, setLoading] = useState(false);
  const [err, setErr] = useState<string>('');
  const [okMsg, setOkMsg] = useState('');
  const [value, setValue] = useState('');
  const [key, setKey] = useState('');
  const handleAdd = () => {
    // reset some state
    setLoading(true);
    setErr('');
    setOkMsg('');
    console.log('Add score', key, value);
    addOP(key, value)
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

function RedisEditSet(props: {
  modifyOP: (oldkey: string, newkey: string) => Promise<void>;
  oldKey: string;
  handleClose: () => void;
}) {
  let { oldKey, modifyOP, handleClose } = props;
  const [loading, setLoading] = useState(false);
  const [err, setErr] = useState<string>('');
  const [okMsg, setOkMsg] = useState('');
  const [newKey, setNewKey] = useState(oldKey);

  // really modify the content.
  const handleModify = () => {
    if (oldKey == newKey) {
      return;
    }
    // reset some state
    setLoading(true);
    setErr('');
    setOkMsg('');
    modifyOP(oldKey, newKey)
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

// const root = ReactDOM.createRoot(document.getElementById('root'));

// root.render(<React.StrictMode></React.StrictMode>);

export {};
