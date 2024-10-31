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
  MenuItem,
  FormControl,
  InputLabel,
  Select,
  Pagination,
  TableSortLabel,
} from '@mui/material';

import ReactDOM from 'react-dom/client';

import AddIcon from '@mui/icons-material/Add';

import apis from './ApiStruct';
import {
  generateRandomString,
  randomTestThrowFunction,
  voidTestFunction,
} from './helper';

import {
  testExampleRedisHash,
  testExampleRedisSetList,
  testExampleRedisZset,
} from './exampleTest';
import {
  CloseSpace,
  errOkHoldTime,
  modalStyle,
  SelectPageSize,
} from './common';
import LongMenu from './LongMenu';

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

function RedisZsetTableByRange(props: {
  reloadOP: (
    min: number | null,
    max: number | null,
    page: number,
    pageSize: number
  ) => Promise<apis.TableListResp>;
}) {
  const { reloadOP } = props;

  const [resp, setResp] = useState<apis.TableListResp | null>(null);
  const [err, setErr] = useState<string>('');
  const [min, setMin] = useState<number | null>(null);
  const [minErr, setMinErr] = useState(false);
  const [max, setMax] = useState<number | null>(null);
  const [maxErr, setMaxErr] = useState(false);
  const [page, setPage] = useState(1);
  const [pageSize, setPageSize] = useState(100);
  const [gotoPage, setGotoPage] = useState<string>('');
  const [gotoPageErr, setGotoPageErr] = useState(false);

  const [order, setOrder] = useState('asc');

  const handleRequestSort = () => {
    setOrder(order == 'asc' ? 'desc' : 'asc');
  };
  const sortedRows = () => {
    return [...resp?.data].sort((a, b) => {
      if (a[0] < b[0]) {
        return order === 'asc' ? -1 : 1;
      }
      if (a[0] > b[0]) {
        return order === 'asc' ? 1 : -1;
      }
      return 0;
    });
  };

  const [loading, setLoading] = useState(false);

  const handleReload = () => {
    if (min != null && max != null && min > max) {
      setErr('max must bigger than min.');
      setTimeout(() => {
        setErr('');
      }, errOkHoldTime);
      return;
    }

    setLoading(true);
    setErr('');
    reloadOP(min, max, page, pageSize)
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
  }, [page, pageSize]);

  useEffect(() => {
    handleReload();
  }, []);

  const getPageCount = () => {
    if (null == resp) {
      return 0;
    }
    var c = Math.ceil(resp.total / pageSize);

    return c;
  };

  return (
    <Stack>
      {err && <Alert severity='error'>{err}</Alert>}
      <Stack direction='row'>
        <TextField
          id='outlined-basic'
          label='min'
          variant='outlined'
          error={minErr}
          helperText='put a valid float64'
          onChange={(event) => {
            let value = parseFloat(event.target.value);
            if (!isNaN(value)) {
              setMin(value);
              setMinErr(false);
            } else {
              setMinErr(true);
            }
          }}
        >
          {min}
        </TextField>
        <TextField
          id='outlined-basic'
          label='max'
          variant='outlined'
          error={maxErr}
          helperText='put a valid float64'
          onChange={(event) => {
            let value = parseFloat(event.target.value);
            if (!isNaN(value)) {
              setMax(value);
              setMaxErr(false);
            } else {
              setMaxErr(true);
            }
          }}
        >
          {max}
        </TextField>
        <SelectPageSize
          pageSize={pageSize}
          setPageSize={setPageSize}
        ></SelectPageSize>
        &nbsp;
        <Button variant='contained' color='primary' onClick={handleReload}>
          Apply
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
                  <TableCell> key</TableCell>
                  <TableCell>
                    <TableSortLabel
                      active={order != ''}
                      direction={order || 'asc'}
                      onClick={handleRequestSort}
                    >
                      score
                    </TableSortLabel>
                  </TableCell>
                </TableRow>
              </TableHead>
              <TableBody>
                {sortedRows().map((row) => (
                  <TableRow key={row[0]}>
                    <TableCell> {row[0]} </TableCell>
                    <TableCell> {row[1]} </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </TableContainer>
          {/* here is the paging */}
          <Stack direction='row'>
            <Pagination
              count={getPageCount()}
              page={page}
              color='primary'
              onChange={(_event, p) => {
                setPage(p);
              }}
              shape='rounded'
            />
            &nbsp;
            {getPageCount() > 25 && (
              <Stack direction='row'>
                <TextField
                  value={gotoPage}
                  label='goto page'
                  error={gotoPageErr}
                  onChange={(e) => {
                    setGotoPage(e.target.value);
                    if (isNaN(parseInt(e.target.value))) {
                      setGotoPageErr(true);
                    } else {
                      setGotoPageErr(false);
                    }
                  }}
                >
                  {gotoPage}
                </TextField>
                <Button
                  color='primary'
                  variant='contained'
                  onClick={() => {
                    let v = parseInt(gotoPage);
                    if (!isNaN(v)) {
                      setPage(v);
                    }
                  }}
                >
                  goto page
                </Button>
              </Stack>
            )}
          </Stack>

          <Typography variant='h6'>
            Result {resp.data.length} of {resp.total}
          </Typography>
        </Stack>
      )}
    </Stack>
  );
}

