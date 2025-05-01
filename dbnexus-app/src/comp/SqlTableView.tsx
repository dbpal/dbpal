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
import EditIcon from '@mui/icons-material/Edit';
import { NotificationContext } from './Notification';
import { IoIosAddCircle } from 'react-icons/io';
import { DataGrid, GridSlotsComponentsProps } from '@mui/x-data-grid';
import { v4 as uuidv4 } from 'uuid';
import {
  keyNotInTable,
  modalStyle,
  reloadOK,
  SettingContext,
  SimpleResizeEditor,
} from './Common';
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
import { CancellationToken, editor, languages, Position } from 'monaco-editor';
import { loader } from '@monaco-editor/react';

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
const whereLanguage = 'ckqQtTcQDmO3e2fiBMB4QOv6Mhalfk';
let whereAtuoCompletion: (
  sql: string,
  pos: Position
) => Promise<languages.CompletionList> = null;

loader.init().then((monaco) => {
  monaco.languages.register({ id: whereLanguage });
  monaco.languages.setMonarchTokensProvider(whereLanguage, {
    ignoreCase: true,
    tokenizer: {
      root: [[/\'(?:\\.|[^\\\'])*\'/, 'string']],
    },
  });

  monaco.languages.registerCompletionItemProvider(whereLanguage, {
    provideCompletionItems: (
      model: editor.ITextModel,
      position: Position,
      _context: any,
      _token: CancellationToken
    ) => {
      return whereAtuoCompletion(model.getValue(), position);
    },
  });
});

function WhereSort(props: {
  open: boolean;
  onClose;
  value: string;
  setValue;
  label;
  fields: types.Field[];
  autoCompletion: (
    sql: string,
    pos: Position
  ) => Promise<languages.CompletionList>;
}) {
  const { open, onClose, value, setValue, label, fields, autoCompletion } =
    props;
  const [newValue, setNewValue] = useState(value);

  whereAtuoCompletion = async (sql, p) => {
    return autoCompletion(sql, p).then((list) => {
      fields.forEach((e) => {
        list.suggestions.push({
          label: e.field,
          kind: languages.CompletionItemKind.Field,
          insertText: e.field,
          range: undefined,
          detail: e.typ,
          documentation: e.typ,
        });
      });
      return list;
    });
  };

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
      <Box sx={modalStyle} width="75%">
        <Stack spacing={1}>
          <SimpleResizeEditor
            value={newValue}
            onMount={(e) => {
              e.onDidChangeModelContent(() => {
                setNewValue(e.getValue());
              });
            }}
            language={whereLanguage}
          ></SimpleResizeEditor>

          <Stack direction="row" spacing={1}>
            <Button
              variant="contained"
              onClick={() => {
                setValue(newValue);
                onClose();
              }}
            >
              set {`'${label}'`}
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

function EditInModal(props: { content: string; handleModify }) {
  const { content, handleModify } = props;
  const languages = [
    { language: 'text', color: 'info' },
    { language: 'html', color: 'primary' },
    { language: 'json', color: 'secondary' },
  ] as {
    language: string;
    color: string;
  }[];
  const [language, setLanguage] = useState(languages[0].language);
  const [value, setValue] = useState(content);
  return (
    <Stack spacing={1}>
      <Stack direction="row" spacing={1}>
        {languages.map((e) => (
          <Chip
            key={e.language}
            label={e.language}
            //@ts-ignore
            color={e.color}
            size="small"
            disabled={language == e.language}
            onClick={() => {
              setLanguage(e.language);
            }}
          ></Chip>
        ))}
      </Stack>
      <SimpleResizeEditor
        value={content}
        language={language}
        onMount={(editor, _) => {
          editor.onDidChangeModelContent(() => {
            setValue(editor.getValue());
          });
        }}
        minHeigth={350}
      ></SimpleResizeEditor>
      <Divider></Divider>
      <Stack spacing={1} direction="row">
        <Button
          onClick={() => {
            handleModify(value);
          }}
          variant="contained"
        >
          modify
        </Button>
      </Stack>
    </Stack>
  );
}

class EditInModalData {
  constructor(
    public row: object,
    public field: string,
    public content: string
  ) {}
}

function SqlTableView(props: {
  reloadOP: (
    where: string,
    orderBy: string,
    page: number,
    pageSize: number,
    extra
  ) => Promise<types.MuiDataGridData>;
  updateOP?: (wheres: object, values: object) => Promise<types.SqlExecuteRet>;
  deleteOP?: (wheres: object) => Promise<types.SqlExecuteRet>;
  put_value_in_sql: (typ: string, value?: string) => Promise<string>;
  addOP?: (x: object) => Promise<void>;
  notWarnPri?: boolean;
  kind: DataSourceKind;
  tableName: string;
  autoCompletion?: (
    sql: string,
    pos: Position
  ) => Promise<languages.CompletionList>;
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
    autoCompletion,
  } = props;
  const [loading, setLoading] = useState(false);
  const [where, setWhere] = useState('');
  const [showSetWhere, setShowSetWhere] = useState(false);
  const [sort, setSort] = useState('');
  const [showSetSort, setShowSetSort] = useState(false);
  const [page, setPage] = useState(0);
  const setting = useContext(SettingContext);
  const [pageSize, setPageSize] = useState(setting.pageSize);
  const [resp, setResp] = useState<types.MuiDataGridData | null>(null);
  const { showNotification } = useContext(NotificationContext);
  const [lastLoadTime, setLastLoadTime] = useState(new Date());
  const visiable = useContext(VisiableContext);
  const [allowFiltering, setAllowFiltering] = useState(false);
  const [editInModal, setEditInModal] = useState<EditInModalData>(null);

  const handleResp = (resp: types.MuiDataGridData) => {
    resp.data.forEach((data) => {
      data[keyNotInTable] = uuidv4();
    });

    for (let i = 0; i < resp.fields.length; i++) {
      let field = resp.fields[i];
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
      await handleResp(resp);
      resp.fields.forEach((e) => {
        const width = columnWidths.getWidth(e.field);
        if (width) {
          e['width'] = width;
        }
      });
      setLoading(false);
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
      if (!(await confrimAboutPrimary(notWarnPri, resp.fields))) {
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
        showNotification('error', 'update failed', e);
        throw e;
      }
      return newRow;
    },
    [resp]
  );

  const handleDetele = async (id) => {
    if (!(await confrimAboutPrimary(notWarnPri, resp.fields))) {
      return;
    }
    let old;
    for (const e of resp.data) {
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
  }, [page, pageSize, where, sort, allowFiltering]);

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
          let row = { ...resp.data.find((e) => e[keyNotInTable] == id) };
          delete row[keyNotInTable];
          let nullable = resp.fields.find((e) => e.field == field).nullable;

          setUpMenu({
            content: (
              <Menu
                editInModal={() => {
                  setEditInModal(new EditInModalData(row, field, row[field]));
                }}
                nullable={nullable}
                fields={resp.fields}
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
                    resp.fields.find((e) => e.field == field).typ,
                    value
                  );
                }}
                setNull={
                  updateOP
                    ? async () => {
                        let x = resp.data.find((e) => e[keyNotInTable] == id);
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
        {/* <Typography color="info"> select * from {tableName} where </Typography> */}
        <Chip
          label={where}
          onClick={() => {
            setShowSetWhere(true);
          }}
          color="primary"
          icon={<FilterAltIcon />}
          style={{ ...filterStyle.style }}
        ></Chip>
        {/* <Typography color="info"> order by </Typography> */}
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
            sx={{ alignItems: 'center', justifyContent: 'space-between' }}
          >
            <Stack direction="row" spacing={1} sx={{ alignItems: 'center' }}>
              {resp && (
                <Typography
                  sx={{
                    cursor: 'pointer',

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
                height: datagridContainerHeight(resp.data.length),
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
                columns={resp.fields}
                rows={resp.data}
                onColumnResize={onColumnResize}
                processRowUpdate={handleProcessRowUpdate}
                onProcessRowUpdateError={onProcessRowUpdateError}
              ></DataGrid>
            </div>
          )}
        </>
      )}

      {editInModal && (
        <Modal
          open={!!editInModal}
          onClose={(_, reason) => {
            if (reason === 'backdropClick') {
              return;
            }
            setEditInModal(null);
          }}
        >
          <Box sx={modalStyle} width="80%">
            <EditInModal
              content={editInModal.content}
              handleModify={async (value) => {
                // clone
                let newRow = { ...editInModal.row };
                // modify
                newRow[editInModal.field] = value;
                await handleProcessRowUpdate(newRow, editInModal.row);
                handleReload(false);
                setEditInModal(null);
              }}
            ></EditInModal>
          </Box>
        </Modal>
      )}

      {showSetWhere && (
        <WhereSort
          open={showSetWhere}
          value={where}
          setValue={(v) => {
            setWhere(v);
          }}
          autoCompletion={autoCompletion}
          onClose={() => {
            setShowSetWhere(false);
          }}
          label="filter"
          fields={resp.fields}
        ></WhereSort>
      )}
      {showSetSort && (
        <WhereSort
          autoCompletion={autoCompletion}
          open={showSetSort}
          value={sort}
          setValue={(v) => {
            setSort(v);
          }}
          fields={resp.fields}
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
            sx={{ alignContent: 'center', justifyContent: 'center' }}
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
  editInModal;
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
    editInModal,
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
        new TabContent(
          generateRandomString(20),
          `${tableName}*`,
          t,
          <IoIosAddCircle> </IoIosAddCircle>
        ),
      ];
    });
  };
  return (
    <Paper>
      <MenuList>
        {addOP && (
          <MenuItem
            onClick={() => {
              editInModal();
            }}
          >
            <ListItemIcon>
              <EditIcon></EditIcon>
            </ListItemIcon>
            <ListItemText> Edit in Modal </ListItemText>
          </MenuItem>
        )}

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

        {content && (
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
        )}
        <Divider></Divider>

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
          <MenuItem onClick={handleClone}>
            <ListItemIcon>
              <FileCopyIcon></FileCopyIcon>
            </ListItemIcon>
            <ListItemText> Clone Row </ListItemText>
          </MenuItem>
        )}
        {addOP && (
          <MenuItem onClick={handleDelete}>
            <ListItemIcon>
              <DeleteForeverIcon></DeleteForeverIcon>
            </ListItemIcon>
            <ListItemText> Delete Row</ListItemText>
          </MenuItem>
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
          <Box sx={modalStyle} width="50%">
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
