import { useContext, useEffect, useState } from 'react';
import DeleteForeverIcon from '@mui/icons-material/DeleteForever';
import types from './ApiStruct';
import {
  Alert,
  Box,
  Button,
  CircularProgress,
  Modal,
  Stack,
  TextField,
  Tooltip,
} from '@mui/material';
import { NotificationContext } from './Notification';
import { DataGrid, GridRowSelectionModel } from '@mui/x-data-grid';
import { v4 as uuidv4 } from 'uuid';
import { isTypeLargeText, modalStyle, pageSizeOptions } from './common';
import KeyIcon from '@mui/icons-material/Key';
import AddIcon from '@mui/icons-material/Add';
function SqlTableView(props: {
  reloadOP: (
    where: string,
    orderBy: string,
    page: number,
    pageSize: number
  ) => Promise<types.MuiDataTableListResp>;
  updateOP: (
    wheres: Map<string, string>,
    updates: Map<string, string>
  ) => Promise<types.SqlExecuteRet>;
  deleteOP: (wheres: Map<string, string>) => Promise<void>;
  addOP: (values: Map<string, string>) => Promise<void>;
}) {
  let { reloadOP, updateOP, deleteOP, addOP } = props;
  const [loading, setLoading] = useState(false);
  const [where, setWhere] = useState('');
  const [orderBy, setOrderBy] = useState('');
  const [page, setPage] = useState(1);
  const [pageSize, setPageSize] = useState(10);
  const [resp, setResp] = useState<types.MuiDataTableListResp | null>(null);
  const { showNotification } = useContext(NotificationContext);
  const [selectIDs, setSelectIDs] = useState<GridRowSelectionModel>([]);
  const [showAdd, setShowAdd] = useState(false);
  const uuid_key = 'qwertyuioplkjhgfdsazxcvbnm';

  const handleResp = (resp: types.MuiDataTableListResp) => {
    resp.data.data.forEach((e) => {
      e[uuid_key] = uuidv4();
    });
    resp.data.fields.forEach((e) => {
      //@ts-ignore
      e.flex = 1;
      //@ts-ignore
      e.renderHeader = (e) => {
        let def = e.colDef;
        const makeTitle = (): string => {
          let f = def as types.Field;
          let ret = f.typ;
          if (f.nullable) {
            ret += ' null';
          } else {
            ret += ' not null';
          }
          if (f.is_auto_increment) {
            ret += ' auto increment';
          }
          if (f.default_value) {
            ret += ' ' + f.default_value;
          }
          return ret;
        };
        return (
          <Tooltip title={makeTitle()}>
            <strong>
              {def.field} {def.is_primary && <KeyIcon></KeyIcon>}
            </strong>
          </Tooltip>
        );
      };

      if (!e.is_auto_increment) {
        //@ts-ignore
        e.editable = true;
      }
    });
  };

  const handleReload = () => {
    setLoading(false);
    reloadOP(where, orderBy, page, pageSize)
      .then((resp) => {
        setLoading(false);
        handleResp(resp);
        setResp(resp);
      })
      .catch((e) => {
        setLoading(false);
        showNotification('error', 'query data failed', e);
      });
  };
  const handleProcessRowUpdate = async (
    newRow: Map<string, string>,
    oldRow: Map<string, string>
  ) => {
    const findDiff = (): Map<string, string | null> => {
      let m = new Map();
      for (const key in newRow) {
        if (newRow[key] != oldRow[key]) {
          m.set(key, newRow[key]);
        }
      }
      return m;
    };
    const updateRow = findDiff();
    try {
      await updateOP(oldRow, updateRow);
    } catch (e) {
      throw e;
    }
    return newRow;
  };

  const handleDetele = async () => {
    let selectSet = new Set();
    selectIDs.forEach((e) => {
      selectSet.add(e);
    });

    for (let i = 0; i < resp.data.data.length; i++) {
      let e = resp.data.data[i];
      if (!selectSet.has(e[uuid_key])) {
        continue;
      }
      try {
        //@ts-ignore
        await deleteOP(e);
      } catch (e) {
        showNotification('error', 'delete failed', e);
      }
    }
    handleReload();
  };

  useEffect(() => {
    handleReload();
  }, [page, pageSize]);

  return (
    <Stack spacing={1}>
      <Stack direction='row' spacing={1} alignItems='flex-start'>
        <TextField
          label='where'
          variant='outlined' // 可以是 'outlined', 'filled', or 'standard'
          value={where}
          onChange={(event) => {
            setWhere(event.target.value);
          }}
          size='small'
        />
        <TextField
          label='orderby'
          variant='outlined' // 可以是 'outlined', 'filled', or 'standard'
          value={orderBy}
          onChange={(event) => {
            setOrderBy(event.target.value);
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
        {loading && (
          <Box sx={{ display: 'flex' }}>
            <CircularProgress />
          </Box>
        )}

        {resp && <Alert color='info'> {resp.sql}</Alert>}
      </Stack>

      {resp && (
        <Stack spacing={1}>
          <Stack direction='row' spacing={1}>
            <Button
              variant='contained'
              color='error'
              disabled={selectIDs.length == 0}
              onClick={() => {
                handleDetele();
              }}
            >
              <DeleteForeverIcon></DeleteForeverIcon>
            </Button>
            <Button
              color='primary'
              variant='contained'
              onClick={() => {
                setShowAdd(true);
              }}
            >
              <AddIcon></AddIcon>
            </Button>

            {showAdd && (
              <Modal
                open={showAdd}
                onClose={() => {
                  setShowAdd(false);
                }}
                aria-labelledby='set filter'
              >
                <Box sx={modalStyle} width='600px'>
                  <SqlTableViewAdd
                    addOP={async (args) => {
                      try {
                        await addOP(args);
                      } catch (e) {
                        throw e;
                      }
                      handleReload();
                    }}
                    fields={resp.data.fields}
                  ></SqlTableViewAdd>
                </Box>
              </Modal>
            )}
          </Stack>

          <DataGrid
            getRowId={(e) => e[uuid_key]}
            paginationModel={{ page, pageSize }}
            pageSizeOptions={pageSizeOptions}
            paginationMode='server'
            onPaginationModelChange={(e) => {
              setPage(e.page);
              setPageSize(e.pageSize);
            }}
            checkboxSelection
            onRowSelectionModelChange={(e) => {
              setSelectIDs(e);
            }}
            rowCount={resp.total}
            sortingMode='server'
            onSortModelChange={(sorts) => {
              if (sorts.length == 0) {
                setOrderBy('');
                return;
              }
              let sort = sorts[0];
              if (!sort.sort) {
                setOrderBy('');
                return;
              }
              setOrderBy(`${sort.field} ${sort.sort}`);
            }}
            columns={resp.data.fields}
            rows={resp.data.data}
            //@ts-ignore
            processRowUpdate={handleProcessRowUpdate}
            onProcessRowUpdateError={(e) => {
              showNotification('error', 'update failed', e);
            }}
          ></DataGrid>
        </Stack>
      )}
    </Stack>
  );
}

function SqlTableViewAdd(props: {
  addOP: (args: any) => Promise<void>;
  fields: types.Field[];
}) {
  let { addOP, fields } = props;
  const [loading, setLoading] = useState(false);
  const [args, setArgs] = useState({});
  const { showNotification } = useContext(NotificationContext);

  const handleAdd = () => {
    // verify not null
    // TODO more check

    for (let i = 0; i < fields.length; i++) {
      let f = fields[i];
      if (f.is_auto_increment) {
        continue;
      }
      console.log(args[f.field], f.nullable);
      if (!args[f.field] && !f.nullable) {
        showNotification('error', `filed '${f.field}' need be filled`);
        return;
      }
    }
    setLoading(true);
    addOP(args)
      .then(() => {
        setLoading(false);
        showNotification('success', 'Add OK');
      })
      .catch((e) => {
        setLoading(false);
        showNotification('error', 'Add failure', e);
      });
  };

  return (
    <Stack spacing={1}>
      {fields.map((f) => {
        if (f.is_auto_increment) {
          return;
        }
        return (
          <TextField
            value={args[f.field]}
            key={f.field}
            label={f.field + ' ' + f.typ}
            required={!f.nullable}
            multiline={isTypeLargeText(f.typ)}
            rows={4}
            onChange={(e) => {
              const value = e.target.value;
              const v = value ? value : null;
              let newArgs = { ...args };
              newArgs[f.field] = v;
              setArgs(newArgs);
            }}
          ></TextField>
        );
      })}
      <Stack direction='row' spacing={1}>
        <Button variant='contained' onClick={handleAdd}>
          Add
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

export { SqlTableView };
