import {
  CSSProperties,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
} from 'react';

import types from './ApiStruct';
import {
  Box,
  Button,
  CircularProgress,
  ListItemIcon,
  ListItemText,
  MenuItem,
  MenuList,
  Modal,
  Paper,
  Stack,
  TextField,
  Tooltip,
  Typography,
} from '@mui/material';
import { NotificationContext } from './Notification';
import { DataGrid, GridSlotsComponentsProps } from '@mui/x-data-grid';
import { v4 as uuidv4 } from 'uuid';
import {
  ContextMenuContext,
  keyNotInTable,
  modalStyle,
  progressBarSize,
  reloadOK,
  SettingsContext,
  VisiableContext,
} from './Common';
import DeleteForeverIcon from '@mui/icons-material/DeleteForever';
import ContentCopyIcon from '@mui/icons-material/ContentCopy';
import KeyIcon from '@mui/icons-material/Key';
import AddIcon from '@mui/icons-material/Add';

import {
  calMenuStyle,
  cloumnWidths,
  handleCopyToClipboard,
  isTypeLargeText,
  saveColumnWidths,
  secondConfirm,
} from './helper';
import {
  datagridHeaderHeight,
  datagridRowHeight,
  pageSizeOptions,
} from './CommonComp';

const confrimAboutPrimary = async (
  notWarnPri,
  resp: types.MuiDataTableListResp
): Promise<boolean> => {
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

const getRowId = (e) => e[keyNotInTable];

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
  notWarnPri?: boolean;
}) {
  let { reloadOP, updateOP, deleteOP, addOP, notWarnPri } = props;

  const [loading, setLoading] = useState(false);
  const [where, setWhere] = useState('');
  const [triggerReload, setTriggerReload] = useState(false);
  const [orderBy, setOrderBy] = useState('');
  const [page, setPage] = useState(0);
  const [pageSize, setPageSize] = useState(
    useContext(SettingsContext).pageSize
  );

  const [resp, setResp] = useState<types.MuiDataTableListResp | null>(null);
  const { showNotification } = useContext(NotificationContext);
  const [showAdd, setShowAdd] = useState(false);
  const [lastLoadTime, setLastLoadTime] = useState(new Date());

  const visiable = useContext(VisiableContext);

  const handleResp = (resp: types.MuiDataTableListResp) => {
    resp.data.data.forEach((data) => {
      data[keyNotInTable] = uuidv4();
    });

    for (let i = 0; i < resp.data.fields.length; i++) {
      let field = resp.data.fields[i];
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

  const handleReload = async (showOK: boolean) => {
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

      resp.data.fields.forEach((e) => {
        const width = cloumnWidths[e.field];
        if (width) {
          e['width'] = width;
        }
      });
      setResp(resp);
      if (showOK) {
        reloadOK(showNotification);
      }
    } catch (e) {
      setLoading(false);
      showNotification('error', 'query data failed', e);
      return;
    }
  };

  const handleProcessRowUpdate = useCallback(
    async (newRow: Map<string, string>, oldRow: Map<string, string>) => {
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
      old.delete(keyNotInTable);
      if (updateRow.size == 0) {
        return newRow;
      }
      if (!(await confrimAboutPrimary(notWarnPri, resp))) {
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
    },
    [resp]
  );

  const handleDetele = async (id) => {
    if (!(await confrimAboutPrimary(notWarnPri, resp))) {
      return;
    }
    let old;
    for (const e of resp.data.data) {
      if (e[keyNotInTable] == id) {
        old = new Map(Object.entries(e));
        break;
      }
    }
    if (!old) {
      showNotification(
        'error',
        'not found the record to delete this is a internal bug'
      );
      return;
    }
    try {
      old.delete(keyNotInTable);
      const ret = await deleteOP(old);
      showNotification(
        'success',
        'delete OK',
        `sql:${ret.sql} row affected:${ret.changes}`
      );
    } catch (e) {
      showNotification('error', 'delete failed', e);
    }
    handleReload(false);
  };

  useEffect(() => {
    handleReload(false);
  }, [page, pageSize, triggerReload]);

  useEffect(() => {}, [orderBy]);
  useEffect(() => {
    setLastLoadTime(new Date());
  }, [resp]);

  const paginationModel = useMemo(() => {
    return { page, pageSize };
  }, [page, pageSize]);
  const onPaginationModelChange = useCallback((e) => {
    setPage(e.page);
    setPageSize(e.pageSize);
  }, []);

  const { setUpMenu } = useContext(ContextMenuContext);

  const onSortModelChange = useCallback((sorts) => {
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
  }, []);

  const onProcessRowUpdateError = useCallback((e) => {
    showNotification('error', 'update failed', e);
  }, []);
  const dataGridStyle = useMemo(() => {
    return {
      overflow: 'auto',
    } as CSSProperties;
  }, []);
  const onColumnResize = useCallback(({ colDef, width }) => {
    saveColumnWidths(colDef.field, width);
  }, []);

  const slotProps = useMemo(() => {
    return {
      row: {
        onContextMenu: (e) => {
          const { currentTarget, target } = e;
          let field = (target as HTMLDivElement).getAttribute('data-field');
          let content = (target as HTMLDivElement).innerHTML;
          let id = (currentTarget as HTMLDivElement).getAttribute('data-id');
          setUpMenu({
            content: (
              <Menu
                field={field}
                content={content}
                handleDelete={() => {
                  handleDetele(id);
                }}
                setOrderBy={(o) => {
                  setOrderBy(o);
                  handleReload(true);
                }}
              ></Menu>
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
          fullWidth
          label="where"
          variant="outlined"
          value={where}
          onChange={(event) => {
            setWhere(event.target.value);
          }}
          size="small"
        />
        <TextField
          fullWidth
          label="order by"
          variant="outlined"
          value={orderBy}
          onChange={(event) => {
            setOrderBy(event.target.value);
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
        {loading && <CircularProgress size={progressBarSize} />}
      </Stack>
      {resp && (
        <>
          <Stack
            direction="row"
            spacing={1}
            sx={{ alignItems: 'flex-end', justifyContent: 'space-between' }}
          >
            <Stack direction="row" spacing={1} sx={{ alignItems: 'flex-end' }}>
              {addOP && (
                <Button
                  size="small"
                  color="primary"
                  variant="contained"
                  onClick={() => {
                    setShowAdd(true);
                  }}
                >
                  <AddIcon></AddIcon>
                </Button>
              )}
              {resp && <Typography variant="subtitle1">{resp.sql}</Typography>}
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
                aria-labelledby="add data"
              >
                <Box sx={modalStyle} width="800px">
                  <SqlTableViewAdd
                    addOP={async (args) => {
                      try {
                        await addOP(args);
                      } catch (e) {
                        throw e;
                      }
                      handleReload(false);
                    }}
                    fields={resp.data.fields}
                  ></SqlTableViewAdd>
                </Box>
              </Modal>
            )}
          </Stack>

          {visiable && (
            <div style={{ display: 'grid' }}>
              <DataGrid
                slotProps={slotProps}
                style={dataGridStyle}
                rowHeight={datagridRowHeight}
                columnHeaderHeight={datagridHeaderHeight}
                getRowId={getRowId}
                paginationModel={paginationModel}
                pageSizeOptions={pageSizeOptions}
                paginationMode="server"
                onPaginationModelChange={onPaginationModelChange}
                checkboxSelection={false}
                rowCount={resp.total}
                sortingMode="server"
                onSortModelChange={onSortModelChange}
                columns={resp.data.fields}
                rows={resp.data.data}
                onColumnResize={onColumnResize}
                processRowUpdate={handleProcessRowUpdate}
                onProcessRowUpdateError={onProcessRowUpdateError}
              ></DataGrid>
            </div>
          )}
        </>
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
            size="small"
          ></TextField>
        );
      })}
      <Stack direction="row" spacing={1}>
        <Button size="small" variant="contained" onClick={handleAdd}>
          Add
        </Button>
        {loading && <CircularProgress size={progressBarSize} />}
      </Stack>
    </Stack>
  );
}

function Menu(props: {
  field: string;
  content: string;
  handleDelete;
  setOrderBy;
}) {
  const { field, content, handleDelete, setOrderBy } = props;
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
      </MenuList>
      <MenuItem
        onClick={() => {
          handleCopyToClipboard(field, showNotification);
        }}
      >
        <ListItemIcon>
          <ContentCopyIcon></ContentCopyIcon>
        </ListItemIcon>
        <ListItemText> copy field </ListItemText>
      </MenuItem>
      <MenuItem
        onClick={() => {
          setOrderBy(`${field} ASC`);
        }}
      >
        <ListItemText> order by {field} "ASC" </ListItemText>
      </MenuItem>
      <MenuItem
        onClick={() => {
          setOrderBy(`${field} DESC`);
        }}
      >
        <ListItemText> order by {field} "DESC" </ListItemText>
      </MenuItem>

      <MenuItem onClick={handleDelete}>
        <ListItemIcon>
          <DeleteForeverIcon></DeleteForeverIcon>
        </ListItemIcon>
        <ListItemText> delete </ListItemText>
      </MenuItem>
    </Paper>
  );
}

export { SqlTableView };
