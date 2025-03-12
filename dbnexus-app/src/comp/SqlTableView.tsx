import {
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from 'react';
import CancelIcon from '@mui/icons-material/Cancel';
import types from './ApiStruct';
import {
  Box,
  Button,
  Chip,
  Divider,
  FormControlLabel,
  ListItemIcon,
  ListItemText,
  MenuItem,
  MenuList,
  Modal,
  Paper,
  Stack,
  Switch,
  TextField,
  Tooltip,
  Typography,
} from '@mui/material';
import { NotificationContext } from './Notification';
import { DataGrid, GridSlotsComponentsProps } from '@mui/x-data-grid';
import { v4 as uuidv4 } from 'uuid';
import { keyNotInTable, modalStyle, reloadOK, SettingContext } from './Common';
import DeleteForeverIcon from '@mui/icons-material/DeleteForever';
import ContentCopyIcon from '@mui/icons-material/ContentCopy';
import KeyIcon from '@mui/icons-material/Key';
import ArrowUpwardIcon from '@mui/icons-material/ArrowUpward';
import ArrowDownwardIcon from '@mui/icons-material/ArrowDownward';
import FilterAltIcon from '@mui/icons-material/FilterAlt';
import { FaArrowsAltV } from 'react-icons/fa';
import {
  calMenuStyle,
  columnWidths,
  ContextMenuContext,
  datagridContainerHeight,
  DataSourceKind,
  filterStyle,
  generateRandomString,
  handleCopyToClipboard,
  isTypeLargeText,
  secondConfirm,
  TabContent,
  TabsSetContext,
  VisiableContext,
} from './helper';
import FileCopyIcon from '@mui/icons-material/FileCopy';
import { MenuResetColumnWidths, pageSizeOptions } from './CommonComp';

const confrimAboutPrimary = async (
  notWarnPri,
  fields: types.Field[]
): Promise<boolean> => {
  if (notWarnPri) {
    return true;
  }
  const hasPri = !!fields.find((x) => x.is_primary);
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
    pageSize: number,
    extra
  ) => Promise<types.MuiDataTableListResp>;
  updateOP?: (wheres: object, values: object) => Promise<types.SqlExecuteRet>;
  deleteOP?: (wheres: object) => Promise<types.SqlExecuteRet>;
  put_value_in_sql: (typ: string, value?: string) => Promise<string>;
  addOP?: (x: object) => Promise<void>;
  notWarnPri?: boolean;
  kind: DataSourceKind;
  tableName: string;
}) {
  let {
    reloadOP,
    updateOP,
    deleteOP,
    notWarnPri,
    put_value_in_sql,
    kind,
    tableName,
    addOP,
  } = props;
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
  const [allowFiltering, setAllowFiltering] = useState(false);

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
          <Stack direction="row" style={{ alignItems: 'center' }} spacing={1}>
            <Tooltip title={makeTitle()}>
              <strong>{def.field}</strong>
            </Tooltip>
            {def.is_primary && <KeyIcon style={{ fontSize: '16px' }}></KeyIcon>}
          </Stack>
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
      let extra = null;
      switch (kind) {
        case DataSourceKind.Cassandra:
          extra = { extra: 'cassandra', allow_filtering: allowFiltering };
          break;
      }
      let resp = await reloadOP(
        where.trim(),
        sort.trim(),
        page + 1,
        pageSize,
        extra
      );
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
    async (newRow: object, oldRow: object) => {
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
      if (!(await confrimAboutPrimary(notWarnPri, resp.data.fields))) {
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
    if (!(await confrimAboutPrimary(notWarnPri, resp.data.fields))) {
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
    handleReload(true);
  }, [page, pageSize, where, sort, allowFiltering]);

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

  const onColumnResize = useCallback(({ colDef, width }) => {
    columnWidths.saveColumnWidths(colDef.field, width);
  }, []);

  const slotProps = useMemo(() => {
    return {
      row: {
        onContextMenu: (e) => {
          const { currentTarget, target } = e;
          let field = (target as HTMLDivElement).getAttribute('data-field');
          if (!field) {
            return;
          }
          let content = (target as HTMLDivElement).innerHTML;
          let id = (currentTarget as HTMLDivElement).getAttribute('data-id');
          let row = { ...resp.data.data.find((e) => e[keyNotInTable] == id) };
          delete row[keyNotInTable];
          let nullable = resp.data.fields.find(
            (e) => e.field == field
          ).nullable;

          setUpMenu({
            content: (
              <Menu
                nullable={nullable}
                fields={resp.data.fields}
                field={field}
                content={content}
                handleDelete={() => {
                  handleDetele(id);
                }}
                row={row}
                tableName={tableName}
                addOP={addOP}
                setSort={setSort}
                setWhere={setWhere}
                put_value_in_sql={(value) => {
                  return put_value_in_sql(
                    resp.data.fields.find((e) => e.field == field).typ,
                    value
                  );
                }}
                setNull={
                  updateOP
                    ? async () => {
                        let x = resp.data.data.find(
                          (e) => e[keyNotInTable] == id
                        );
                        try {
                          //ts-ignoe
                          let values = {};
                          values[field] = null;
                          await updateOP(x, values);
                        } catch (e) {
                          showNotification('error', 'update failed', e);
                          return;
                        }
                        showNotification('success', 'set null OK');
                        handleReload(false);
                        return;
                      }
                    : undefined
                }
              ></Menu>
            ),
            style: calMenuStyle(e, 300),
          });
        },
        style: { cursor: 'context-menu' },
      },
    } as GridSlotsComponentsProps;
  }, [resp]);

  return (
    <Stack spacing={1}>
      <Stack
        direction="row"
        spacing={1}
        style={{ flexWrap: 'wrap', alignItems: 'center' }}
      >
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

        <Chip
          label={where}
          onClick={() => {
            setShowSetWhere(true);
          }}
          color="primary"
          icon={<FilterAltIcon />}
          style={{ ...filterStyle.style }}
        ></Chip>

        <Chip
          label={sort}
          onClick={() => {
            setShowSetSort(true);
          }}
          color="secondary"
          icon={<FaArrowsAltV />}
          style={{ ...filterStyle.style }}
        ></Chip>
        {kind == DataSourceKind.Cassandra && (
          <FormControlLabel
            control={
              <Switch
                size="small"
                checked={allowFiltering}
                onChange={(e) => setAllowFiltering(e.target.checked)}
              />
            }
            label="allow filtering"
          />
        )}
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
                <Typography
                  sx={{
                    cursor: 'pointer',
                    fontWeight: '800',
                    fontSize: '0.9rem',
                    fontStyle: 'italic',
                    color: 'green',
                  }}
                  variant="body1"
                  onClick={() => {
                    handleCopyToClipboard(resp.sql, showNotification);
                  }}
                >
                  {resp.sql}
                </Typography>
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
  initValues?: object;
}) {
  let { addOP, fields, initValues } = props;
  if (!initValues) {
    initValues = {};
  }
  fields.forEach((e) => {
    if (e.is_auto_increment) {
      delete initValues[e.field];
    }
  });

  const [loading, setLoading] = useState(false);
  const [args, setArgs] = useState(initValues);
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
  setNull?: () => void;
  addOP?: (x: object) => Promise<void>;
  tableName: string;
  row: object;
  fields;
  nullable: boolean;
}) {
  const {
    field,
    content,
    handleDelete,
    setSort,
    setWhere,
    put_value_in_sql,
    setNull,
    tableName,
    addOP,
    row,
    fields,
    nullable,
  } = props;
  const { showNotification } = useContext(NotificationContext);
  const [showFilter, setShowFilter] = useState(false);
  const { setTabs, setSelectTab } = useContext(TabsSetContext);
  const handleClone = () => {
    setTabs((tabs) => {
      let t = (
        <SqlTableViewAdd
          addOP={addOP}
          fields={fields}
          initValues={row}
        ></SqlTableViewAdd>
      );
      setSelectTab(tabs.length);
      return [
        ...tabs,
        new TabContent(generateRandomString(20), `${tableName}*`, t),
      ];
    });
  };
  return (
    <Paper>
      <MenuList>
        {content && (
          <MenuItem
            onClick={() => {
              handleCopyToClipboard(content, showNotification);
            }}
          >
            <ListItemIcon>
              <ContentCopyIcon></ContentCopyIcon>
            </ListItemIcon>
            <ListItemText> Copy </ListItemText>
          </MenuItem>
        )}

        <MenuItem
          onClick={() => {
            handleCopyToClipboard(field, showNotification);
          }}
        >
          <ListItemIcon>
            <ContentCopyIcon></ContentCopyIcon>
          </ListItemIcon>
          <ListItemText> Copy Field </ListItemText>
        </MenuItem>
        <Divider></Divider>
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
        <Divider></Divider>
        {content && (
          <>
            <MenuItem
              onClick={() => {
                setShowFilter(true);
              }}
            >
              <ListItemIcon>
                <FilterAltIcon></FilterAltIcon>
              </ListItemIcon>
              <ListItemText> Quick Filter </ListItemText>
            </MenuItem>
          </>
        )}
        {content && nullable && setNull && (
          <MenuItem
            onClick={() => {
              setNull();
            }}
          >
            <ListItemIcon>
              <CancelIcon></CancelIcon>
            </ListItemIcon>
            <ListItemText> Set as Null </ListItemText>
          </MenuItem>
        )}
        {addOP && (
          <>
            <Divider></Divider>
            <MenuItem onClick={handleClone}>
              <ListItemIcon>
                <FileCopyIcon></FileCopyIcon>
              </ListItemIcon>
              <ListItemText> Clone Row </ListItemText>
            </MenuItem>
            <MenuItem onClick={handleDelete}>
              <ListItemIcon>
                <DeleteForeverIcon></DeleteForeverIcon>
              </ListItemIcon>
              <ListItemText> Delete Row</ListItemText>
            </MenuItem>
          </>
        )}
        <MenuResetColumnWidths></MenuResetColumnWidths>
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
