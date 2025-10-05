'use strict';
import {
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
  IconButton,
  InputAdornment,
  ListItemIcon,
  ListItemText,
  MenuItem,
  MenuList,
  Modal,
  Stack,
  Switch,
  TextField,
  Tooltip,
  Typography,
} from '@mui/material';
import VisibilityIcon from '@mui/icons-material/Visibility';
import EditIcon from '@mui/icons-material/Edit';
import { NotificationContext } from './Notification';
import { IoIosAddCircle } from 'react-icons/io';
import {
  DataGrid,
  GridCallbackDetails,
  GridPaginationModel,
  GridSlotsComponent,
  GridSlotsComponentsProps,
  GridSortModel,
} from '@mui/x-data-grid';
import ClearIcon from '@mui/icons-material/Clear';
import {
  EscDownEvent,
  keyNotInTable,
  makeFormatFn,
  modalHandleClose,
  modalStyle,
  NameAndQueryId,
  reloadOK,
  SetAutoCompletionName,
} from './Common';
import DeleteForeverIcon from '@mui/icons-material/DeleteForever';
import KeyIcon from '@mui/icons-material/Key';
import ArrowUpwardIcon from '@mui/icons-material/ArrowUpward';
import ArrowDownwardIcon from '@mui/icons-material/ArrowDownward';
import FilterAltIcon from '@mui/icons-material/FilterAlt';
import { FaArrowsAltV } from 'react-icons/fa';
import {
  calMenuStyle,
  ColumnWidth,
  ContextMenuContext,
  DataSourceKind,
  generateRandomString,
  gridStyle,
  handleCopyToClipboard,
  isTypeLargeText,
  ErrAndLoc,
  resetMuiDataRespWidth,
  saveAndSetSize,
  secondConfirm,
  SettingContext,
  TabContent,
  TabsSetContext,
  validJson,
  SavePageSize,
  textEllipsisStyle,
  quoteIdentifier,
  MuiDataGridData,
  extractFieldAndIDFromMouseEvent,
} from './helper';
import { FaCopy } from 'react-icons/fa6';
import FileCopyIcon from '@mui/icons-material/FileCopy';
import {
  AppColumnMenu,
  ExecutingTime,
  ExportMarkDownTable,
  MenuExportMarkDownTable,
  MenuResetColumnWidths,
  ModalHeader,
  pageSizeOptions,
  ViewRowInModal,
} from './CommonComp';
import { editor, Position } from 'monaco-editor';
import wcmatch from 'wildcard-match';
import { MdEditNote } from 'react-icons/md';
import { SimpleFlexEditor } from './CommonComp2';

