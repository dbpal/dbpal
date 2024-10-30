import React, { useEffect, useState } from 'react';
import {
  Table,
  TableBody,
  TableCell,
  TableContainer,
  TableHead,
  TableRow,
  Button,
  Stack,
  Alert,
  Typography,
  Box,
  CircularProgress,
  TextField,
  Modal,
  IconButton,
} from '@mui/material';

import ReactDOM from 'react-dom/client';
import CloseIcon from '@mui/icons-material/Close';

import apis from './ApiStruct';
import { randomTestThrowFunction, voidTestFunction } from './helper';
import { testExampleRedisZset } from './exampleTest';
import { errOkHoldTime, modalStyle } from './common';
import LongMenu from './LongMenu';

class RedisZsetModifyReq {
  oldKey: string;
  newKey: string;
  value: string;
  constructor(oldKey: string, newKey: string, value: string) {
    this.oldKey = oldKey;
    this.newKey = newKey;
    this.value = value;
  }
}

function RedisZsetTable(props: {
  reloadOP: (keymatch: string | null) => Promise<apis.TableListResp>;
  modifyOP: (req: RedisZsetModifyReq) => Promise<void>;
  delOP: (req: string) => Promise<void>;
  addOP: (key: String, value: number) => Promise<void>;
}) {
  const [contextMenu, setContextMenu] = React.useState(null);

  const { reloadOP, modifyOP, delOP, addOP } = props;

  const [resp, setResp] = useState<apis.TableListResp | null>(null);
  const [err, setErr] = useState<string>('');

  const [keyMatch, setKeyMatch] = useState<string>('');
  const [showEdit, setShowEdit] = useState(false);
  const [showDelete, setShowDelete] = useState(false);

  const [loading, setLoading] = useState(false);

  const [selectIndex, setSelectIndex] = useState(0);

  const handleReload = () => {
    setLoading(true);
    setErr('');
    reloadOP(keyMatch)
      .then((e) => {
        setLoading(false);
        setResp(e);
      })
      .catch((e: Error) => {
        setErr(e.toString());
        setLoading(false);
        setResp(null);
        setTimeout(() => {
          setErr('');
        }, errOkHoldTime);
      });
  };
  useEffect(() => {
    handleReload();
  }, []);

  const editRecord = (index: number) => {
    setShowEdit(true);
    setSelectIndex(index);
  };

  const deleteRecord = (index: number) => {
    setShowDelete(true);
    setSelectIndex(index);
  };

  const newRecord = () => {
    alert('create new ');
  };

  const options = [
    { label: 'Edit', action: editRecord },
    { label: 'Delete', action: deleteRecord },
    { label: 'Create new', action: newRecord },
  ];

  const handleContextMenu = (event: any, index: number) => {
    event.preventDefault();
    setContextMenu(
      contextMenu === null
        ? {
            top: event.clientY - 6,
            left: event.clientX + 2,
            index,
          }
        : // repeated contextmenu when it is already open closes it with Chrome 84 on Ubuntu
          // Other native context menus might behave different.
          // With this behavior we prevent contextmenu from the backdrop to re-locale existing context menus.
          null
    );
  };

  return (
    <Stack>
      {!!contextMenu?.top && (
        <LongMenu
          anchorPosition={{ top: contextMenu.top, left: contextMenu.left }}
          options={options}
          handleClose={() => setContextMenu(null)}
          index={contextMenu?.index}
        />
      )}

      {err && <Alert severity='error'>{err}</Alert>}
      <Stack direction='row'>
        <TextField
          id='outlined-basic'
          label='match pattern'
          variant='outlined'
          onChange={(event) => {
            setKeyMatch(event.target.value);
          }}
        >
          {keyMatch}
        </TextField>
        &nbsp;
        <Button variant='contained' color='primary' onClick={handleReload}>
          Reload
        </Button>
        &nbsp;
        {loading && (
          <Box sx={{ display: 'flex' }}>
            <CircularProgress />
          </Box>
        )}
      </Stack>

      {!!resp && (
        <Stack>
          <TableContainer>
            <Table>
              <TableHead>
                <TableRow>
                  <TableCell>key</TableCell>
                  <TableCell>score</TableCell>
                </TableRow>
              </TableHead>
              <TableBody>
                {resp.data.map((row, index) => (
                  <TableRow
                    key={row[0]}
                    onContextMenu={(event) => handleContextMenu(event, index)}
                  >
                    <TableCell> {row[0]} </TableCell>
                    <TableCell> {row[1]} </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </TableContainer>
          <Typography variant='h6'>Total keys {resp?.total}</Typography>
        </Stack>
      )}
      <Modal
        open={showEdit}
        onClose={() => {
          setShowEdit(false);
        }}
      >
        <Box sx={modalStyle} width='500px' maxWidth='1000px'>
          <RedisEditZset
            modifyOP={async (req) => {
              try {
                await modifyOP(req);
              } catch (e) {
                throw e;
              }
              let data = [...resp.data];
              data[selectIndex][0] = req.newKey;
              data[selectIndex][1] = req.value;
              setResp({ data, total: resp?.total, fields: resp?.fields });
            }}
            handleClose={() => {
              setShowEdit(false);
            }}
            oldKey={resp?.data[selectIndex][0]}
            oldValue={resp?.data[selectIndex][1]}
          ></RedisEditZset>
        </Box>
      </Modal>

      <Modal
        open={showDelete}
        onClose={() => {
          setShowDelete(false);
        }}
      >
        <Box sx={modalStyle} width='400px'>
          <RedisDeleteZset
            delOP={async (req) => {
              try {
                await delOP(req);
              } catch (e) {
                throw e;
              }
              let data = [];
              for (var index = 0; index < resp?.data.length; index++) {
                if (index != selectIndex) {
                  data.push(resp?.data[index]);
                }
              }
              setResp({ data, total: resp?.total, fields: resp?.fields });
            }}
            handleClose={() => {
              setShowDelete(false);
            }}
            rediskey={resp?.data[selectIndex][0]}
          ></RedisDeleteZset>
        </Box>
      </Modal>
    </Stack>
  );
}

function RedisDeleteZset(props: {
  delOP: (key: string) => Promise<void>;
  rediskey: string;
  handleClose: () => void;
}) {
  const { rediskey, delOP, handleClose } = props;
  const [loading, setLoading] = useState(false);
  const [err, setErr] = useState<string>('');
  const [okMsg, setOkMsg] = useState('');

  const handleDel = () => {
    setLoading(true);
    setErr('');
    setOkMsg('');
    delOP(rediskey)
      .then(() => {
        setLoading(false);
        setOkMsg('Delete ok,Please close this.');
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
      <Stack direction='row' alignItems='right' justifyContent='space-between'>
        <span></span>
        <IconButton onClick={handleClose} edge='end' aria-label='delete'>
          <CloseIcon />
        </IconButton>
      </Stack>

      {err && <Alert severity='error'>{err}</Alert>}
      {okMsg && <Alert>{okMsg}</Alert>}
      {loading && (
        <Box sx={{ display: 'flex' }}>
          <CircularProgress />
        </Box>
      )}

      {!okMsg && (
        <Alert severity='error'>Are you sure to Delete key '{rediskey}'</Alert>
      )}
      <Stack direction='row'>
        <Button
          type='primary'
          variant='contained'
          color='error'
          onClick={handleDel}
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
function RedisEditZset(props: {
  modifyOP: (req: RedisZsetModifyReq) => Promise<void>;
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
    // reset some state
    setLoading(true);
    setErr('');
    setOkMsg('');

    modifyOP(new RedisZsetModifyReq(oldKey, newKey, newValue))
      .then(() => {
        setLoading(false);
        oldKey = newKey;
        oldValue = newValue;
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
        <Button type='primary' variant='contained' onClick={handleModify}>
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

const root = ReactDOM.createRoot(document.getElementById('root'));

root.render(
  <React.StrictMode>
    <RedisZsetTable
      reloadOP={async (_) => {
        await randomTestThrowFunction();
        return testExampleRedisZset();
      }}
      modifyOP={voidTestFunction}
      delOP={voidTestFunction}
    />
  </React.StrictMode>
);

export { RedisZsetTable };
