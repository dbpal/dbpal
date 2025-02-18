import {
  CSSProperties,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from 'react';

import types from './ApiStruct';
import {
  Box,
  Button,
  Divider,
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
  keyNotInTable,
  modalStyle,
  reloadOK,
  SettingContext,
  VisiableContext,
} from './Common';
import DeleteForeverIcon from '@mui/icons-material/DeleteForever';
import ContentCopyIcon from '@mui/icons-material/ContentCopy';
import KeyIcon from '@mui/icons-material/Key';
import ArrowUpwardIcon from '@mui/icons-material/ArrowUpward';
import ArrowDownwardIcon from '@mui/icons-material/ArrowDownward';
import FilterAltIcon from '@mui/icons-material/FilterAlt';
import {
  calMenuStyle,
  columnWidths,
  ContextMenuContext,
  datagridContainerHeight,
  filterStyle,
  handleCopyToClipboard,
  isTypeLargeText,
  secondConfirm,
} from './helper';
import { MenuResetColumnWidths, pageSizeOptions } from './CommonComp';

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

function WhereSort(props: {
  open: boolean;
  onClose;
  value: string;
  setValue;
  label;
}) {
  const { open, onClose, value, setValue, label } = props;
  const [newValue, setNewValue] = useState(value);

  return (
    <Modal
      open={open}
      onClose={(_, reason) => {
        if (reason === 'backdropClick') {
          return;
        }
        onClose();
      }}
      aria-labelledby="create database"
    >
      <Box sx={modalStyle} width="700px">
        <Stack spacing={1}>
          <TextField
            fullWidth
            value={newValue}
            onChange={(e) => {
              setNewValue(e.target.value);
            }}
            label={label}
            size="small"
          ></TextField>

          <Stack direction="row" spacing={1}>
            <Button
              variant="contained"
              onClick={() => {
                setValue(newValue);
                onClose();
              }}
            >
              set
            </Button>
            {newValue && (
              <Button
                variant="contained"
                onClick={() => {
                  setNewValue('');
                }}
              >
                clear
              </Button>
            )}
          </Stack>
        </Stack>
      </Box>
    </Modal>
  );
}

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
  put_value_in_sql: (typ: string, value?: string) => Promise<string>;
  notWarnPri?: boolean;
}) {
  let { reloadOP, updateOP, deleteOP, notWarnPri, put_value_in_sql } = props;

  const [loading, setLoading] = useState(false);
  const [where, setWhere] = useState('');
  const [showSetWhere, setShowSetWhere] = useState(false);

  const [sort, setSort] = useState('');
  const [showSetSort, setShowSetSort] = useState(false);
  const [page, setPage] = useState(0);
  const setting = useContext(SettingContext);
  const [pageSize, setPageSize] = useState(setting.pageSize);

  const [resp, setResp] = useState<types.MuiDataTableListResp | null>(null);
  const { showNotification } = useContext(NotificationContext);

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
      let resp = await reloadOP(where.trim(), sort.trim(), page + 1, pageSize);
      setLoading(false);
      await handleResp(resp);
      resp.data.fields.forEach((e) => {
        const width = columnWidths.getWidth(e.field);
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
  }, [page, pageSize, where, sort]);

  useEffect(() => {}, [sort]);
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
      setSort('');
      return;
    }
    let sort = sorts[0];
    if (!sort.sort) {
      setSort('');
      return;
    }
    setSort(`${sort.field} ${sort.sort}`);
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
    columnWidths.saveColumnWidths(colDef.field, width);
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
                setSort={setSort}
                setWhere={setWhere}
                put_value_in_sql={(value) => {
                  return put_value_in_sql(
                    resp.data.fields.find((e) => e.field == field).typ,
                    value
                  );
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
      <Stack direction="row" spacing={1} style={{ flexWrap: 'wrap' }}>
        <span
          style={{
            ...filterStyle.style,
            backgroundColor: filterStyle.filter,
          }}
          onClick={() => {
            setShowSetWhere(true);
          }}
        >
          <span> filter:&nbsp;</span>
          <span style={{ ...filterStyle.on }}>{where}</span>
        </span>

        <span
          style={{
            ...filterStyle.style,
            backgroundColor: filterStyle.sort,
          }}
          onClick={() => {
            setShowSetSort(true);
          }}
        >
          <span> sort: </span>
          <span style={{ ...filterStyle.on }}>{sort}</span>
        </span>

        <Button
          loading={loading}
          variant="contained"
          color="primary"
          onClick={() => {
            handleReload(true);
          }}
        >
          reload
        </Button>
      </Stack>
      {resp && (
        <>
          <Stack
            direction="row"
            spacing={1}
            sx={{ alignItems: 'flex-end', justifyContent: 'space-between' }}
          >
            <Stack direction="row" spacing={1} sx={{ alignItems: 'flex-end' }}>
              {resp && (
                <span
                  style={{ cursor: 'pointer' }}
                  onClick={() => {
                    handleCopyToClipboard(resp.sql, showNotification);
                  }}
                >
                  {resp.sql}
                </span>
              )}
            </Stack>
            {resp && (
              <Typography>loaded at:{lastLoadTime.toLocaleString()}</Typography>
            )}
          </Stack>

          {visiable && (
            <div
              style={{
                display: 'grid',
                height: datagridContainerHeight(resp.data.data.length),
              }}
            >
              <DataGrid
                loading={loading}
                showCellVerticalBorder
                showColumnVerticalBorder
                slotProps={slotProps}
                style={dataGridStyle}
                rowHeight={setting.datagridRowHeight}
                columnHeaderHeight={setting.datagridHeaderHeight}
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
      {showSetWhere && (
        <WhereSort
          open={showSetWhere}
          value={where}
          setValue={(v) => {
            setWhere(v);
          }}
          onClose={() => {
            setShowSetWhere(false);
          }}
          label="filter"
        ></WhereSort>
      )}
      {showSetSort && (
        <WhereSort
          open={showSetSort}
          value={sort}
          setValue={(v) => {
            setSort(v);
          }}
          onClose={() => {
            setShowSetSort(false);
          }}
          label="sort"
        ></WhereSort>
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
  const inputDoms = useRef([]);
  const handleAdd = () => {
    for (let i = 0; i < fields.length; i++) {
      let f = fields[i];
      if (f.is_auto_increment) {
        continue;
      }
      if (!args[f.field] && !f.nullable) {
        inputDoms.current[i].focus();
        showNotification('error', `filed '${f.field}' need be filled`);
        return;
      }
    }
    setLoading(true);
    addOP(args)
      .then(() => {
        showNotification('success', 'Add OK');
      })
      .catch((e) => {
        showNotification('error', 'Add failure', e);
      })
      .finally(() => {
        setLoading(false);
      });
  };

  return (
    <Stack spacing={1}>
      {fields.map((f, index) => {
        if (f.is_auto_increment) {
          return;
        }
        return (
          <Stack
            key={f.field}
            direction="row"
            spacing={1}
            sx={{ alignContent: 'flex-end', justifyContent: 'flex-end' }}
          >
            <TextField
              inputRef={(x) => {
                inputDoms.current[index] = x;
              }}
              size="small"
              value={args[f.field]}
              helperText={f.typ + ' ' + (f.nullable ? 'null' : 'not null')}
              required={!f.nullable}
              label={f.field}
              multiline={isTypeLargeText(f.typ)}
              rows={4}
              fullWidth
              onChange={(e) => {
                const value = e.target.value;
                const v = value ? value : null;
                let newArgs = { ...args };
                newArgs[f.field] = v;
                setArgs(newArgs);
              }}
            ></TextField>
          </Stack>
        );
      })}
      <Divider></Divider>
      <Stack direction="row" spacing={1}>
        <Button loading={loading} variant="contained" onClick={handleAdd}>
          add
        </Button>
      </Stack>
    </Stack>
  );
}

function Menu(props: {
  field: string;
  content: string;
  handleDelete;
  setSort;
  setWhere;
  put_value_in_sql: (value?: string) => Promise<string>;
}) {
  const { field, content, handleDelete, setSort, setWhere, put_value_in_sql } =
    props;
  const { showNotification } = useContext(NotificationContext);
  const [showFilter, setShowFilter] = useState(false);
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
            setSort(`${field} ASC`);
          }}
        >
          <ListItemIcon>
            <ArrowUpwardIcon></ArrowUpwardIcon>
          </ListItemIcon>
          <ListItemText> {field} ASC </ListItemText>
        </MenuItem>
        <MenuItem
          onClick={() => {
            setSort(`${field} DESC`);
          }}
        >
          <ListItemIcon>
            <ArrowDownwardIcon></ArrowDownwardIcon>
          </ListItemIcon>
          <ListItemText> {field} DESC </ListItemText>
        </MenuItem>

        <MenuItem
          onClick={() => {
            setShowFilter(true);
          }}
        >
          <ListItemIcon>
            <FilterAltIcon></FilterAltIcon>
          </ListItemIcon>
          <ListItemText> filter </ListItemText>
        </MenuItem>
        <MenuResetColumnWidths></MenuResetColumnWidths>
        <MenuItem onClick={handleDelete}>
          <ListItemIcon>
            <DeleteForeverIcon></DeleteForeverIcon>
          </ListItemIcon>
          <ListItemText> delete row</ListItemText>
        </MenuItem>
      </MenuList>

      {showFilter && (
        <Modal
          open={showFilter}
          onClose={(_, reason) => {
            if (reason === 'backdropClick') {
              return;
            }
            setShowFilter(false);
          }}
          aria-labelledby="create database"
        >
          <Box sx={modalStyle} width="700px">
            <FilterOnValue
              content={content}
              setWhere={setWhere}
              field={field}
              handleClose={() => {
                setShowFilter(false);
              }}
              put_value_in_sql={(value) => {
                return put_value_in_sql(value);
              }}
            ></FilterOnValue>
          </Box>
        </Modal>
      )}
    </Paper>
  );
}

function FilterOnValue(props: {
  content: string;
  setWhere;
  field: string;
  handleClose;
  put_value_in_sql: (x: string) => Promise<string>;
}) {
  const { content, setWhere, field, handleClose, put_value_in_sql } = props;
  const compares = ['=', '<>', '>', '>=', '<', '<='];
  const [compare, setCompare] = useState(compares[0]);
  const { showNotification } = useContext(NotificationContext);
  const [formatValue, setFormatValue] = useState('');
  useEffect(() => {
    put_value_in_sql(content)
      .then((value) => {
        setFormatValue(value);
      })
      .catch((e) => {
        showNotification('error', 'put value in sql failed', e);
      });
  }, []);

  const where = () => {
    return `${field} ${compare} ${formatValue}`;
  };
  return (
    <Stack spacing={1}>
      <Stack direction="row" spacing={1}>
        {compares.map((x) => (
          <Button
            key={x}
            onClick={() => {
              setCompare(x);
            }}
          >
            {x}
          </Button>
        ))}
      </Stack>

      <Typography>
        on: <em> {where()} </em>
      </Typography>
      <Divider></Divider>
      <Stack direction="row">
        <Button
          variant="contained"
          onClick={() => {
            setWhere(where());
            handleClose();
          }}
        >
          set
        </Button>
      </Stack>
    </Stack>
  );
}
export { SqlTableView, SqlTableViewAdd };