const confirmAboutPrimary = async (
  notWarnPri: boolean,
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

function SetFilterSort(props: {
  open: boolean;
  handleClose;
  value: string;
  setValue;
  label: 'filter' | 'sort';
  tableName;
  kind: DataSourceKind;
  uuid: string;
  extractFn: (sql: string) => Promise<String | null>;
}) {
  let {
    open,
    handleClose,
    value,
    setValue,
    label,
    tableName,
    kind,
    extractFn,
    uuid,
  } = props;
  tableName = quoteIdentifier(kind)(tableName);

  const editor = useRef<editor.IStandaloneCodeEditor>(null);
  const [modalBox, setModalBox] = useState(null);
  const { showNotification } = useContext(NotificationContext);
  useEffect(() => {
    if (modalBox) {
      let c = saveAndSetSize(modalBox, 'where-sort');
      return c;
    }
  }, [modalBox]);

  const handleSet = async () => {
    try {
      let value = await extractFn(editor.current.getValue());
      if (value == null) {
        value = '';
      }
      setValue(value);
      handleClose();
    } catch (e) {
      showNotification('error', 'extract sql error.', e);
      return;
    }
  };
  return (
    <Modal
      open={open}
      onClose={modalHandleClose((e: EscDownEvent) => {
        e.preventDefault();
        handleClose();
      })}
    >
      <Box
        sx={{ resize: 'both', ...modalStyle }}
        width="75%"
        height="30vh"
        ref={setModalBox}
      >
        <ModalHeader title={`set ${label}`}></ModalHeader>
        <SimpleFlexEditor
          value={makeFormatFn(kind, showNotification)(
            label == 'filter'
              ? `select * from ${tableName} where ${value}`
              : `select * from ${tableName} order by ${value}`,
            { tabSize: 4, insertSpaces: true }
          )}
          handleEditor={(e) => {
            editor.current = e;
            SetAutoCompletionName.set(
              e.getModel().id,
              new NameAndQueryId(uuid)
            );
          }}
          language={kind}
        ></SimpleFlexEditor>

        <Stack direction="row" spacing={1} style={{ alignItems: 'center' }}>
          <Button onClick={handleSet}>set {label}</Button>
          <Button
            onClick={() => {
              editor.current.setValue(`select * from ${tableName}`);
            }}
            variant="outlined"
          >
            clear
          </Button>
        </Stack>
      </Box>
    </Modal>
  );
}

let EditInModalLanguage = 'text';
const editInModalLanguages = [
  { language: 'text' },
  { language: 'html' },
  { language: 'json' },
  { language: 'xml' },
] as {
  language: string;
}[];

function EditInModal(props: { content: string; handleModify }) {
  const { content, handleModify } = props;
  const [jsonErr, setJsonErr] = useState<ErrAndLoc>(null);
  const [jsonErrPos, setJsonErrPos] = useState<Position>(null);
  const [language, setLanguage] = useState(EditInModalLanguage);
  const [value, setValue] = useState(content);
  useEffect(() => {
    if (language != 'json') {
      setJsonErr(null);
      return;
    }
    validJson(value)
      .then((e) => {
        setJsonErr(e);
      })
      .catch((e) => {
        console.log('call valid json error', e);
      });
  }, [value, language]);

  return (
    <Stack flex={1} spacing={1}>
      <Stack direction="row" spacing={1}>
        {editInModalLanguages.map((e) => (
          <Chip
            key={e.language}
            label={e.language}
            color={language == e.language ? 'success' : 'default'}
            onClick={() => {
              setLanguage(e.language);
              EditInModalLanguage = e.language;
            }}
          ></Chip>
        ))}
      </Stack>
      <SimpleFlexEditor
        value={content}
        language={language}
        handleValue={(v) => {
          setValue(v);
        }}
        errPos={jsonErrPos}
        barLeft={
          jsonErr ? (
            <Typography
              color="error"
              variant="body1"
              style={{ cursor: 'pointer' }}
              onClick={() => {
                if (!jsonErr) {
                  return;
                }
                setJsonErrPos(new Position(jsonErr.line, jsonErr.column));
              }}
            >
              {jsonErr.msg}
            </Typography>
          ) : undefined
        }
      ></SimpleFlexEditor>
      <Divider />
      <Stack spacing={1} direction="row">
        <Button
          onClick={() => {
            handleModify(value);
          }}
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
    extra: types.ShowTableExtraArgs
  ) => Promise<MuiDataGridData>;
  updateOP?: (wheres: object, values: object) => Promise<types.SqlExecuteRet>;
  deleteOP?: (wheres: object) => Promise<types.SqlExecuteRet>;
  put_value_in_sql: (typ: string, value?: string) => Promise<string>;
  addOP?: (x: object) => Promise<void>;
  notWarnPri?: boolean;
  kind: DataSourceKind;
  tableName: string;
  uuid;
  invalidCassandraTableViewCache?: () => Promise<void>;
  extractTableSelectWhere: (sql: string) => Promise<String | null>;
  extractTableSelectSort: (sql: string) => Promise<String | null>;
  isTable: boolean;
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
    uuid,
    invalidCassandraTableViewCache,
    extractTableSelectSort,
    extractTableSelectWhere,
    isTable,
  } = props;
  const [loading, setLoading] = useState(false);
  const [where, setWhere] = useState('');
  const [showSetWhere, setShowSetWhere] = useState(false);
  const [sort, setSort] = useState('');
  const [showSetSort, setShowSetSort] = useState(false);
  const [page, setPage] = useState(0);
  const setting = useContext(SettingContext);

  const [pageSize, setPageSize] = useState(SavePageSize.instance.get());
  const [resp, setResp] = useState<MuiDataGridData>({
    sql: '',
    fields: [],
    data: [],
    total: 0,
    cache_time: null,
    has_more: false,
  });
  const { showNotification } = useContext(NotificationContext);

  const [allowFiltering, setAllowFiltering] = useState(false);
  const [editCellInModal, setEditCellInModal] =
    useState<EditInModalCellData>(null);
  const [editInModalRow, setEditInModalRow] = useState<object>(null);
  const [viewInModal, setViewInModal] = useState<object>(null);
  const [showExportMarkDown, setShowExportMarkDown] = useState(false);
  const handleResp = (resp: MuiDataGridData) => {
    resp.data.forEach((data, index) => {
      data[keyNotInTable] = index;
    });
    for (let i = 0; i < resp.fields.length; i++) {
      let field = resp.fields[i];
      field['renderHeader'] = (e) => {
        let def = e.colDef;
        const makeTitle = (): string => {
          let f = def as types.Field;
          let ret = f.field + ' ';
          ret += f.data_type;
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
            <Tooltip title={makeTitle()}>{def.field}</Tooltip>
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
      let extra = null as types.ShowTableExtraArgs;
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
        const width = ColumnWidth.instance.get(field.field);
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
      if (!(await confirmAboutPrimary(notWarnPri, resp.fields))) {
        return oldRow;
      }
      try {
        let ret = await updateOP(old, updateRow);
        showNotification(
          'success',
          'Update OK',
          <Stack>
            <Typography> sql:{ret.sql} </Typography>
            <Typography> row affected :{ret.changes} </Typography>
          </Stack>
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

  const handleDelete = async (id) => {
    if (!(await confirmAboutPrimary(notWarnPri, resp.fields))) {
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
        <Stack>
          <Typography> sql:{ret.sql} </Typography>
          <Typography> row affected :{ret.changes} </Typography>
        </Stack>
      );
    } catch (e) {
      showNotification('error', 'delete failed', e);
    }
    handleReload(false);
  };

  useEffect(() => {
    handleReload(false);
  }, [page, pageSize, where, sort, allowFiltering]);

  const paginationModel = useMemo(() => {
    return { page, pageSize };
  }, [page, pageSize]);
  const onPaginationModelChange = useCallback(
    (e: GridPaginationModel, reason: GridCallbackDetails<'pagination'>) => {
      setPage(e.page);
      setPageSize(e.pageSize);
      if (reason.reason == 'setPaginationModel') {
        SavePageSize.instance.save(pageSize);
      }
    },
    []
  );
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
    setSort(`${quoteIdentifier(kind)(sort.field)} ${sort.sort.toUpperCase()}`);
  }, []);

  const onProcessRowUpdateError = useCallback((e) => {
    showNotification('error', 'update failed', e);
  }, []);

  const onColumnResize = useCallback(({ colDef, width }) => {
    ColumnWidth.instance.save(colDef.field, width);
  }, []);

  const slotProps = useMemo(() => {
    return {
      row: {
        onContextMenu: (e) => {
          const { id, field } = extractFieldAndIDFromMouseEvent(e);
          if (!field) {
            return;
          }
          let row = { ...resp.data.find((e) => e[keyNotInTable] == id) };
          let content = row[field];
          delete row[keyNotInTable];
          let nullable = resp.fields.find((e) => e.field == field).nullable;
          setUpMenu({
            content: (
              <Menu
                updateOP={updateOP}
                kind={kind}
                showExportMarkDown={() => {
                  setShowExportMarkDown(true);
                }}
                editCellInModal={() => {
                  setEditCellInModal(
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
                handleDelete={
                  deleteOP
                    ? () => {
                        handleDelete(id);
                      }
                    : undefined
                }
                row={row}
                tableName={tableName}
                addOP={addOP}
                setSort={setSort}
                setWhere={setWhere}
                put_value_in_sql={(value) => {
                  return put_value_in_sql(
                    resp.fields.find((e) => e.field == field).data_type,
                    value
                  );
                }}
                setNull={
                  updateOP
                    ? async () => {
                        let x = resp.data.find((e) => e[keyNotInTable] == id);
                        try {
                          let values = {};
                          values[field] = null;
                          delete x[keyNotInTable];
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
                resetColumnWidthCallBack={() => {
                  setResp((old) => {
                    return resetMuiDataRespWidth(old);
                  });
                }}
                viewInModal={() => {
                  setViewInModal(row);
                }}
              ></Menu>
            ),
            style: calMenuStyle(e),
          });
        },
        style: { cursor: 'context-menu' },
      },
      toolbar: {
        csvOptions: {
          delimiter: setting.csvDelimiter,
          includeHeaders: setting.includeHeader,
        },
      },
    } as GridSlotsComponentsProps;
  }, [resp]);

  const [editInModalBox, setEditInModalBox] = useState(null);
  useEffect(() => {
    if (editInModalBox) {
      let x1 = saveAndSetSize(editInModalBox, 'edit-cell');
      return () => {
        x1();
      };
    }
  }, [editInModalBox]);
  const [editRowInModalBox, setEditRowInModalBox] = useState(null);
  useEffect(() => {
    if (editRowInModalBox) {
      let x1 = saveAndSetSize(editRowInModalBox, 'edit-row', 'width');
      return () => {
        x1();
      };
    }
  }, [editRowInModalBox]);
  const [viewInModalBox, setViewInModalBox] = useState(null);
  useEffect(() => {
    if (viewInModalBox) {
      let x1 = saveAndSetSize(viewInModalBox, 'view-row-in-modal');
      return () => {
        x1();
      };
    }
  }, [viewInModalBox]);
  const slots = useMemo(() => {
    return { columnMenu: AppColumnMenu } as Partial<GridSlotsComponent>;
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
          color="primary"
          onClick={() => {
            handleReload(true);
          }}
        >
          reload
        </Button>

        <Tooltip title="filter something like 'age=18'">
          <Chip
            label={where}
            onClick={() => {
              setShowSetWhere(true);
            }}
            size="medium"
            color="primary"
            icon={<FilterAltIcon />}
          ></Chip>
        </Tooltip>
        {/* <Typography color="info"> order by </Typography> */}
        <Tooltip title="sort something like 'age asc'">
          <Chip
            label={sort}
            onClick={() => {
              setShowSetSort(true);
            }}
            size="medium"
            color="secondary"
            variant="outlined"
            icon={<FaArrowsAltV />}
          ></Chip>
        </Tooltip>
        {kind == DataSourceKind.Cassandra && (
          <>
            <FormControlLabel
              control={
                <Switch
                  checked={allowFiltering}
                  onChange={(e) => setAllowFiltering(e.target.checked)}
                />
              }
              label="allow filtering"
            />
            {resp.cache_time && (
              <>
                <Tooltip title="last cache time">
                  <Typography color="info">
                    {new Date(resp.cache_time * 1000).toLocaleString()}
                  </Typography>
                </Tooltip>
                <Tooltip
                  title="Cassandra does not support pagination with offset and limit, so the data is cached in the background,
                then returns data based on pagination."
                >
                  <Button
                    variant="outlined"
                    onClick={() => {
                      invalidCassandraTableViewCache()
                        .then(() => {
                          showNotification(
                            'success',
                            'invalid cassandra table or view cache OK'
                          );
                          handleReload(false);
                        })
                        .catch((e) => {
                          showNotification(
                            'error',
                            'invalid cassandra cache failed',
                            e
                          );
                        });
                    }}
                  >
                    clear cache
                  </Button>
                </Tooltip>
              </>
            )}
          </>
        )}
      </Stack>

      <Stack direction="row" spacing={1} sx={{ alignItems: 'center' }}>
        <Typography
          sx={{
            cursor: 'pointer',
            fontSize: '0.9rem',
            fontStyle: 'italic',
            color: 'success.main',
            ...textEllipsisStyle,
          }}
          variant="body1"
          onClick={() => {
            handleCopyToClipboard(resp.sql, showNotification);
          }}
        >
          {resp.sql}
        </Typography>

        <Stack
          direction="row"
          spacing={1}
          style={{
            justifyContent: 'flex-end',
            alignItems: 'center',
          }}
        >
          <ExecutingTime loading={loading}></ExecutingTime>
        </Stack>
      </Stack>
      <div style={{ display: 'grid' }}>
        <DataGrid
          showToolbar
          slots={slots}
          loading={loading}
          style={gridStyle}
          showCellVerticalBorder
          showColumnVerticalBorder
          slotProps={slotProps}
          rowHeight={setting.datagridRowHeightXXX()}
          columnHeaderHeight={setting.datagridRowHeightXXX()}
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

      {viewInModal && (
        <Modal
          open={!!viewInModal}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setViewInModal(null);
          })}
        >
          <Box
            sx={{ resize: 'both', ...modalStyle }}
            ref={setViewInModalBox}
            width="80%"
            height="30vh"
          >
            <ModalHeader title="View Row"></ModalHeader>
            <ViewRowInModal
              fields={resp.fields}
              x={viewInModal}
              isTable={isTable}
            ></ViewRowInModal>
          </Box>
        </Modal>
      )}
      {editCellInModal && (
        <Modal
          open={!!editCellInModal}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setEditCellInModal(null);
          })}
        >
          <Box
            sx={{ resize: 'both', ...modalStyle }}
            ref={setEditInModalBox}
            width="80%"
            height="30vh"
          >
            <ModalHeader title="edit cell"></ModalHeader>
            <EditInModal
              content={editCellInModal.content}
              handleModify={async (value) => {
                let newRow = { ...editCellInModal.row };
                newRow[editCellInModal.field] = value;
                await handleProcessRowUpdate(newRow, editCellInModal.row);
                handleReload(false);
                setEditCellInModal(null);
              }}
            ></EditInModal>
          </Box>
        </Modal>
      )}
      {editInModalRow && (
        <Modal
          open={!!editInModalRow}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setEditInModalRow(null);
          })}
        >
          <Box
            sx={{ resize: 'horizontal', ...modalStyle }}
            ref={setEditRowInModalBox}
            width="80%"
          >
            <ModalHeader title="edit row"></ModalHeader>
            <SqlTableViewData
              initValues={editInModalRow}
              buttonName="modify"
              fields={resp.fields}
              isAdd={false}
              handleData={async (args) => {
                await handleProcessRowUpdate(args, editInModalRow);
                setEditInModalRow(null);
                handleReload(false);
              }}
            ></SqlTableViewData>
          </Box>
        </Modal>
      )}

      {showExportMarkDown && (
        <Modal
          open={showExportMarkDown}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setShowExportMarkDown(false);
          })}
        >
          <Box sx={modalStyle} width="60%">
            <ModalHeader title="export markdown table"></ModalHeader>
            <ExportMarkDownTable fields={resp.fields} data={resp.data} />
          </Box>
        </Modal>
      )}

      {showSetWhere && (
        <SetFilterSort
          tableName={tableName}
          open={showSetWhere}
          value={where}
          setValue={(v) => {
            setWhere(v);
          }}
          handleClose={() => {
            setShowSetWhere(false);
          }}
          extractFn={extractTableSelectWhere}
          label="filter"
          kind={kind}
          uuid={uuid}
        ></SetFilterSort>
      )}

      {showSetSort && (
        <SetFilterSort
          tableName={tableName}
          open={showSetSort}
          value={sort}
          setValue={(v) => {
            setSort(v);
          }}
          kind={kind}
          handleClose={() => {
            setShowSetSort(false);
          }}
          label="sort"
          uuid={uuid}
          extractFn={extractTableSelectSort}
        ></SetFilterSort>
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
  isAdd: boolean;
}) {
  let { handleData, fields, initValues, buttonName, isAdd } = props;
  if (!initValues) {
    initValues = {};
  }
  fields.forEach((e) => {
    if (e.is_auto_increment && isAdd) {
      initValues[e.field] = '';
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
      if (!args[f.field] && !f.nullable && !f.default_value) {
        inputDoms.current[f.field].focus();
        showNotification('error', `filed '${f.field}' need be filled`);
        return;
      }
    }
    for (const key in args) {
      if (!args[key]) {
        // make sure '' convert to null
        args[key] = null;
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
                disabled={f.is_auto_increment}
                value={args[f.field]}
                helperText={
                  f.data_type +
                  ' ' +
                  (f.nullable ? 'null' : 'not null') +
                  ' ' +
                  (f.is_auto_increment ? 'auto increment' : '') +
                  ' ' +
                  (f.default_value ? `default ${f.default_value}` : '')
                }
                required={!f.nullable}
                label={f.field}
                multiline={isTypeLargeText(f.data_type)}
                rows={4}
                fullWidth
                onChange={(e) => {
                  const value = e.target.value;
                  let newArgs = { ...args };
                  newArgs[f.field] = value;
                  setArgs(newArgs);
                }}
              ></TextField>
            </Stack>
          );
        })}
      <Divider />
      <Stack direction="row" spacing={1} style={{ alignItems: 'flex-start' }}>
        <Button loading={loading} onClick={handleModify}>
          {buttonName}
        </Button>

        <TextField
          value={filter}
          onChange={(e) => {
            setFilter(e.target.value);
          }}
          label="filter"
          fullWidth
          helperText="can match field name and content"
          slotProps={{
            input: {
              endAdornment: filter && (
                <InputAdornment position="end">
                  <IconButton
                    onClick={() => {
                      setFilter('');
                    }}
                    edge="end"
                  >
                    <ClearIcon fontSize="small" />
                  </IconButton>
                </InputAdornment>
              ),
            },
          }}
        ></TextField>
      </Stack>
    </Stack>
  );
}

