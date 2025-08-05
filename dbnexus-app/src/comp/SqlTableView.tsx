import {
  CSSProperties,
  memo,
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
import {
  DataGrid,
  GridSlotsComponentsProps,
  GridSortModel,
} from '@mui/x-data-grid';
import { v4 as uuidv4 } from 'uuid';
import {
  EscDownEvent,
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
  DataSourceKind,
  filterStyle,
  generateRandomString,
  handleCopyToClipboard,
  isTypeLargeText,
  resetMuiDataRespWidth,
  secondConfirm,
  TabContent,
  TabsSetContext,
  valid_json,
  VisiableContext,
} from './helper';
import { FaCopy } from 'react-icons/fa6';
import FileCopyIcon from '@mui/icons-material/FileCopy';
import { MenuResetColumnWidths, pageSizeOptions } from './CommonComp';
import { CancellationToken, editor, languages, Position } from 'monaco-editor';
import { loader } from '@monaco-editor/react';
import wcmatch from 'wildcard-match';
import { MdEditNote } from 'react-icons/md';
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
let whereAutoCompletion: (
  sql: string,
  pos: Position
) => Promise<languages.CompletionList> = null;

loader
  .init()
  .then((monaco) => {
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
        return whereAutoCompletion(model.getValue(), position);
      },
    });
  })
  .catch((e) => {
    console.log('init monaco language error:', e);
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

  whereAutoCompletion = async (sql, p) => {
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
      onClose={(e: EscDownEvent, reason) => {
        if (reason === 'backdropClick') {
          return;
        }
        e.preventDefault();
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

var EditInModalLanguage = 'text';
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

  const [jsonErr, setJsonErr] = useState<string | null>(null);

  const [language, setLanguage] = useState(EditInModalLanguage);
  const [value, setValue] = useState(content);

  useEffect(() => {
    if (language != 'json') {
      setJsonErr(null);
      return;
    }
    valid_json(value)
      .then(() => {
        setJsonErr(null);
      })
      .catch((e) => {
        setJsonErr(e);
      });
  }, [value, language]);
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
              EditInModalLanguage = e.language;
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
        barLeft={
          jsonErr ? (
            <Typography color="error" variant="body1">
              {jsonErr}
            </Typography>
          ) : undefined
        }
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

class EditInModalCellData {
  constructor(
    public row: object,
    public field: string,
    public content: string
  ) {}
}

function SqlTableView_(props: {
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
  const [editInModal, setEditInModal] = useState<EditInModalCellData>(null);
  const [editInModalRow, setEditInModalRow] = useState<object>(null);

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
          let ret = f.field + ' ';
          ret += f.typ;
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
      resp.fields.forEach((field) => {
        const width = columnWidths.getWidth(field.field);
        if (width) {
          field['width'] = width;
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
        handleReload(false);
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

  const onSortModelChange = useCallback((sorts: GridSortModel) => {
    if (sorts.length == 0) {
      setSort('');
      return;
    }
    let sort = sorts[0];
    if (!sort.sort) {
      setSort('');
      return;
    }
    setSort(`${sort.field} ${sort.sort.toUpperCase()}`);
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
                editCellInModal={() => {
                  setEditInModal(
                    new EditInModalCellData(row, field, row[field])
                  );
                }}
                editRowInModal={() => {
                  setEditInModalRow(row);
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
                resetColunmWidthCallBack={() => {
                  setResp((old) => {
                    return resetMuiDataRespWidth(old);
                  });
                }}
              ></Menu>
            ),
            style: calMenuStyle(e, 300),
          });
        },

        style: { cursor: 'context-menu' },
      },
    } as GridSlotsComponentsProps;
  }, [resp]);
  const gridStyle = useMemo(() => {
    return { height: 'auto', maxHeight: '78vh' } as CSSProperties;
  }, []);
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

        <Tooltip title="filter something like 'age=18'">
          <Chip
            label={where}
            onClick={() => {
              setShowSetWhere(true);
            }}
            color="primary"
            icon={<FilterAltIcon />}
            style={{ ...filterStyle.style }}
          ></Chip>
        </Tooltip>
        {/* <Typography color="info"> order by </Typography> */}
        <Tooltip title="sort something like 'age asc'">
          <Chip
            label={sort}
            onClick={() => {
              setShowSetSort(true);
            }}
            color="secondary"
            icon={<FaArrowsAltV />}
            style={{ ...filterStyle.style }}
          ></Chip>
        </Tooltip>
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
                    fontWeight: '600',
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
              }}
            >
              <DataGrid
                style={gridStyle}
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
          onClose={(e: EscDownEvent, reason) => {
            if (reason === 'backdropClick') {
              return;
            }
            e.preventDefault();
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

      {editInModalRow && (
        <Modal
          open={!!editInModalRow}
          onClose={(e: EscDownEvent, reason) => {
            if (reason === 'backdropClick') {
              return;
            }
            e.preventDefault();
            setEditInModalRow(null);
          }}
        >
          <Box sx={modalStyle} width="80%">
            <SqlTableViewData
              initValues={editInModalRow}
              buttonName="modify"
              fields={resp.fields}
              handleData={async (args) => {
                await handleProcessRowUpdate(args, editInModalRow);
                setEditInModalRow(null);
                handleReload(false);
              }}
            ></SqlTableViewData>
          </Box>
        </Modal>
      )}
      {editInModal && (
        <Modal
          open={!!editInModal}
          onClose={(e: EscDownEvent, reason) => {
            if (reason === 'backdropClick') {
              return;
            }

            e.preventDefault();
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

let SqlTableViewDataMatch = '';

function SqlTableViewData(props: {
  handleData: (args: any) => Promise<void>;
  fields: types.Field[];
  initValues?: object;
  buttonName;
}) {
  let { handleData, fields, initValues, buttonName } = props;
  if (!initValues) {
    initValues = {};
  }
  fields.forEach((e) => {
    if (e.is_auto_increment) {
      delete initValues[e.field];
    }
  });
  const [filter, setFilter] = useState(SqlTableViewDataMatch);

  const filterFn = useMemo(() => {
    SqlTableViewDataMatch = filter;
    return filter
      ? wcmatch('*' + filter + '*', { flags: 'i' })
      : () => {
          return true;
        };
  }, [filter]);

  const [loading, setLoading] = useState(false);
  const [args, setArgs] = useState(initValues);
  const { showNotification } = useContext(NotificationContext);
  const inputDoms = useRef({});
  const handleModify = () => {
    for (let i = 0; i < fields.length; i++) {
      let f = fields[i];
      if (f.is_auto_increment) {
        continue;
      }
      if (!args[f.field] && !f.nullable) {
        inputDoms.current[f.field].focus();
        showNotification('error', `filed '${f.field}' need be filled`);
        return;
      }
    }
    setLoading(true);
    handleData(args)
      .then(() => {
        showNotification('success', `${buttonName} OK`);
      })
      .catch((e) => {
        showNotification('error', `${buttonName} failure`, e);
      })
      .finally(() => {
        setLoading(false);
      });
  };

  return (
    <Stack spacing={1}>
      {fields
        .filter(
          (f) =>
            filterFn(f.field) ||
            (args[f.field] ? filterFn(args[f.field]) : !filter)
        )
        .map((f) => {
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
                  inputDoms.current[f.field] = x;
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
      <Stack direction="row" spacing={1} style={{ alignItems: 'flex-start' }}>
        <Button loading={loading} variant="contained" onClick={handleModify}>
          {buttonName}
        </Button>

        <TextField
          size="small"
          value={filter}
          onChange={(e) => {
            setFilter(e.target.value);
          }}
          label="filter"
          fullWidth
          helperText="can match field name and content"
        ></TextField>
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
  editCellInModal;
  editRowInModal;
  resetColunmWidthCallBack;
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
    editCellInModal,
    editRowInModal,
    resetColunmWidthCallBack,
  } = props;
  const { showNotification } = useContext(NotificationContext);
  const [showFilter, setShowFilter] = useState(false);
  const { setTabs, setSelectTab } = useContext(TabsSetContext);
  const handleClone = () => {
    setTabs((tabs) => {
      let t = (
        <SqlTableViewData
          buttonName="add"
          handleData={addOP}
          fields={fields}
          initValues={row}
        ></SqlTableViewData>
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
              editCellInModal();
            }}
          >
            <ListItemIcon>
              <EditIcon></EditIcon>
            </ListItemIcon>
            <ListItemText> Edit Cell in Modal </ListItemText>
          </MenuItem>
        )}

        {addOP && (
          <MenuItem
            onClick={() => {
              editRowInModal();
            }}
          >
            <ListItemIcon>
              <MdEditNote fontSize="1.7rem"></MdEditNote>
            </ListItemIcon>
            <ListItemText> Edit Row in Modal </ListItemText>
          </MenuItem>
        )}

        {nullable && setNull && (
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

        {content && (
          <MenuItem
            onClick={() => {
              handleCopyToClipboard(content, showNotification);
            }}
          >
            <ListItemIcon>
              <FaCopy fontSize="1.4rem"></FaCopy>
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
        <MenuResetColumnWidths
          reset={resetColunmWidthCallBack}
        ></MenuResetColumnWidths>
      </MenuList>

      {showFilter && (
        <Modal
          open={showFilter}
          onClose={(e: EscDownEvent, reason) => {
            if (reason === 'backdropClick') {
              return;
            }

            e.preventDefault();
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
    if (compare == 'null') {
      return `${field} IS NULL`;
    }
    if (compare == 'not null') {
      return `${field} IS NOT NULL`;
    }

    return `${field} ${compare} ${formatValue}`;
  };
  return (
    <Stack spacing={2}>
      <Stack direction="row" spacing={1}>
        {compares.map((x) => (
          <Chip
            key={x}
            onClick={() => {
              setCompare(x);
            }}
            label={x}
            color="primary"
            size="small"
            disabled={compare == x}
          ></Chip>
        ))}
      </Stack>
      <Stack direction="row" spacing={1}>
        {['null', 'not null'].map((e) => (
          <Chip
            label={e}
            disabled={compare == e}
            onClick={() => {
              setCompare(e);
            }}
            size="small"
            color="primary"
          />
        ))}
      </Stack>

      <Divider></Divider>
      <Stack direction="row" spacing={1} style={{ alignItems: 'center' }}>
        <Button
          variant="contained"
          onClick={() => {
            setWhere(where());
            handleClose();
          }}
        >
          set
        </Button>

        <Typography>
          on: <em> {where()} </em>
        </Typography>
      </Stack>
    </Stack>
  );
}

const SqlTableView = memo(SqlTableView_);

export { SqlTableView, SqlTableViewData };
