import {
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from 'react';
import types from './ApiStruct';
import {
  DefAndMetas,
  EscDownEvent,
  modalHandleClose,
  modalStyle,
} from './Common';
import {
  calMenuStyle,
  ColumnWidth,
  ContextMenuContext,
  DataSourceKind,
  gridStyle,
  handleCopyToClipboard,
  saveAndSetSize,
  secondConfirm,
  SettingContext,
} from './helper';
import CancelIcon from '@mui/icons-material/Cancel';
import { NotificationContext } from './Notification';
import { invoke } from '@tauri-apps/api/core';
import CheckCircleIcon from '@mui/icons-material/CheckCircle';
import {
  DataGrid,
  GridColDef,
  GridSlotsComponentsProps,
} from '@mui/x-data-grid';
import {
  Box,
  Button,
  Checkbox,
  Chip,
  Divider,
  FormControl,
  FormControlLabel,
  FormLabel,
  IconButton,
  InputAdornment,
  InputLabel,
  ListItemIcon,
  ListItemText,
  MenuItem,
  MenuList,
  Modal,
  Radio,
  RadioGroup,
  Select,
  Stack,
  TextField,
  Tooltip,
  Typography,
} from '@mui/material';
import ContentCopyIcon from '@mui/icons-material/ContentCopy';
import EditIcon from '@mui/icons-material/Edit';
import DeleteForeverIcon from '@mui/icons-material/DeleteForever';
import AddIcon from '@mui/icons-material/Add';
import { BSON } from 'bson';
import { MongoValueEditor } from './MongoDB';
import { ModalHeader, SelectColumns, TrueOrFalse } from './CommonComp';
import ClearIcon from '@mui/icons-material/Clear';
import { DefAndMetasView, TriggerDetail } from './UdfAndMetaView';
function ColumnsStructure(props: {
  uuid: string;
  kind: DataSourceKind;
  table: string;
}) {
  const { uuid, kind, table } = props;
  const setting = useContext(SettingContext);
  const [loading, setLoading] = useState(false);
  const [fields, setFields] = useState<types.Field[]>([]);
  const { showNotification } = useContext(NotificationContext);
  const [row, setRow] = useState<types.Field>(null);
  const supportModifyColumn = new Set([
    DataSourceKind.MariaDB,
    DataSourceKind.MsSql,
    DataSourceKind.Mysql,
    DataSourceKind.Postgres,
    DataSourceKind.Cockroach,
    DataSourceKind.Oracle,
    DataSourceKind.TiDB,
    DataSourceKind.Cassandra,
    DataSourceKind.ClickHouse,
    DataSourceKind.DuckDB,
    DataSourceKind.Sqlite,
    DataSourceKind.LibSql,
  ]).has(kind);

  const onColumnResize = useCallback(({ colDef, width }) => {
    ColumnWidth.instance.save(colDef.field, width);
  }, []);

  const handleDrop = async (col: string) => {
    const confirm = await secondConfirm({
      title: `Drop Column '${col}'`,
      content: '',
      confirmText: 'DROP COLUMN',
    });
    if (!confirm) {
      return;
    }
    invoke('drop_table_column', {
      name: uuid,
      tableName: table,
      columnName: col,
    })
      .then(() => {
        showNotification('success', 'Drop column OK');
        handleReload();
      })
      .catch((e) => {
        showNotification('error', 'Drop column Failure', e);
      });
  };

  const [showAdd, setShowAdd] = useState(false);
  const handleReload = () => {
    setLoading(true);
    invoke<types.Field[]>('show_table_columns', {
      name: uuid,
      tableName: table,
    })
      .then((fs) => {
        setFields(fs);
      })
      .catch((e) => {
        showNotification('error', 'list column failed', e);
      })
      .finally(() => {
        setLoading(false);
      });
  };
  useEffect(() => {
    handleReload();
  }, []);
  let cols = useMemo(() => {
    return [
      { field: 'field' },
      {
        field: 'data_type',
        renderHeader: () => {
          return 'type';
        },
      },
      {
        field: 'nullable',
        renderCell: (row) => {
          return <TrueOrFalse b={row.row['nullable']} />;
        },
      },
      {
        field: 'is_primary',
        renderHeader: () => {
          return 'primary key';
        },
        renderCell: (row) => {
          return <TrueOrFalse b={row.row['is_primary']} />;
        },
      },
      {
        field: 'is_auto_increment',
        renderHeader: () => {
          return 'auto increment';
        },
        renderCell: (row) => {
          return <TrueOrFalse b={row.row['is_auto_increment']} />;
        },
      },
      {
        field: 'default_value',
        renderHeader: () => {
          return 'default value';
        },
      },
      {
        field: 'sortable',
        renderCell: (row) => {
          return <TrueOrFalse b={row.row['sortable']} />;
        },
      },
    ] as GridColDef[];
  }, []);
  cols.forEach((e) => {
    const width = ColumnWidth.instance.get(e.field);
    if (width) {
      e.width = width;
    }
  });
  const { setUpMenu } = useContext(ContextMenuContext);

  function Menu(props: { col: string; content: string }) {
    const { col, content } = props;
    return (
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
        {supportModifyColumn && (
          <MenuItem
            onClick={() => {
              handleDrop(col);
            }}
          >
            <ListItemIcon>
              <DeleteForeverIcon></DeleteForeverIcon>
            </ListItemIcon>
            <ListItemText> drop </ListItemText>
          </MenuItem>
        )}
      </MenuList>
    );
  }

  const slotProps = useMemo(() => {
    return {
      row: {
        onContextMenu: (e) => {
          const { currentTarget, target } = e;
          {
            const field = (target as HTMLDivElement).getAttribute('data-field');
            if (!field) {
              return;
            }
          }
          let content = (target as HTMLDivElement).innerHTML;
          const col = (currentTarget as HTMLDivElement).getAttribute('data-id');
          setUpMenu({
            content: <Menu col={col} content={content}></Menu>,
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
  }, []);

  const getRowId = useCallback((e) => {
    return e['field'];
  }, []);

  const onRowDoubleClick = useCallback((row) => {
    setRow(row.row);
  }, []);

  const [modalBox, setModalBox] = useState(null);

  useEffect(() => {
    if (modalBox) {
      let c = saveAndSetSize(modalBox, 'table-column', 'width');
      return c;
    }
  }, [modalBox]);
  useEffect(() => {
    if (row) {
      if ([DataSourceKind.Sqlite, DataSourceKind.LibSql].includes(kind)) {
        showNotification('error', `${kind} not support modify column`);
      }
    }
  }, [row]);
  return (
    <Stack spacing={1}>
      <Stack direction="row" spacing={1} style={{ alignItems: 'center' }}>
        <Button loading={loading} onClick={handleReload}>
          reload
        </Button>

        {supportModifyColumn && (
          <Button
            onClick={() => {
              setShowAdd(true);
            }}
          >
            <AddIcon></AddIcon>
          </Button>
        )}
        {supportModifyColumn && (
          <Tooltip title="double click a row to modify.">
            <Typography color="info">modify </Typography>
          </Tooltip>
        )}
      </Stack>

      <div style={{ display: 'grid' }}>
        <DataGrid
          showToolbar
          getRowId={getRowId}
          slotProps={slotProps}
          style={gridStyle}
          onColumnResize={onColumnResize}
          showCellVerticalBorder
          showColumnVerticalBorder
          rowHeight={setting.datagridRowHeightXXX()}
          columnHeaderHeight={setting.datagridRowHeightXXX()}
          columns={cols}
          rows={fields}
          onRowDoubleClick={onRowDoubleClick}
        ></DataGrid>
      </div>

      {showAdd && (
        <Modal
          open={showAdd}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setShowAdd(false);
          })}
        >
          <Box
            sx={{ resize: 'horizontal', ...modalStyle }}
            width="60%"
            ref={setModalBox}
          >
            <ModalHeader title="Add Column"></ModalHeader>
            <TableAddColumn
              table={table}
              uuid={uuid}
              handleClose={() => {
                setShowAdd(false);
              }}
              reload={handleReload}
              kind={kind}
              fields={fields}
            ></TableAddColumn>
          </Box>
        </Modal>
      )}
      {supportModifyColumn &&
        row &&
        ![DataSourceKind.Sqlite, DataSourceKind.LibSql].includes(kind) && (
          <Modal
            open={!!row}
            onClose={modalHandleClose((e: EscDownEvent) => {
              e.preventDefault();
              setRow(null);
            })}
          >
            <Box
              sx={{ resize: 'horizontal', ...modalStyle }}
              width="60%"
              ref={setModalBox}
            >
              <ModalHeader title="Modify Column"></ModalHeader>
              <ModifyTableColumn
                table={table}
                uuid={uuid}
                handleClose={() => {
                  setRow(null);
                }}
                col={row.field}
                typ={row.data_type}
                reload={handleReload}
                kind={kind}
              ></ModifyTableColumn>
            </Box>
          </Modal>
        )}
    </Stack>
  );
}

function RenameIndex(props: {
  uuid: string;
  table: string;
  reload;
  handleClose;
  indexName;
}) {
  const { uuid, table, reload, handleClose, indexName } = props;
  const [loading, setLoading] = useState(false);
  const [newName, setNewname] = useState(indexName);
  const { showNotification } = useContext(NotificationContext);
  const handleRename = () => {
    if (newName == indexName) {
      showNotification('error', 'name not change');
      return;
    }
    setLoading(true);

    invoke('rename_index', {
      name: uuid,
      tableName: table,
      oldName: indexName,
      newName: newName,
    })
      .then(() => {
        showNotification('success', 'Rename OK');
        reload();
        handleClose();
      })
      .catch((e) => {
        showNotification('error', 'Rename Failure', e);
      })
      .finally(() => {
        setLoading(false);
      });
  };
  return (
    <Stack spacing={1}>
      <TextField
        label="new name"
        value={newName}
        onChange={(e) => {
          setNewname(e.target.value);
        }}
      ></TextField>
      <Stack direction="row" spacing={1}>
        <Button color="primary" loading={loading} onClick={handleRename}>
          rename
        </Button>
      </Stack>
    </Stack>
  );
}

function IndexesStructure(props: {
  uuid: string;
  kind: DataSourceKind;
  table: string;
}) {
  const { uuid, kind, table } = props;
  const setting = useContext(SettingContext);
  const [loading, setLoading] = useState(false);
  const [indexes, setIndexes] = useState<types.Index[]>([]);
  const { showNotification } = useContext(NotificationContext);
  const [renameIndexName, setRenameIndexName] = useState('');
  const [showAdd, setShowAdd] = useState(false);

  const onColumnResize = useCallback(({ colDef, width }) => {
    ColumnWidth.instance.save(colDef.field, width);
  }, []);

  const handleReload = () => {
    setLoading(true);
    invoke<types.Index[]>('show_indexes', {
      name: uuid,
      tableName: table,
    })
      .then((indexes) => {
        setIndexes(indexes);
      })
      .catch((e) => {
        showNotification('error', 'list indexes failed', e);
      })
      .finally(() => {
        setLoading(false);
      });
  };

  const handleDrop = async (indexName) => {
    let confirm = await secondConfirm({
      title: `drop index '${indexName}'`,
      content: 'Are u sure to drop the index',
      confirmText: 'Drop',
    });
    if (!confirm) {
      return;
    }
    invoke('drop_index', {
      name: uuid,
      tableName: table,
      index: indexName,
    })
      .then(() => {
        showNotification('success', 'Drop OK');
        handleReload();
      })
      .catch((e) => {
        showNotification('error', 'Drop index failure', e);
      });
  };

  useEffect(() => {
    handleReload();
  }, []);
  let cols = useMemo(() => {
    return [
      { field: 'name' },
      { field: 'kind' },
      { field: 'columns' },
    ] as GridColDef[];
  }, []);
  cols.forEach((e) => {
    const width = ColumnWidth.instance.get(e.field);
    if (width) {
      e.width = width;
    }
  });

  const { setUpMenu } = useContext(ContextMenuContext);

  function Menu(props: { content: string; indexName: string }) {
    const { content, indexName } = props;

    return (
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
            setRenameIndexName(indexName);
          }}
        >
          <ListItemIcon>
            <EditIcon></EditIcon>
          </ListItemIcon>
          <ListItemText> rename </ListItemText>
        </MenuItem>
        <MenuItem
          onClick={() => {
            handleDrop(indexName);
          }}
        >
          <ListItemIcon>
            <DeleteForeverIcon></DeleteForeverIcon>
          </ListItemIcon>
          <ListItemText> drop </ListItemText>
        </MenuItem>
      </MenuList>
    );
  }

  const slotProps = useMemo(() => {
    return {
      row: {
        onContextMenu: (e) => {
          const { currentTarget, target } = e;
          {
            const field = (target as HTMLDivElement).getAttribute('data-field');
            if (!field) {
              return;
            }
          }
          let content = (target as HTMLDivElement).innerHTML;
          const indexName = (currentTarget as HTMLDivElement).getAttribute(
            'data-id'
          );
          setUpMenu({
            content: <Menu indexName={indexName} content={content}></Menu>,
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
  }, []);

  const getRowId = useCallback((e) => {
    return e['name'];
  }, []);

  const [modalBox, setModalBox] = useState(null);
  useEffect(() => {
    if (modalBox) {
      let c = saveAndSetSize(
        modalBox,
        'create-index' + (kind == DataSourceKind.Mongo ? '-mongo' : '-other'),
        kind == DataSourceKind.Mongo ? 'both' : 'width'
      );
      return c;
    }
  }, [modalBox]);
  return (
    <Stack spacing={1}>
      <Stack direction="row" spacing={1}>
        <Button loading={loading} onClick={handleReload}>
          reload
        </Button>

        <Button
          onClick={() => {
            setShowAdd(true);
          }}
        >
          <AddIcon></AddIcon>
        </Button>
      </Stack>

      <div style={{ display: 'grid' }}>
        <DataGrid
          showToolbar
          getRowId={getRowId}
          style={gridStyle}
          slotProps={slotProps}
          onColumnResize={onColumnResize}
          showCellVerticalBorder
          showColumnVerticalBorder
          rowHeight={setting.datagridRowHeightXXX()}
          columnHeaderHeight={setting.datagridRowHeightXXX()}
          columns={cols}
          rows={indexes}
        ></DataGrid>
      </div>

      {showAdd && kind == DataSourceKind.Mongo && (
        <Modal
          open={showAdd}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setShowAdd(false);
          })}
        >
          <Box
            sx={{
              resize: 'both',
              ...modalStyle,
            }}
            width="60%"
            ref={setModalBox}
            height={kind == DataSourceKind.Mongo ? '40vh' : 'auto'}
          >
            <ModalHeader title="create index"></ModalHeader>
            <MongoAddIndex
              uuid={uuid}
              table={table}
              handleClose={() => {
                setShowAdd(false);
              }}
              reload={handleReload}
            ></MongoAddIndex>
          </Box>
        </Modal>
      )}

      {showAdd && kind != DataSourceKind.Mongo && (
        <Modal
          open={showAdd}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setShowAdd(false);
          })}
        >
          <Box
            sx={{
              resize: 'horizontal',
              ...modalStyle,
            }}
            width="60%"
            ref={setModalBox}
          >
            <ModalHeader title="create index"></ModalHeader>
            <AddIndex
              uuid={uuid}
              table={table}
              handleClose={() => {
                setShowAdd(false);
              }}
              kind={kind}
              reload={handleReload}
            ></AddIndex>
          </Box>
        </Modal>
      )}

      {renameIndexName && (
        <Modal
          open={!!renameIndexName}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setRenameIndexName('');
          })}
        >
          <Box sx={modalStyle} width="40%">
            <ModalHeader title={`Rename Index`}></ModalHeader>
            <RenameIndex
              uuid={uuid}
              table={table}
              indexName={renameIndexName}
              handleClose={() => {
                setRenameIndexName('');
              }}
              reload={handleReload}
            ></RenameIndex>
          </Box>
        </Modal>
      )}
    </Stack>
  );
}