function RedisListTable(props: {
  reloadOP: (page: number, pageSize: number) => Promise<apis.TableListResp>;
  lpush: (value: string) => Promise<void>;
  rpush: (value: string) => Promise<void>;
}) {
  const { reloadOP, lpush, rpush } = props;
  const [resp, setResp] = useState<apis.TableListResp | null>(null);
  const [err, setErr] = useState<string>('');
  const [page, setPage] = useState(1);
  const [pageSize, setPageSize] = useState(100);
  const [gotoPage, setGotoPage] = useState<string>('');
  const [gotoPageErr, setGotoPageErr] = useState(false);

  const [loading, setLoading] = useState(false);

  const getSeq = () => {
    return (page - 1) * pageSize;
  };
  const [seq, setSeq] = useState(getSeq());

  useEffect(() => {
    setSeq(getSeq());
  }, [page, pageSize]);

  const handleReload = () => {
    setLoading(true);
    setErr('');
    reloadOP(page, pageSize)
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
  }, [page, pageSize]);

  useEffect(() => {
    handleReload();
  }, []);

  const getPageCount = () => {
    if (null == resp) {
      return 0;
    }
    var c = Math.ceil(resp.total / pageSize);
    return c;
  };
  const [showAdd, setShowAdd] = useState(false);
  return (
    <Stack>
      {err && <Alert severity='error'>{err}</Alert>}
      <Stack direction='row'>
        <SelectPageSize
          pageSize={pageSize}
          setPageSize={setPageSize}
        ></SelectPageSize>
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
                  <TableCell>seq</TableCell>
                  <TableCell>value</TableCell>
                </TableRow>
              </TableHead>
              <TableBody>
                {resp.data.map((row, index) => (
                  <TableRow key={row[0]}>
                    <TableCell> {seq + index} </TableCell>
                    <TableCell> {row[0]} </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </TableContainer>
          {/* here is the paging */}
          <Stack direction='row'>
            <Pagination
              count={getPageCount()}
              page={page}
              color='primary'
              onChange={(_event, p) => {
                setPage(p);
              }}
              shape='rounded'
            />
            &nbsp;
            {getPageCount() > 25 && (
              <Stack direction='row'>
                <TextField
                  value={gotoPage}
                  label='goto page'
                  error={gotoPageErr}
                  onChange={(e) => {
                    setGotoPage(e.target.value);
                    if (isNaN(parseInt(e.target.value))) {
                      setGotoPageErr(true);
                    } else {
                      setGotoPageErr(false);
                    }
                  }}
                >
                  {gotoPage}
                </TextField>
                <Button
                  color='primary'
                  variant='contained'
                  onClick={() => {
                    let v = parseInt(gotoPage);
                    if (!isNaN(v)) {
                      setPage(v);
                    }
                  }}
                >
                  goto page
                </Button>
              </Stack>
            )}
          </Stack>

          <Typography variant='h6'>
            Result {resp.data.length} of {resp.total}
          </Typography>
        </Stack>
      )}
      {
        // add item button
      }
      <Stack direction='row'>
        <IconButton
          onClick={() => {
            //
            setShowAdd(true);
          }}
          color='primary'
        >
          <AddIcon />
        </IconButton>
      </Stack>
      {/*  show add modal */}
      <Modal
        open={showAdd}
        onClose={() => {
          setShowAdd(false);
        }}
      >
        <Box sx={modalStyle} width='500px' maxWidth='1000px'>
          <RedisListAdd
            lpush={async (req) => {
              try {
                await lpush(req);
              } catch (e) {
                throw e;
              }
              handleReload();
            }}
            rpush={async (req) => {
              try {
                await rpush(req);
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

function RedisHashTable(props: {
  reloadOP: (keymatch: string | null) => Promise<apis.TableListResp>;
  modifyOP: (req: RedisZsetHashModifyReq) => Promise<void>;
  delOP: (req: string) => Promise<void>;
  addOP: (key: String, value: string) => Promise<void>;
}) {
  const [contextMenu, setContextMenu] = React.useState<{
    top: number;
    left: number;
    index: string;
  } | null>(null);

  const { reloadOP, modifyOP, delOP, addOP } = props;

  const [resp, setResp] = useState<apis.TableListResp | null>(null);
  const [err, setErr] = useState<string>('');

  const [keyMatch, setKeyMatch] = useState<string>('');
  const [showEdit, setShowEdit] = useState(false);
  const [showDelete, setShowDelete] = useState(false);

  const [loading, setLoading] = useState(false);

  const [selectKey, setSelectkey] = useState('');
  const [showAdd, setShowAdd] = useState(false);
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
  const [order, setOrder] = useState('');

  const handleRequestSort = () => {
    setOrder(order == 'asc' ? 'desc' : 'asc');
  };

  const sortedRows = () => {
    return [...resp?.data].sort((a, b) => {
      if (!order) {
        return 0;
      }
      if (a[0] < b[0]) {
        return order === 'asc' ? -1 : 1;
      }
      if (a[0] > b[0]) {
        return order === 'asc' ? 1 : -1;
      }
      return 0;
    });
  };

  useEffect(() => {
    handleReload();
  }, []);

  const editRecord = (index: string) => {
    setShowEdit(true);
    setSelectkey(index);
  };

  const deleteRecord = (index: string) => {
    setShowDelete(true);
    setSelectkey(index);
  };

  const newRecord = () => {
    setShowAdd(true);
  };

  const options = [
    { label: 'Edit', action: editRecord },
    { label: 'Delete', action: deleteRecord },
    { label: 'Create new', action: newRecord },
  ];

  const handleContextMenu = (event: any, index: string) => {
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
  const getOldValue = (key: string) => {
    const e = resp?.data.find((e) => e[0] == key);
    return e ? e[1] : '';
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
          helperText='valid syntax are key* , *key , *key*'
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
                  <TableCell>
                    <TableSortLabel
                      active={order != ''}
                      direction={order || 'asc'}
                      onClick={handleRequestSort}
                    >
                      key
                    </TableSortLabel>
                  </TableCell>
                  <TableCell>score</TableCell>
                </TableRow>
              </TableHead>
              <TableBody>
                {sortedRows().map((row) => (
                  <TableRow
                    key={row[0]}
                    onContextMenu={(event) =>
                      handleContextMenu(event, row[0] as string)
                    }
                  >
                    <TableCell> {row[0]} </TableCell>
                    <TableCell> {row[1]} </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </TableContainer>
          <Typography variant='h6'>
            Result {resp.data.length} of {resp.total}
          </Typography>
        </Stack>
      )}
      <Modal
        open={showEdit}
        onClose={() => {
          setShowEdit(false);
        }}
      >
        <Box sx={modalStyle} width='500px' maxWidth='1000px'>
          <RedisEditHash
            modifyOP={async (req) => {
              try {
                await modifyOP(req);
              } catch (e) {
                throw e;
              }
              let data = [...resp.data];
              data.forEach((e) => {
                if ((e[0] as string) == selectKey) {
                  e[0] = req.newKey;
                  e[1] = req.value;
                }
              });
              setSelectkey(req.newKey);
              setResp({
                data,
                total: resp?.total,
                fields: resp?.fields as apis.Field[],
              });
            }}
            handleClose={() => {
              setShowEdit(false);
            }}
            oldKey={selectKey}
            oldValue={getOldValue(selectKey)}
          ></RedisEditHash>
        </Box>
      </Modal>

      <Modal
        open={showDelete}
        onClose={() => {
          setShowDelete(false);
        }}
      >
        <Box sx={modalStyle} width='400px'>
          <RedisDelKey
            delOP={async (req) => {
              try {
                await delOP(req);
              } catch (e) {
                throw e;
              }
              let data = [];
              for (var index = 0; index < resp?.data.length; index++) {
                if (resp?.data[index][0] != selectKey) {
                  data.push(resp?.data[index]);
                }
              }

              setResp({
                data,
                total: (resp?.total as number) - 1,
                fields: resp?.fields,
              });
            }}
            handleClose={() => {
              setShowDelete(false);
            }}
            rediskey={selectKey}
          ></RedisDelKey>
        </Box>
      </Modal>
      <Modal
        open={showAdd}
        onClose={() => {
          setShowDelete(false);
        }}
      >
        <Box sx={modalStyle} width='500px'>
          <RedisAddHash
            addOP={async (key, value) => {
              try {
                await addOP(key, value);
              } catch (e) {
                throw e;
              }
              let data = [...resp?.data];
              var e = data.find((e) => {
                return e[0] == key;
              });
              if (e) {
                e[1] = value;
              } else {
                data.push([key, value]);
              }
              setResp({ data, total: resp?.total + 1, fields: resp?.fields });
            }}
            handleClose={() => {
              setShowAdd(false);
            }}
          ></RedisAddHash>
        </Box>
      </Modal>
    </Stack>
  );
}
/// zset table main entry point.
function RedisZsetTable(props: {
  reloadOP: (keymatch: string | null) => Promise<apis.TableListResp>;
  modifyOP: (req: RedisZsetHashModifyReq) => Promise<void>;
  delOP: (req: string) => Promise<void>;
  addOP: (key: String, value: string) => Promise<void>;
}) {
  const [contextMenu, setContextMenu] = React.useState<{
    top: number;
    left: number;
    index: number;
  } | null>(null);

  const { reloadOP, modifyOP, delOP, addOP } = props;

  const [resp, setResp] = useState<apis.TableListResp | null>(null);
  const [err, setErr] = useState<string>('');

  const [keyMatch, setKeyMatch] = useState<string>('');
  const [showEdit, setShowEdit] = useState(false);
  const [showDelete, setShowDelete] = useState(false);

  const [loading, setLoading] = useState(false);

  const [selectKey, setSelectKey] = useState(0);
  const [showAdd, setShowAdd] = useState(false);

  const [order, setOrder] = useState('asc');

  const handleRequestSort = () => {
    setOrder(order == 'asc' ? 'desc' : 'asc');
  };

  const sortedRows = () => {
    return [...resp?.data].sort((a, b) => {
      if (a[1] < b[1]) {
        return order === 'asc' ? -1 : 1;
      }
      if (a[1] > b[1]) {
        return order === 'asc' ? 1 : -1;
      }
      return 0;
    });
  };

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
    setSelectKey(index);
  };

  const deleteRecord = (index: number) => {
    setShowDelete(true);
    setSelectKey(index);
  };

  const newRecord = () => {
    setShowAdd(true);
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

  const getOldValue = (key: string) => {
    const e = resp?.data.find((e) => e[0] == key);
    return e ? e[1] : '';
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
          helperText='valid syntax are key* , *key , *key*'
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
                  <TableCell> key </TableCell>
                  <TableCell>
                    <TableSortLabel
                      active={order != ''}
                      direction={order}
                      onClick={handleRequestSort}
                    >
                      score
                    </TableSortLabel>
                  </TableCell>
                </TableRow>
              </TableHead>
              <TableBody>
                {sortedRows().map((row) => (
                  <TableRow
                    key={row[0]}
                    onContextMenu={(event) => handleContextMenu(event, row[0])}
                  >
                    <TableCell> {row[0]} </TableCell>
                    <TableCell> {row[1]} </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </TableContainer>
          <Typography variant='h6'>
            Result {resp.data.length} of {resp.total}
          </Typography>
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
              const e = data.find((e) => e[0] == selectKey);
              if (e) {
                e[0] = req.newKey;
                e[1] = req.value;
              }
              setSelectKey(req.newKey);
              setResp({
                data,
                total: resp?.total,
                fields: resp?.fields as apis.Field[],
              });
            }}
            handleClose={() => {
              setShowEdit(false);
            }}
            oldKey={selectKey}
            oldValue={getOldValue(selectKey)}
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
          <RedisDelKey
            delOP={async (req) => {
              try {
                await delOP(req);
              } catch (e) {
                throw e;
              }
              let data = [];
              for (var index = 0; index < resp?.data.length; index++) {
                if (resp?.data[index][0] != selectKey) {
                  data.push(resp?.data[index]);
                }
              }
              setResp({ data, total: resp?.total - 1, fields: resp?.fields });
            }}
            handleClose={() => {
              setShowDelete(false);
            }}
            rediskey={selectKey}
          ></RedisDelKey>
        </Box>
      </Modal>

      <Modal
        open={showAdd}
        onClose={() => {
          setShowDelete(false);
        }}
      >
        <Box sx={modalStyle} width='500px'>
          <RedisAddZset
            addOP={async (key, score) => {
              try {
                await addOP(key, score);
              } catch (e) {
                throw e;
              }
              let data = [...resp?.data];
              var e = data.find((e) => {
                return e[0] == key;
              });
              if (e) {
                e[1] = score;
              } else {
                data.push([key, score]);
              }
              setResp({ data, total: resp?.total + 1, fields: resp?.fields });
            }}
            handleClose={() => {
              setShowAdd(false);
            }}
          ></RedisAddZset>
        </Box>
      </Modal>
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

function RedisSetTable(props: {
  reloadOP: (keymatch: string | null) => Promise<apis.TableListResp>;
  modifyOP: (oldKey: string, newKey: String) => Promise<void>;
  delOP: (req: string) => Promise<void>;
  addOP: (key: String) => Promise<void>;
}) {
  const [contextMenu, setContextMenu] = React.useState<{
    top: number;
    left: number;
    index: number;
  } | null>(null);

  const { reloadOP, modifyOP, delOP, addOP } = props;

  const [resp, setResp] = useState<apis.TableListResp | null>(null);
  const [err, setErr] = useState<string>('');
  const [keyMatch, setKeyMatch] = useState<string>('');
  const [showEdit, setShowEdit] = useState(false);
  const [showDelete, setShowDelete] = useState(false);

  const [loading, setLoading] = useState(false);

  const [selectKey, setSelectKey] = useState(0);
  const [showAdd, setShowAdd] = useState(false);

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
    setSelectKey(index);
  };

  const deleteRecord = (index: number) => {
    setShowDelete(true);
    setSelectKey(index);
  };

  const newRecord = () => {
    setShowAdd(true);
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
          helperText='valid syntax are key* , *key , *key*'
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
                  <TableCell>Set</TableCell>
                </TableRow>
              </TableHead>
              <TableBody>
                {resp.data.map((row) => (
                  <TableRow
                    key={row[0]}
                    onContextMenu={(event) => handleContextMenu(event, row[0])}
                  >
                    <TableCell> {row[0]} </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </TableContainer>
          <Typography variant='h6'>
            Result {resp.data.length} of {resp.total}
          </Typography>
        </Stack>
      )}

      <Modal
        open={showEdit}
        onClose={() => {
          setShowEdit(false);
        }}
      >
        <Box sx={modalStyle} width='500px' maxWidth='1000px'>
          <RedisEditSet
            modifyOP={async (oldkey, newKey) => {
              try {
                await modifyOP(oldkey, newKey);
              } catch (e) {
                throw e;
              }

              let data = [...resp.data];
              var e = data.find((e) => {
                return e[0] == oldkey;
              });
              if (e) {
                e[0] = newKey;
              }
              setSelectKey(newKey);
              setResp({
                data,
                total: resp?.total,
                fields: resp?.fields as apis.Field[],
              });
            }}
            handleClose={() => {
              setShowEdit(false);
            }}
            oldKey={selectKey}
          ></RedisEditSet>
        </Box>
      </Modal>

      <Modal
        open={showDelete}
        onClose={() => {
          setShowDelete(false);
        }}
      >
        <Box sx={modalStyle} width='400px'>
          <RedisDelKey
            delOP={async (req) => {
              try {
                await delOP(req);
              } catch (e) {
                throw e;
              }
              let data = [];
              for (var index = 0; index < resp?.data.length; index++) {
                if (resp?.data[index][0] != selectKey) {
                  data.push(resp?.data[index]);
                }
              }

              setResp({
                data,
                total: resp?.total - 1,
                fields: resp?.fields as apis.Field[],
              });
            }}
            handleClose={() => {
              setShowDelete(false);
            }}
            rediskey={selectKey}
          ></RedisDelKey>
        </Box>
      </Modal>

      <Modal
        open={showAdd}
        onClose={() => {
          setShowDelete(false);
        }}
      >
        <Box sx={modalStyle} width='500px'>
          <RedisAddSet
            addOP={async (key) => {
              try {
                await addOP(key);
              } catch (e) {
                throw e;
              }
              let data = [...resp?.data];
              var e = data.find((e) => {
                return e[0] == key;
              });
              if (!e) {
                data.push([key]);
              }
              setResp({
                data,
                total: resp?.total + 1,
                fields: resp?.fields as apis.Field[],
              });
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

const root = ReactDOM.createRoot(document.getElementById('root'));

root.render(
  <React.StrictMode>
    <h1>list </h1>
    <RedisListTable
      reloadOP={async (_) => {
        await randomTestThrowFunction();
        return testExampleRedisSetList();
      }}
      lpush={voidTestFunction}
      rpush={voidTestFunction}
    />
    <hr />

    <h1> Zset </h1>
    <RedisZsetTable
      reloadOP={async (_) => {
        await randomTestThrowFunction();
        return testExampleRedisZset();
      }}
      modifyOP={voidTestFunction}
      delOP={voidTestFunction}
      addOP={voidTestFunction}
    />
    <hr />

    <h1> Zset by range </h1>
    <RedisZsetTableByRange
      reloadOP={async (_) => {
        await randomTestThrowFunction();
        return testExampleRedisZset();
      }}
    ></RedisZsetTableByRange>
    <hr />

    <h1> Set </h1>
    <RedisSetTable
      reloadOP={async (_) => {
        await randomTestThrowFunction();
        return testExampleRedisSetList();
      }}
      modifyOP={voidTestFunction}
      delOP={voidTestFunction}
      addOP={voidTestFunction}
    />

    <h1> Hash </h1>
    <RedisHashTable
      reloadOP={async (_) => {
        await randomTestThrowFunction();
        return testExampleRedisHash();
      }}
      modifyOP={voidTestFunction}
      delOP={voidTestFunction}
      addOP={voidTestFunction}
    />

    <h1> string </h1>

    <RedisStringTable
      modifyOP={voidTestFunction}
      content={generateRandomString(1000)}
    ></RedisStringTable>
  </React.StrictMode>
);

export {
  RedisZsetTable,
  RedisZsetTableByRange,
  RedisStringTable,
  RedisSetTable,
  RedisHashTable,
};
