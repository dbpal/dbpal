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
  EscDownEvent,
  initMonacoLanague,
  modalStyle,
  SettingContext,
  SimpleResizeEditor,
} from './Common';
import {
  calMenuStyle,
  columnWidths,
  ContextMenuContext,
  datagridContainerHeight,
  DataSourceKind,
  handleCopyToClipboard,
  secondConfirm,
  VisiableContext,
} from './helper';
import CloseIcon from '@mui/icons-material/Close';
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
  InputLabel,
  ListItemIcon,
  ListItemText,
  MenuItem,
  MenuList,
  Modal,
  Paper,
  Radio,
  RadioGroup,
  Select,
  Stack,
  TextField,
  Typography,
} from '@mui/material';
import ContentCopyIcon from '@mui/icons-material/ContentCopy';
import EditIcon from '@mui/icons-material/Edit';
import DeleteForeverIcon from '@mui/icons-material/DeleteForever';
import AddIcon from '@mui/icons-material/Add';
import { BSON } from 'bson';
import { MongoBsonEditor } from './MongoDB';
import { TriggerDetail } from './TriggerDetail';
import { TrueOrFalse } from './CommonComp';

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
  const supportModifyColumn =
    kind == DataSourceKind.MariaDB ||
    kind == DataSourceKind.MsSql ||
    kind == DataSourceKind.Mysql ||
    kind == DataSourceKind.Postgres ||
    kind == DataSourceKind.Cockroach ||
    kind == DataSourceKind.Oracle ||
    kind == DataSourceKind.TiDB ||
    kind == DataSourceKind.Cassandra ||
    kind == DataSourceKind.ClickHouse ||
    kind == DataSourceKind.DuckDB;

  const onColumnResize = useCallback(({ colDef, width }) => {
    columnWidths.saveColumnWidths(colDef.field, width);
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
        field: 'typ',
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
    const width = columnWidths.getWidth(e.field);
    if (width) {
      e.width = width;
    }
  });
  const vis = useContext(VisiableContext);
  const { setUpMenu } = useContext(ContextMenuContext);

  function Menu(props: { col: string; content: string }) {
    const { col, content } = props;
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
      </Paper>
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
            style: calMenuStyle(e, 100),
          });
        },
        style: { cursor: 'context-menu' },
      },
    } as GridSlotsComponentsProps;
  }, []);

  const getRowId = useCallback((e) => {
    return e['field'];
  }, []);

  const onRowDoubleClick = useCallback((row) => {
    setRow(row.row);
  }, []);
  return (
    <Stack spacing={1}>
      <Stack direction="row" spacing={1} style={{ alignItems: 'center' }}>
        <Button loading={loading} onClick={handleReload} variant="contained">
          reload
        </Button>

        {supportModifyColumn && (
          <Button
            onClick={() => {
              setShowAdd(true);
            }}
            variant="contained"
          >
            <AddIcon></AddIcon>
          </Button>
        )}
        {supportModifyColumn && (
          <Typography color="info"> double click a row to modify. </Typography>
        )}
      </Stack>
      {vis && (
        <div
          style={{
            display: 'grid',
            height: datagridContainerHeight(fields.length),
          }}
        >
          <DataGrid
            getRowId={getRowId}
            slotProps={slotProps}
            onColumnResize={onColumnResize}
            showCellVerticalBorder
            showColumnVerticalBorder
            rowHeight={setting.datagridRowHeight}
            columnHeaderHeight={setting.datagridHeaderHeight}
            columns={cols}
            rows={fields}
            onRowDoubleClick={onRowDoubleClick}
          ></DataGrid>
        </div>
      )}
      {showAdd && (
        <Modal
          open={showAdd}
          onClose={(e: EscDownEvent, reason) => {
            if (reason === 'backdropClick') {
              return;
            }

            e.preventDefault();
            setShowAdd(false);
          }}
        >
          <Box sx={modalStyle} width="60%">
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
      {supportModifyColumn && row && (
        <Modal
          open={!!row}
          onClose={(e: EscDownEvent, reason) => {
            if (reason === 'backdropClick') {
              return;
            }

            e.preventDefault();
            setRow(null);
          }}
        >
          <Box sx={modalStyle} width="60%">
            <ModifyTableColumn
              table={table}
              uuid={uuid}
              handleClose={() => {
                setRow(null);
              }}
              col={row.field}
              typ={row.typ}
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
        size="small"
        label="new name"
        value={newName}
        onChange={(e) => {
          setNewname(e.target.value);
        }}
      ></TextField>
      <Stack direction="row" spacing={1}>
        <Button
          color="primary"
          variant="contained"
          loading={loading}
          onClick={handleRename}
        >
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
    columnWidths.saveColumnWidths(colDef.field, width);
  }, []);

  const handleReload = () => {
    setLoading(true);
    invoke<types.Index[]>('show_indexes', {
      name: uuid,
      tableName: table,
    })
      .then((fs) => {
        setIndexes(fs);
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
    const width = columnWidths.getWidth(e.field);
    if (width) {
      e.width = width;
    }
  });
  const vis = useContext(VisiableContext);
  const { setUpMenu } = useContext(ContextMenuContext);

  function Menu(props: { content: string; indexName: string }) {
    const { content, indexName } = props;

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
      </Paper>
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
            style: calMenuStyle(e, 100),
          });
        },
        style: { cursor: 'context-menu' },
      },
    } as GridSlotsComponentsProps;
  }, []);

  const getRowId = useCallback((e) => {
    return e['name'];
  }, []);

  let addIndex;

  switch (kind) {
    case DataSourceKind.Mongo:
      addIndex = (
        <MongoAddIndex
          uuid={uuid}
          table={table}
          handleClose={() => {
            setShowAdd(false);
          }}
          realod={handleReload}
        ></MongoAddIndex>
      );
      break;

    default:
      addIndex = (
        <AddIndex
          uuid={uuid}
          table={table}
          handleClose={() => {
            setShowAdd(false);
          }}
          kind={kind}
          realod={handleReload}
        ></AddIndex>
      );
  }

  return (
    <Stack spacing={1}>
      <Stack direction="row" spacing={1}>
        <Button loading={loading} onClick={handleReload} variant="contained">
          reload
        </Button>

        <Button
          onClick={() => {
            setShowAdd(true);
          }}
          variant="contained"
        >
          <AddIcon></AddIcon>
        </Button>
      </Stack>

      {vis && (
        <div
          style={{
            display: 'grid',
            height: datagridContainerHeight(indexes.length),
          }}
        >
          <DataGrid
            getRowId={getRowId}
            slotProps={slotProps}
            onColumnResize={onColumnResize}
            showCellVerticalBorder
            showColumnVerticalBorder
            rowHeight={setting.datagridRowHeight}
            columnHeaderHeight={setting.datagridHeaderHeight}
            columns={cols}
            rows={indexes}
          ></DataGrid>
        </div>
      )}
      {showAdd && (
        <Modal
          open={showAdd}
          onClose={(e: EscDownEvent, reason) => {
            if (reason === 'backdropClick') {
              return;
            }

            e.preventDefault();
            setShowAdd(false);
          }}
        >
          <Box sx={modalStyle} width="60%">
            {addIndex}
          </Box>
        </Modal>
      )}
      {renameIndexName && (
        <Modal
          open={!!renameIndexName}
          onClose={(e: EscDownEvent, reason) => {
            if (reason === 'backdropClick') {
              return;
            }
            e.preventDefault();
            setRenameIndexName('');
          }}
        >
          <Box sx={modalStyle} width="60%">
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

function MongoAddIndex(props: { uuid; table; handleClose; realod }) {
  const { uuid, table, handleClose, realod } = props;

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
      realod();
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
        size="small"
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
      <MongoBsonEditor
        doc={{}}
        height="250px"
        buttons={[
          {
            buttonText: 'create index',
            callBack: (keys) => {
              handleAdd(keys);
            },
          },
        ]}
      ></MongoBsonEditor>
    </Stack>
  );
}

function AddIndex(props: {
  uuid;
  table;
  handleClose;
  realod;
  kind: DataSourceKind;
}) {
  let { uuid, table, handleClose, realod, kind } = props;

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
      realod();
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
        size="small"
        required
      ></TextField>
      <Stack
        direction="row"
        style={{ flexWrap: 'wrap', alignItems: 'center' }}
        spacing={1}
      >
        <Typography> includes: </Typography>
        {includes.map((x) => (
          <Chip
            onClick={() => {
              setIncludes((e) => e.filter((e) => e != x));
            }}
            size="small"
            key={x}
            color="success"
            label={x}
          ></Chip>
        ))}
      </Stack>

      <Stack
        direction="row"
        style={{ flexWrap: 'wrap', alignItems: 'center' }}
        spacing={1}
      >
        <Typography> candidates: </Typography>
        {fields
          .filter((f) => includes.find((e) => e == f.field) == undefined)
          .map((x) => (
            <Chip
              size="small"
              key={x.field}
              onClick={() => {
                setIncludes((e) => [...e, x.field]);
              }}
              color="default"
              label={x.field}
            ></Chip>
          ))}
      </Stack>

      {indexTypeJsx}
      <Divider></Divider>
      <Stack direction="row">
        <Button onClick={handleAdd} variant="contained" color="primary">
          add
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
      showNotification('error', 'type name is empty');
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
        size="small"
        value={name}
        required
        inputRef={nameDom}
        onChange={(e) => {
          setName(e.target.value);
        }}
        label="column name"
      ></TextField>
      <TableTypes
        uuid={uuid}
        handleChange={(e) => {
          setTyp(e);
        }}
      ></TableTypes>
      {/*   TODO some mariadb require a length parameter    */}
      <Stack direction="row" spacing={1}>
        <TextField
          fullWidth
          size="small"
          value={typ}
          onChange={(e) => setTyp(e.target.value)}
          label="type"
          helperText={typeHint}
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
        <Stack>
          <TextField
            size="small"
            value={defaultValue}
            onChange={(e) => setDefaultValue(e.target.value)}
            label="default value"
          ></TextField>
        </Stack>
      )}

      {supportAfter && (
        <FormControl style={{ minWidth: '180px' }}>
          <InputLabel>Select After Column</InputLabel>
          <Select
            size="small"
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

      <Divider></Divider>
      <Stack direction="row">
        <Button
          loading={loading}
          variant="contained"
          color="primary"
          onClick={handleAdd}
        >
          add
        </Button>
      </Stack>
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
  let supoortModifyType = kind != DataSourceKind.Cassandra;
  return (
    <Stack spacing={1}>
      <TextField
        size="small"
        fullWidth
        label="New Name"
        value={newName}
        onChange={(e) => {
          setNewName(e.target.value);
        }}
        required
        error={!newName}
      ></TextField>
      {supoortModifyType && !modifyType && (
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
          ></TableTypes>

          <Stack direction="row" spacing={1} style={{ alignItems: 'center' }}>
            <TextField
              size="small"
              label="Type"
              value={newTyp}
              onChange={(e) => {
                setNewTyp(e.target.value);
              }}
              fullWidth
              helperText={typeHint}
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
        <Button
          loading={loading}
          color="error"
          variant="contained"
          onClick={handleModify}
        >
          modify
        </Button>
      </Stack>
    </Stack>
  );
}

function TableTypes(props: { handleChange; uuid }) {
  let { handleChange, uuid } = props;
  const [types, setTypes] = useState([]);
  const [show, setShow] = useState(false);
  useEffect(() => {
    invoke<string[]>('list_data_types', { name: uuid })
      .then((ts) => {
        setTypes(ts);
      })
      .catch((e) => {
        console.log('should not return a error', e);
      });
  }, []);
  const toggleShow = () => {
    setShow((e) => !e);
  };
  return (
    <Stack direction="row" spacing={1} style={{ flexWrap: 'wrap' }}>
      <Button onClick={toggleShow}>
        {show && <CloseIcon></CloseIcon>} all types
      </Button>
      {show &&
        types.map((t) => (
          <Chip
            color="info"
            key={t}
            style={{ margin: '1px' }}
            size="small"
            onClick={() => {
              handleChange(t);
            }}
            label={t}
          ></Chip>
        ))}
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
  initMonacoLanague(uuid, kind);
  const setting = useContext(SettingContext);
  const [loading, setLoading] = useState(false);
  const [triggers, setTriggers] = useState<types.TriggerDetail[]>([]);
  const [row, setRow] = useState(null);
  const { showNotification } = useContext(NotificationContext);

  const onColumnResize = useCallback(({ colDef, width }) => {
    columnWidths.saveColumnWidths(colDef.field, width);
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

  const handleEnabless = (invoke_name: string, trigger, operation: string) => {
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
    const width = columnWidths.getWidth(e.field);
    if (width) {
      e.width = width;
    }
  });
  const vis = useContext(VisiableContext);
  const { setUpMenu } = useContext(ContextMenuContext);

  function Menu(props: { content: string; detail: types.TriggerDetail }) {
    const { content, detail } = props;
    let enable;
    if (detail.enable) {
      enable = triggerIsEnable(detail.enable) ? (
        <MenuItem
          sx={{ color: 'error.main' }}
          onClick={() => {
            handleEnabless('disable_trigger', detail.name, 'Disable trigger');
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
            handleEnabless('enable_trigger', detail.name, 'Enable trigger');
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
      </Paper>
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
            style: calMenuStyle(e, 100),
          });
        },
        style: { cursor: 'context-menu' },
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
        <Button loading={loading} onClick={handleReload} variant="contained">
          reload
        </Button>
        {triggers.length > 0 && (
          <Typography color="info">double click a row to see detail</Typography>
        )}
      </Stack>

      {vis && (
        <div
          style={{
            display: 'grid',
            height: datagridContainerHeight(triggers.length),
          }}
        >
          <DataGrid
            getRowId={getRowId}
            slotProps={slotProps}
            onColumnResize={onColumnResize}
            showCellVerticalBorder
            showColumnVerticalBorder
            rowHeight={setting.datagridRowHeight}
            columnHeaderHeight={setting.datagridHeaderHeight}
            columns={cols}
            rows={triggers}
            onRowDoubleClick={onRowDoubleClick}
          ></DataGrid>
        </div>
      )}

      {row && (
        <Modal
          open={!!row}
          onClose={(e: EscDownEvent, reason) => {
            if (reason === 'backdropClick') {
              return;
            }
            e.preventDefault();
            setRow(null);
          }}
        >
          <Box sx={modalStyle} width="80%">
            <TriggerDetail
              reloadOP={async () => {
                return Promise.resolve(row);
              }}
              uuid={uuid}
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
  const structures = [{ name: 'columns', color: 'secondary' }, ,];

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
    structures.push({ name: 'statement', color: 'primary' });
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
    structures.push({ name: 'indexes', color: 'success' });
  }
  const supportTrigger = [
    DataSourceKind.MariaDB,
    DataSourceKind.Mysql,
    DataSourceKind.Sqlite,
    DataSourceKind.Oracle,
    DataSourceKind.MsSql,
    DataSourceKind.Postgres,
  ].includes(kind);
  if (supportTrigger) {
    structures.push({ name: 'triggers', color: 'info' });
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
            //@ts-ignore
            color={e.name == structure ? e.color : 'default'}
            label={e.name}
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
  const [sql, setSql] = useState<types.CreateTableAndIndexSql>(null);
  const [loading, setLoading] = useState(false);
  const { showNotification } = useContext(NotificationContext);
  const handleReload = () => {
    setLoading(true);
    invoke<types.CreateTableAndIndexSql>('create_table_and_index_sql', {
      name: uuid,
      tableName: table,
    })
      .then((e) => {
        setSql(e);
      })
      .catch((e) => {
        showNotification('error', 'fetch create sql failed', e);
      })
      .finally(() => {
        setLoading(false);
      });
  };
  useEffect(() => {
    handleReload();
  }, []);

  const getValue = () => {
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
  initMonacoLanague(uuid, kind);
  return (
    <Stack spacing={1}>
      <Stack direction="row">
        <Button
          loading={loading}
          variant="contained"
          onClick={() => {
            handleReload();
          }}
        >
          reload
        </Button>
      </Stack>
      {sql && (
        <SimpleResizeEditor
          language={uuid}
          value={getValue()}
          readOnly={true}
        ></SimpleResizeEditor>
      )}
    </Stack>
  );
}

export { TableStructure };
