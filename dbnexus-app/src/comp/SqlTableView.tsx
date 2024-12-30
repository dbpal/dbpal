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
  Typography,
} from '@mui/material';
import { NotificationContext } from './Notification';
import { DataGrid, GridRowSelectionModel } from '@mui/x-data-grid';
import { v4 as uuidv4 } from 'uuid';
import {
  datagridHeaderHeight,
  datagridRowHeight,
  isTypeLargeText,
  modalStyle,
  pageSizeOptions,
  progressBarSize,
  SettingsContext,
} from './Common';
import KeyIcon from '@mui/icons-material/Key';
import AddIcon from '@mui/icons-material/Add';
function SqlTableView(props: {
  reloadOP: (
    where: string,
    orderBy: string,
    page: number,
    pageSize: number
  ) => Promise<types.MuiDataTableListResp>;
  updateOP?: (
    wheres: Map<string, string | null>,
    values: Map<string, string | null>
  ) => Promise<types.SqlExecuteRet>;
  deleteOP?: (wheres: Map<string, string>) => Promise<types.SqlExecuteRet>;
  addOP?: (values: Map<string, string | null>) => Promise<types.SqlExecuteRet>;
}) {
  let { reloadOP, updateOP, deleteOP, addOP } = props;
  const [loading, setLoading] = useState(false);
  const [where, setWhere] = useState('');
  const [triggerReload, setTriggerReload] = useState(false);
  const [orderBy, setOrderBy] = useState('');
  const [page, setPage] = useState(0);
  const [pageSize, setPageSize] = useState(
    useContext(SettingsContext).page_size
  );
  const [resp, setResp] = useState<types.MuiDataTableListResp | null>(null);
  const { showNotification } = useContext(NotificationContext);
  const [selectIDs, setSelectIDs] = useState<GridRowSelectionModel>([]);
  const [showAdd, setShowAdd] = useState(false);
  const [lastLoadTime, setLastLoadTime] = useState(new Date());
  const key_not_in_table = 'qwertyuioplkjhgfdsazxcvbnm';
  const handleResp = (resp: types.MuiDataTableListResp) => {
    resp.data.data.forEach((e) => {
      e[key_not_in_table] = uuidv4();
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
    reloadOP(where.trim(), orderBy.trim(), page + 1, pageSize)
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
    if (!updateOP) {
      return newRow;
    }
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
    const old = new Map(Object.entries(oldRow));
    old.delete(key_not_in_table);
    if (updateRow.size == 0) {
      return newRow;
    }
    try {
      let ret = await updateOP(old, updateRow);
      showNotification(
        'success',
        'Update OK',
        `sql:${ret.sql} row affected:${ret.changes}`
      );
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
      if (!selectSet.has(e[key_not_in_table])) {
        continue;
      }

      try {
        const old = new Map(Object.entries(e));
        old.delete(key_not_in_table);
        const ret = await deleteOP(old);
        showNotification(
          'success',
          'delete OK',
          `sql:${ret.sql} row affected:${ret.changes}`
        );
      } catch (e) {
        showNotification('error', 'delete failed', e);
      }
    }
    handleReload();
  };
  useEffect(() => {
    handleReload();
  }, [page, pageSize, triggerReload]);
  useEffect(() => {}, [orderBy]);
  useEffect(() => {
    setLastLoadTime(new Date());
  }, [resp]);

  return (
    <Stack spacing={1}>
      <Stack direction='row' spacing={1} alignItems='flex-start'>
        <TextField
          fullWidth
          label='where'
          variant='outlined'
          value={where}
          onBlur={() => {
            setTriggerReload((e) => !e);
          }}
          onChange={(event) => {
            setWhere(event.target.value);
          }}
          size='small'
        />
        <TextField
          fullWidth
          label='orderby'
          variant='outlined'
          value={orderBy}
          onChange={(event) => {
            setOrderBy(event.target.value);
          }}
          onBlur={() => {
            setTriggerReload((e) => !e);
          }}
          size='small'
        />

        <Button
          variant='contained'
          color='primary'
          onClick={() => {
            handleReload();
          }}
        >
          Reload
        </Button>
        {loading && <CircularProgress size={progressBarSize} />}
      </Stack>

      {resp && (
        <Stack spacing={1}>
          <Stack
            direction='row'
            spacing={1}
            sx={{ alignItems: 'flex-end', justifyContent: 'space-between' }}
          >
            <Stack direction='row' spacing={1}>
              {deleteOP && (
                <Button
                  variant='contained'
                  disabled={selectIDs.length == 0}
                  onClick={() => {
                    handleDetele();
                  }}
                >
                  <DeleteForeverIcon color='error'></DeleteForeverIcon>
                </Button>
              )}
              {addOP && (
                <Button
                  color='primary'
                  variant='contained'
                  onClick={() => {
                    setShowAdd(true);
                  }}
                >
                  <AddIcon></AddIcon>
                </Button>
              )}

              {resp && (
                <Alert color='info' severity='info'>
                  {resp.sql}
                </Alert>
              )}
            </Stack>

            {resp && (
              <Typography>loaded at:{lastLoadTime.toLocaleString()}</Typography>
            )}
            {showAdd && (
              <Modal
                open={showAdd}
                onClose={() => {
                  setShowAdd(false);
                }}
                aria-labelledby='add data'
              >
                <Box sx={modalStyle} width='800px'>
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
            onColumnVisibilityModelChange={(e) => {
              console.log('onColumnVisibilityModelChange', e);
            }}
            onColumnResize={(e) => {
              console.log('onColumnWidthChange', e);
            }}
            rowHeight={datagridRowHeight}
            columnHeaderHeight={datagridHeaderHeight}
            getRowId={(e) => e[key_not_in_table]}
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
                setTriggerReload((e) => !e);
                return;
              }
              let sort = sorts[0];
              if (!sort.sort) {
                setOrderBy('');
                setTriggerReload((e) => !e);
                return;
              }
              setOrderBy(`${sort.field} ${sort.sort}`);
              setTriggerReload((e) => !e);
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
            label={
              f.field + ' ' + f.typ + ' ' + (f.nullable ? 'null' : 'not null')
            }
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
        {loading && <CircularProgress size={progressBarSize} />}
      </Stack>
    </Stack>
  );
}

export { SqlTableView };