function MongoAddIndex(props: { uuid; table; handleClose; reload }) {
  const { uuid, table, handleClose, reload } = props;

  const indexTypes = ['normal', 'unique'];
  const [indexType, setIndexType] = useState(indexTypes[0]);
  const [indexName, setIndexName] = useState('');
  const indexNameDom = useRef<HTMLInputElement>(null);

  const { showNotification } = useContext(NotificationContext);
  const handleAdd = async (keys) => {
    try {
      if (!indexName) {
        showNotification('error', "index name can't be none");
        indexNameDom.current.focus();
        return;
      }
      await invoke('mongo_create_index', {
        name: uuid,
        tableName: table,
        keys: BSON.serialize(keys),
        unique: indexType == 'unique',
        indexName: indexName ? indexName : null,
      });
      showNotification('success', 'create index ok');
      handleClose();
      reload();
    } catch (e) {
      showNotification('error', 'create index failed', e);
    }
  };

  return (
    <Stack spacing={1} flex={1}>
      <TextField
        value={indexName}
        onChange={(e) => {
          setIndexName(e.target.value);
        }}
        inputRef={indexNameDom}
        label="index name"
        required
      ></TextField>

      <RadioGroup
        row
        onChange={(e) => {
          setIndexType(e.target.value);
        }}
        value={indexType}
      >
        {indexTypes.map((e) => (
          <FormControlLabel key={e} value={e} control={<Radio />} label={e} />
        ))}
      </RadioGroup>
      <MongoValueEditor
        setOP={async (doc) => {
          handleAdd(doc);
        }}
        doc={{}}
        setName="create index"
      ></MongoValueEditor>
    </Stack>
  );
}