function Menu(props: {
  field: string;
  content: string;
  handleDelete?;
  setSort;
  setWhere;
  put_value_in_sql: (value?: string) => Promise<string>;
  setNull?: () => void;
  addOP?: (x: object) => Promise<void>;
  updateOP?;
  tableName: string;
  row: object;
  fields: types.Field[];
  nullable: boolean;
  editCellInModal: () => void;
  editRowInModal: () => void;
  resetColumnWidthCallBack: () => void;
  kind: DataSourceKind;
  viewInModal: () => void;
  showExportMarkDown: () => void;
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
    updateOP,
    row,
    fields,
    nullable,
    editCellInModal,
    editRowInModal,
    resetColumnWidthCallBack,
    kind,
    viewInModal,
    showExportMarkDown,
  } = props;
  const { showNotification } = useContext(NotificationContext);
  const [showFilter, setShowFilter] = useState(false);
  const { setTabs, setSelectTab } = useContext(TabsSetContext);
  const quoteFn = quoteIdentifier(kind);
  const handleClone = () => {
    setTabs((tabs) => {
      let t = (
        <SqlTableViewData
          buttonName="add"
          handleData={addOP}
          fields={fields}
          initValues={row}
          isAdd
        ></SqlTableViewData>
      );
      setTimeout(() => {
        setSelectTab(tabs.length);
      }, 100);
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
  let quoteField = quoteFn(field);
  return (
    <div>
      <MenuList>
        {updateOP && (
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

        {updateOP && (
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
            <ListItemText> Set As Null </ListItemText>
          </MenuItem>
        )}

        <MenuItem onClick={viewInModal}>
          <ListItemIcon>
            <VisibilityIcon style={{ fontSize: '1.4rem' }}></VisibilityIcon>
          </ListItemIcon>
          <ListItemText> View in Modal </ListItemText>
        </MenuItem>

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

        <Divider />
        <MenuItem
          onClick={() => {
            setSort(`${quoteField} ASC`);
          }}
        >
          <ListItemIcon>
            <ArrowUpwardIcon></ArrowUpwardIcon>
          </ListItemIcon>
          <ListItemText> {field} ASC </ListItemText>
        </MenuItem>
        <MenuItem
          onClick={() => {
            setSort(`${quoteField} DESC`);
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
        <Divider />

        {addOP && (
          <MenuItem onClick={handleClone}>
            <ListItemIcon>
              <FileCopyIcon></FileCopyIcon>
            </ListItemIcon>
            <ListItemText> Clone Row </ListItemText>
          </MenuItem>
        )}
        {handleDelete && (
          <MenuItem onClick={handleDelete}>
            <ListItemIcon>
              <DeleteForeverIcon></DeleteForeverIcon>
            </ListItemIcon>
            <ListItemText> Delete Row</ListItemText>
          </MenuItem>
        )}
        <MenuResetColumnWidths
          reset={resetColumnWidthCallBack}
        ></MenuResetColumnWidths>

        <MenuExportMarkDownTable
          showExportMarkDown={showExportMarkDown}
        ></MenuExportMarkDownTable>
      </MenuList>

      {showFilter && (
        <Modal
          open={showFilter}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setShowFilter(false);
          })}
        >
          <Box sx={modalStyle} width="50%">
            <ModalHeader title="Quick Filter"></ModalHeader>
            <FilterOnValue
              content={content}
              setWhere={setWhere}
              field={field}
              kind={kind}
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
    </div>
  );
}

let lastCompare = '=';
function FilterOnValue(props: {
  content: string;
  setWhere;
  field: string;
  handleClose;
  kind: DataSourceKind;
  put_value_in_sql: (x: string) => Promise<string>;
}) {
  const { content, setWhere, field, handleClose, put_value_in_sql, kind } =
    props;
  const compares = ['=', '<>', '>', '>=', '<', '<='];
  const [compare, setCompare] = useState(lastCompare);
  const { showNotification } = useContext(NotificationContext);
  const [formatValue, setFormatValue] = useState('');
  useEffect(() => {
    lastCompare = compare;
  }, [compare]);
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
    let quoteField = quoteIdentifier(kind)(field);
    if (compare == 'null') {
      return `${quoteField} IS NULL`;
    }
    if (compare == 'not null') {
      return `${quoteField} IS NOT NULL`;
    }
    if (compare == 'starts') {
      return `${quoteField} LIKE '${content}%'`;
    }
    if (compare == 'ends') {
      return `${quoteField} LIKE '%${content}'`;
    }
    if (compare == 'contains') {
      return `${quoteField} LIKE '%${content}%'`;
    }
    return `${quoteField} ${compare} ${formatValue}`;
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
            color={compare == x ? 'success' : 'default'}
            style={{ minWidth: '3rem' }}
          ></Chip>
        ))}
      </Stack>
      <Stack direction="row" spacing={1}>
        {['null', 'not null'].map((e) => (
          <Chip
            label={e}
            color={compare == e ? 'success' : 'default'}
            onClick={() => {
              setCompare(e);
            }}
            key={e}
          />
        ))}
      </Stack>
      {formatValue.endsWith("'") && (
        <Stack direction="row" spacing={1}>
          {['starts', 'ends', 'contains'].map((e) => (
            <Chip
              label={e}
              color={compare == e ? 'success' : 'default'}
              onClick={() => {
                setCompare(e);
              }}
              key={e}
            />
          ))}
        </Stack>
      )}

      <Divider />
      <Stack direction="row" spacing={1} style={{ alignItems: 'center' }}>
        <Button
          onClick={() => {
            setWhere(where());
            handleClose();
          }}
        >
          set
        </Button>
        <Typography color="success" style={textEllipsisStyle}>
          <em> {where()} </em>
        </Typography>
      </Stack>
    </Stack>
  );
}

const SqlTableView = memo(SqlTableView_);

export { SqlTableView, SqlTableViewData };
