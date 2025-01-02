import { useContext, useEffect, useState } from 'react';
import DeleteForeverIcon from '@mui/icons-material/DeleteForever';
import types from './ApiStruct';
import {
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
  VisiableContext,
} from './Common';
import KeyIcon from '@mui/icons-material/Key';
import AddIcon from '@mui/icons-material/Add';
import { secondConfirm } from './Confirm';

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
  saveWidth: (name: string, width: number) => void;
  loadWidths: () => Promise<object>;
  notWarnPri?: boolean;
}) {
  let {
    reloadOP,
    updateOP,
    deleteOP,
    addOP,
    saveWidth,
    loadWidths,
    notWarnPri,
  } = props;

  const [loading, setLoading] = useState(false);
  const [where, setWhere] = useState('');
  const [triggerReload, setTriggerReload] = useState(false);
  const [orderBy, setOrderBy] = useState('');
  const [page, setPage] = useState(0);
  const [pageSize, setPageSize] = useState(
    useContext(SettingsContext).pageSize
  );

  const [widths, setWidths] = useState(null);
  const [resp, setResp] = useState<types.MuiDataTableListResp | null>(null);
  const { showNotification } = useContext(NotificationContext);
  const [selectIDs, setSelectIDs] = useState<GridRowSelectionModel>([]);
  const [showAdd, setShowAdd] = useState(false);
  const [lastLoadTime, setLastLoadTime] = useState(new Date());
  const key_not_in_table = 'qwertyuioplkjhgfdsazxcvbnm';

  const visiable = useContext(VisiableContext);

  const handleResp = async (resp: types.MuiDataTableListResp) => {
    resp.data.data.forEach((data) => {
      data[key_not_in_table] = uuidv4();
    });
    // https://www.geeksforgeeks.org/is-setstate-method-async/
    // set state 是异步的也就说setWidths 可能读不到它的值呢
    let widths2;
    if (widths == null) {
      let x = await loadWidths();
      setWidths(x);
      widths2 = x;
    } else {
      widths2 = widths;
    }
    for (let i = 0; i < resp.data.fields.length; i++) {
      let field = resp.data.fields[i];
      //@ts-ignore
      field.flex = 1;
      if (widths2[field.field]) {
        //@ts-ignore
        field.width = widths2[field.field];
        //@ts-ignore
        field.flex = 0;
      }
      //@ts-ignore
      field.renderHeader = (e) => {
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

      if (!field.is_auto_increment) {
        //@ts-ignore
        field.editable = true;
      }
    }
  };

  const confrimAboutPrimary = async (): Promise<boolean> => {
    if (notWarnPri) {
      return true;
    }
    const hasPri = !!resp.data.fields.find((x) => x.is_primary);
    if (hasPri) {
      return true;
    }
    return secondConfirm({
      title: 'No Primary key found.',
      content: 'No Primary key found,maybe multi row affected.',
    });
  };
  const handleReload = async () => {
    setLoading(true);
    try {
      let resp = await reloadOP(
        where.trim(),
        orderBy.trim(),
        page + 1,
        pageSize
      );
      setLoading(false);
      await handleResp(resp);
      setResp(resp);
    } catch (e) {
      setLoading(false);
      showNotification('error', 'query data failed', e);
      return;
    }
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
    if (!(await confrimAboutPrimary())) {
      return oldRow;
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
    if (!(await confrimAboutPrimary())) {
      return;
    }
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
          label='order by'
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
          size='small'
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
            <Stack direction='row' spacing={1} sx={{ alignItems: 'flex-end' }}>
              {deleteOP && (
                <Button
                  variant='contained'
                  disabled={selectIDs.length == 0}
                  onClick={() => {
                    handleDetele();
                  }}
                  size='small'
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
                  size='small'
                >
                  <AddIcon></AddIcon>
                </Button>
              )}
              {resp && <Typography variant='subtitle1'>{resp.sql}</Typography>}
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
          {visiable && (
            <DataGrid
              onColumnResize={(e) => {
                if (saveWidth) {
                  saveWidth(e.colDef.field, e.width);
                }
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
          )}
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