function AddIndex(props: {
  uuid;
  table;
  handleClose;
  reload;
  kind: DataSourceKind;
}) {
  let { uuid, table, handleClose, reload, kind } = props;

  const [fields, setFields] = useState<types.Field[]>([]);
  const [includes, setIncludes] = useState<string[]>([]);
  const { showNotification } = useContext(NotificationContext);
  const indexTypes = ['normal', 'unique'];
  const [indexType, setIndexType] = useState(indexTypes[0]);
  const [indexName, setIndexName] = useState('');
  const indexNameDom = useRef<HTMLInputElement>(null);

  let indexTypeJsx;

  switch (kind) {
    case DataSourceKind.Cassandra:
      break;
    default:
      indexTypeJsx = (
        <>
          <FormLabel component="legend">unique</FormLabel>
          <RadioGroup
            row
            onChange={(e) => {
              setIndexType(e.target.value);
            }}
            value={indexType}
          >
            {indexTypes.map((e) => (
              <FormControlLabel
                key={e}
                value={e}
                control={<Radio />}
                label={e}
              />
            ))}
          </RadioGroup>
        </>
      );
      break;
  }

  useEffect(() => {
    invoke<types.Field[]>('show_table_columns', {
      name: uuid,
      tableName: table,
    })
      .then((e) => {
        setFields(e);
        e;
      })
      .catch((e) => {
        showNotification('error', 'list fields failed', e);
      });
  }, []);
  const handleAdd = async () => {
    if (!indexName) {
      showNotification('error', "index name can't be none");
      indexNameDom.current.focus();
      return;
    }
    if (includes.length == 0) {
      showNotification('error', 'select at least one column');
      return;
    }
    try {
      await invoke('create_index', {
        name: uuid,
        tableName: table,
        fields: includes,
        indexType,
        indexName: indexName ? indexName : null,
      });
      showNotification('success', 'create index ok');
      handleClose();
      reload();
    } catch (e) {
      showNotification('error', 'create index failed', e);
    }
  };

  return (
    <Stack spacing={1}>
      <TextField
        value={indexName}
        onChange={(e) => {
          setIndexName(e.target.value);
        }}
        inputRef={indexNameDom}
        label="index name"
        required
        slotProps={{
          input: {
            endAdornment: indexName && (
              <InputAdornment position="end">
                <IconButton
                  onClick={() => {
                    setIndexName('');
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
      <SelectColumns
        includes={includes}
        setIncludes={setIncludes}
        fields={fields}
      ></SelectColumns>
      {indexTypeJsx}
      <Divider />
      <Stack direction="row">
        <Button onClick={handleAdd} color="primary">
          create
        </Button>
      </Stack>
    </Stack>
  );
}

const typeHint = 'some type require specify a size like `varchar(128)`';

function TableAddColumn(props: {
  table: string;
  uuid;
  handleClose;
  reload;
  kind: DataSourceKind;
  fields: types.Field[];
}) {
  let { table, uuid, handleClose, reload, kind, fields } = props;
  const [name, setName] = useState('');
  const [nullness, setNullness] = useState('');
  const [typ, setTyp] = useState('');
  const { showNotification } = useContext(NotificationContext);
  const [after, setAfter] = useState('');
  const [defaultValue, setDefaultValue] = useState('');
  const supportAfter =
    kind == DataSourceKind.Mysql || kind == DataSourceKind.MariaDB;
  const nameDom = useRef(null);
  const [loading, setLoading] = useState(false);
  const handleAdd = () => {
    if (!name) {
      showNotification('error', 'column name is empty');
      nameDom.current.focus();
      return;
    }
    if (!typ) {
      showNotification('error', 'type is empty');
      return;
    }
    setLoading(true);
    invoke('add_table_column', {
      name: uuid,
      tableName: table,
      colName: name,
      typ: {
        typ: typ,
        nullness: nullness ? nullness : null,
        after: after ? after : null,
        default_value: defaultValue ? defaultValue : null,
      } as types.ColumnType,
    })
      .then(() => {
        showNotification('success', 'Add Column OK');
        handleClose();
        reload();
      })
      .catch((e) => {
        showNotification('error', 'Add Column failure', e);
      })
      .finally(() => {
        setLoading(false);
      });
  };

  const supportNull = kindSupportNull(kind);
  const supportDefault = kindSupportDefault(kind);
  return (
    <Stack spacing={1}>
      <TextField
        value={name}
        required
        inputRef={nameDom}
        onChange={(e) => {
          setName(e.target.value.trim());
        }}
        slotProps={{
          input: {
            endAdornment: name && (
              <InputAdornment position="end">
                <IconButton
                  onClick={() => {
                    setName('');
                  }}
                  edge="end"
                >
                  <ClearIcon fontSize="small" />
                </IconButton>
              </InputAdornment>
            ),
          },
        }}
        label="column name"
      ></TextField>
      <TableTypes
        uuid={uuid}
        handleChange={(e) => {
          setTyp(e);
        }}
        type={typ}
      ></TableTypes>
      {/*   TODO some mariadb require a length parameter    */}
      <Stack direction="row" spacing={1}>
        <TextField
          fullWidth
          value={typ}
          onChange={(e) => setTyp(e.target.value)}
          label="type"
          helperText={typeHint}
          slotProps={{
            input: {
              endAdornment: typ && (
                <InputAdornment position="end">
                  <IconButton
                    onClick={() => {
                      setTyp('');
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
      {supportNull && (
        <ColumnNullness
          nullness={nullness}
          handleNullness={(e) => {
            setNullness(e);
          }}
        ></ColumnNullness>
      )}

      {supportDefault && (
        <SqlDefaultValue
          uuid={uuid}
          defaultValue={defaultValue}
          setDefaultValue={setDefaultValue}
        ></SqlDefaultValue>
      )}

      {supportAfter && (
        <FormControl style={{ minWidth: '180px' }}>
          <InputLabel>Select After Column</InputLabel>
          <Select
            value={after}
            onChange={(e) => {
              setAfter(e.target.value);
            }}
            label="Select After Column"
          >
            <MenuItem value="">none</MenuItem>
            {fields.map((e) => (
              <MenuItem key={e.field} value={e.field}>
                {e.field}
              </MenuItem>
            ))}
          </Select>
        </FormControl>
      )}

      <Divider />
      <Stack direction="row">
        <Button loading={loading} color="primary" onClick={handleAdd}>
          add
        </Button>
      </Stack>
    </Stack>
  );
}

function SqlDefaultValue(props: {
  uuid: string;
  defaultValue;
  setDefaultValue;
}) {
  const { uuid, defaultValue, setDefaultValue } = props;
  const [currentValue, setCurrentValue] = useState<string[]>([]);
  const { showNotification } = useContext(NotificationContext);
  useEffect(() => {
    invoke<string[]>('list_sql_current_values', { name: uuid })
      .then((ts) => {
        setCurrentValue(ts);
      })
      .catch((e) => {
        showNotification('error', 'list sql current values failed', e);
      });
  }, []);

  return (
    <Stack direction="row" spacing={1}>
      <TextField
        label="default value"
        value={defaultValue}
        onChange={(e) => {
          setDefaultValue(e.target.value);
        }}
        style={{ flex: 1 }}
        slotProps={{
          input: {
            endAdornment: defaultValue && (
              <InputAdornment position="end">
                <IconButton
                  onClick={() => {
                    setDefaultValue('');
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
      <FormControl style={{ flex: 1 }}>
        <InputLabel>current values </InputLabel>
        <Select
          value={defaultValue}
          label="font family"
          onChange={(e) => setDefaultValue(e.target.value as string)}
        >
          <MenuItem value="">empty </MenuItem>
          {currentValue.map((e) => {
            return (
              <MenuItem key={e} value={e}>
                {e}
              </MenuItem>
            );
          })}
        </Select>
      </FormControl>
    </Stack>
  );
}

function ModifyTableColumn(props: {
  uuid: string;
  table: string;
  col: string;
  handleClose;
  reload;
  typ: string;
  kind: DataSourceKind;
}) {
  let { uuid, table, col, handleClose, reload, typ, kind } = props;
  const [loading, setLoading] = useState(false);
  const [newName, setNewName] = useState(col);
  const [newTyp, setNewTyp] = useState(typ);
  const { showNotification } = useContext(NotificationContext);
  const [modifyType, setModifyType] = useState(false);
  const [nullness, setNullness] = useState('');
  const handleModify = () => {
    if (newName == col && !newTyp) {
      showNotification('error', 'nothing is changed');
      return;
    }
    if (!newName) {
      showNotification('error', "name can't empty");
      return;
    }
    setLoading(true);
    let typePara = null;
    if (typ != newTyp) {
      typePara = { typ: newTyp, null: nullness ? nullness : nullness };
    }
    invoke('modify_table_column', {
      name: uuid,
      tableName: table,
      oldName: col,
      newName: newName,
      typ: typePara,
    })
      .then((_) => {
        showNotification('success', 'Modify column OK');
        reload();
        handleClose();
      })
      .catch((e) => {
        showNotification('error', 'Modify column Failure', e);
      })
      .finally(() => {
        setLoading(false);
      });
  };
  let supportModifyType = kind != DataSourceKind.Cassandra;
  return (
    <Stack spacing={1}>
      <TextField
        fullWidth
        label="New Name"
        value={newName}
        onChange={(e) => {
          setNewName(e.target.value);
        }}
        required
        slotProps={{
          input: {
            endAdornment: newName && (
              <InputAdornment position="end">
                <IconButton
                  onClick={() => {
                    setNewName('');
                  }}
                  edge="end"
                >
                  <ClearIcon fontSize="small" />
                </IconButton>
              </InputAdornment>
            ),
          },
        }}
        error={!newName}
      ></TextField>
      {supportModifyType && !modifyType && (
        <FormControlLabel
          control={
            <Checkbox
              checked={modifyType}
              onChange={(e) => {
                setModifyType(e.target.checked);
              }}
              name="myCheckbox"
              color="primary"
            />
          }
          label="modify TYPE"
        />
      )}
      {modifyType && (
        <Stack spacing={1}>
          <TableTypes
            uuid={uuid}
            handleChange={(e) => {
              setNewTyp(e);
            }}
            type={typ}
          ></TableTypes>

          <Stack direction="row" spacing={1} style={{ alignItems: 'center' }}>
            <TextField
              label="Type"
              value={newTyp}
              onChange={(e) => {
                setNewTyp(e.target.value);
              }}
              fullWidth
              helperText={typeHint}
              slotProps={{
                input: {
                  endAdornment: newTyp && (
                    <InputAdornment position="end">
                      <IconButton
                        onClick={() => {
                          setNewTyp('');
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
          {kindSupportNull(kind) && (
            <ColumnNullness
              nullness={nullness}
              handleNullness={setNullness}
            ></ColumnNullness>
          )}
        </Stack>
      )}
      <Stack direction="row" spacing={1}>
        <Button loading={loading} color="error" onClick={handleModify}>
          modify
        </Button>
      </Stack>
    </Stack>
  );
}

function TableTypes(props: { handleChange; uuid; type }) {
  let { handleChange, uuid, type } = props;
  const [types, setTypes] = useState<string[]>([]);
  const { showNotification } = useContext(NotificationContext);
  const [match, setMatch] = useState('');
  useEffect(() => {
    invoke<string[]>('list_data_types', { name: uuid })
      .then((ts) => {
        setTypes(ts);
      })
      .catch((e) => {
        showNotification('error', 'should not return a error', e);
      });
  }, []);
  return (
    <Stack
      direction="row"
      spacing={1}
      style={{ flexWrap: 'wrap', alignItems: 'center' }}
    >
      <TextField
        style={{ width: '16rem' }}
        label="type match"
        value={match}
        onChange={(e) => {
          setMatch(e.target.value);
        }}
        slotProps={{
          input: {
            endAdornment: match && (
              <InputAdornment position="end">
                <IconButton
                  onClick={() => {
                    setMatch('');
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
      <FormControl style={{ flex: 1 }}>
        <InputLabel>select type</InputLabel>
        <Select
          value={type}
          onChange={(e) => {
            handleChange(e.target.value as string);
          }}
        >
          {types
            .filter((type) => {
              if (match) {
                return type.toLowerCase().includes(match.toLowerCase());
              } else {
                return true;
              }
            })
            .map((type) => (
              <MenuItem key={type} value={type}>
                {type}
              </MenuItem>
            ))}
        </Select>
      </FormControl>
    </Stack>
  );
}

function ColumnNullness({ nullness, handleNullness }) {
  const xxx = [
    { name: 'none', value: '' },
    { name: 'null', value: 'null' },
    {
      name: 'not null',
      value: 'not null',
    },
  ];
  return (
    <RadioGroup
      row
      onChange={(_, value) => {
        handleNullness(value);
      }}
      value={nullness}
    >
      {xxx.map((e) => (
        <FormControlLabel
          key={e.name}
          value={e.value}
          control={<Radio />}
          label={e.name}
        />
      ))}
    </RadioGroup>
  );
}

function kindSupportNull(kind: DataSourceKind): boolean {
  return kind != DataSourceKind.Cassandra;
}
function kindSupportDefault(kind: DataSourceKind): boolean {
  return kind != DataSourceKind.Cassandra;
}

function triggerIsEnable(enable: types.TriggerEnable): boolean {
  if (enable.kind == 'postgres') {
    return enable.enable.toUpperCase() != 'D';
  }
  if (enable.kind == 'bool') {
    return enable.enable;
  }
}

function TriggersStructure(props: {
  uuid: string;
  kind: DataSourceKind;
  table: string;
}) {
  const { uuid, kind, table } = props;
  const setting = useContext(SettingContext);
  const [loading, setLoading] = useState(false);
  const [triggers, setTriggers] = useState<types.TriggerDetail[]>([]);
  const [row, setRow] = useState<types.TriggerDetail>(null);
  const { showNotification } = useContext(NotificationContext);

  const onColumnResize = useCallback(({ colDef, width }) => {
    ColumnWidth.instance.save(colDef.field, width);
  }, []);

  const handleReload = () => {
    setLoading(true);
    invoke<types.TriggerDetail[]>('show_trigger', {
      name: uuid,
      tableName: table,
    })
      .then((ts) => {
        setTriggers(ts);
      })
      .catch((e) => {
        showNotification('error', 'list indexes failed', e);
      })
      .finally(() => {
        setLoading(false);
      });
  };
  let callPara = { name: uuid, tableName: table };

  const handleDrop = async (trigger) => {
    const confirm = await secondConfirm({
      title: `Drop trigger '${trigger}'`,
      confirmText: 'Drop',
    });
    if (!confirm) {
      return;
    }
    invoke('drop_trigger', { trigger, ...callPara })
      .then(() => {
        showNotification('success', 'Drop Trigger OK');
        handleReload();
      })
      .catch((e) => {
        showNotification('error', 'Drop trigger failure', e);
      });
  };

  const handleEnables = (invoke_name: string, trigger, operation: string) => {
    invoke(invoke_name, { trigger, ...callPara })
      .then(() => {
        showNotification('error', `${operation}  OK`);
        handleReload();
      })
      .catch((e) => {
        showNotification('error', `${operation}  Failure`, e);
      });
  };

  useEffect(() => {
    handleReload();
  }, []);
  let cols = useMemo(() => {
    return [
      { field: 'name' },
      { field: 'event' },
      { field: 'sql' },
      {
        field: 'enable',
        renderCell: (r) => {
          let enable = r.row.enable as types.TriggerEnable;
          if (!enable) {
            return;
          }
          switch (enable.kind) {
            case 'postgres':
              return enable.enable;
            case 'bool':
              return <TrueOrFalse b={enable.enable} />;
          }
        },
      },
    ] as GridColDef[];
  }, []);
  cols.forEach((e) => {
    const width = ColumnWidth.instance.get(e.field);
    if (width) {
      e.width = width;
    }
  });
  const { setUpMenu } = useContext(ContextMenuContext);

  function Menu(props: { content: string; detail: types.TriggerDetail }) {
    const { content, detail } = props;
    let enable;
    if (detail.enable) {
      enable = triggerIsEnable(detail.enable) ? (
        <MenuItem
          sx={{ color: 'error.main' }}
          onClick={() => {
            handleEnables('disable_trigger', detail.name, 'Disable trigger');
          }}
        >
          <ListItemIcon>
            <CancelIcon></CancelIcon>
          </ListItemIcon>
          <ListItemText>Disable</ListItemText>
        </MenuItem>
      ) : (
        <MenuItem
          onClick={() => {
            handleEnables('enable_trigger', detail.name, 'Enable trigger');
          }}
        >
          <ListItemIcon>
            <CheckCircleIcon></CheckCircleIcon>
          </ListItemIcon>
          <ListItemText>Enable</ListItemText>
        </MenuItem>
      );
    }

    return (
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

        {enable}
        <MenuItem
          onClick={() => {
            handleDrop(detail.name);
          }}
        >
          <ListItemIcon>
            <DeleteForeverIcon></DeleteForeverIcon>
          </ListItemIcon>
          <ListItemText> drop </ListItemText>
        </MenuItem>
      </MenuList>
    );
  }

  const slotProps = useMemo(() => {
    return {
      row: {
        onContextMenu: (e) => {
          const { currentTarget, target } = e;
          {
            const field = (target as HTMLDivElement).getAttribute('data-field');
            if (!field) {
              return;
            }
          }
          let content = (target as HTMLDivElement).innerHTML;
          const name = (currentTarget as HTMLDivElement).getAttribute(
            'data-id'
          );
          let detail = triggers.find((e) => e.name == name);
          setUpMenu({
            content: <Menu detail={detail} content={content}></Menu>,
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
  }, [triggers]);

  const getRowId = useCallback((e) => {
    return e['name'];
  }, []);
  const onRowDoubleClick = useCallback((row) => {
    setRow(row.row);
  }, []);

  return (
    <Stack spacing={1}>
      <Stack direction="row" spacing={1} style={{ alignItems: 'center' }}>
        <Button loading={loading} onClick={handleReload}>
          reload
        </Button>
        {triggers.length > 0 && (
          <Tooltip title="double click a row to see detail">
            <Typography color="info">detail?</Typography>
          </Tooltip>
        )}
      </Stack>

      <div style={{ display: 'grid' }}>
        <DataGrid
          showToolbar
          getRowId={getRowId}
          style={gridStyle}
          slotProps={slotProps}
          onColumnResize={onColumnResize}
          showCellVerticalBorder
          showColumnVerticalBorder
          rowHeight={setting.datagridRowHeightXXX()}
          columnHeaderHeight={setting.datagridRowHeightXXX()}
          columns={cols}
          rows={triggers}
          onRowDoubleClick={onRowDoubleClick}
        ></DataGrid>
      </div>

      {row && (
        <Modal
          open={!!row}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setRow(null);
          })}
        >
          <Box sx={modalStyle} width="80%">
            <ModalHeader title={`Trigger Detail`}></ModalHeader>
            <TriggerDetail
              reloadOP={async () => {
                return row;
              }}
              uuid={uuid}
              kind={kind}
            ></TriggerDetail>
          </Box>
        </Modal>
      )}
    </Stack>
  );
}

function TableStructure(props: {
  table: string;
  uuid: string;
  kind: DataSourceKind;
}) {
  const { table, uuid, kind } = props;
  const [structure, setStructure] = useState('columns');
  const structures = [{ name: 'columns' }];

  const supportStatement = [
    DataSourceKind.Cassandra,
    DataSourceKind.ClickHouse,
    DataSourceKind.LibSql,
    DataSourceKind.Sqlite,
    DataSourceKind.Mysql,
    DataSourceKind.TiDB,
    DataSourceKind.MariaDB,
    DataSourceKind.DuckDB,
  ].includes(kind);
  if (supportStatement) {
    structures.push({ name: 'statement' });
  }
  const supportIndex = [
    DataSourceKind.MariaDB,
    DataSourceKind.TiDB,
    DataSourceKind.Mysql,
    DataSourceKind.MsSql,
    DataSourceKind.LibSql,
    DataSourceKind.Sqlite,
    DataSourceKind.Oracle,
    DataSourceKind.Postgres,
    DataSourceKind.Cockroach,
    DataSourceKind.LibSql,
    DataSourceKind.Mongo,
    DataSourceKind.Cassandra,
    DataSourceKind.FireBird,
    DataSourceKind.DuckDB,
  ].includes(kind);

  if (supportIndex) {
    structures.push({ name: 'indexes' });
  }
  const supportTrigger = [
    DataSourceKind.MariaDB,
    DataSourceKind.Mysql,
    DataSourceKind.Sqlite,
    DataSourceKind.LibSql,
    DataSourceKind.Oracle,
    DataSourceKind.MsSql,
    DataSourceKind.Postgres,
  ].includes(kind);
  if (supportTrigger) {
    structures.push({ name: 'triggers' });
  }

  return (
    <Stack spacing={1}>
      <Stack direction="row" spacing={1} style={{ justifyContent: 'center' }}>
        {structures.map((e) => (
          <Chip
            onClick={() => {
              setStructure(e.name);
            }}
            key={e.name}
            color={e.name == structure ? 'success' : 'default'}
            label={e.name}
            size="medium"
          ></Chip>
        ))}
      </Stack>

      {structure == 'columns' && (
        <ColumnsStructure
          table={table}
          kind={kind}
          uuid={uuid}
        ></ColumnsStructure>
      )}
      {structure == 'indexes' && (
        <IndexesStructure
          table={table}
          kind={kind}
          uuid={uuid}
        ></IndexesStructure>
      )}
      {structure == 'triggers' && (
        <TriggersStructure
          table={table}
          kind={kind}
          uuid={uuid}
        ></TriggersStructure>
      )}
      {structure == 'statement' && (
        <StatementStructure
          table={table}
          kind={kind}
          uuid={uuid}
        ></StatementStructure>
      )}
    </Stack>
  );
}
function StatementStructure(props: {
  uuid: string;
  kind: DataSourceKind;
  table: string;
}) {
  const { uuid, kind, table } = props;
  const getValue = (sql: types.CreateTableAndIndexSql) => {
    function newLine(ret: string): string {
      if (!ret.endsWith(';')) {
        ret += ';\n';
      }
      if (!ret.endsWith('\n')) {
        ret += '\n';
      }
      return ret;
    }

    let ret = newLine(sql.create_table);
    const indexes = sql.indexes.map((x) => newLine(x));
    ret += indexes.join('');
    return ret;
  };

  return (
    <Stack>
      <DefAndMetasView
        reloadOP={async () => {
          return invoke<types.CreateTableAndIndexSql>(
            'create_table_and_index_sql',
            {
              name: uuid,
              tableName: table,
            }
          ).then((e) => {
            return {
              def: getValue(e),
              metas: [],
            } as DefAndMetas;
          });
        }}
        kind={kind}
        uuid={uuid}
      ></DefAndMetasView>
    </Stack>
  );
}

export { TableStructure };
