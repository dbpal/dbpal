"use strict";
import {
  CSSProperties,
  ReactNode,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import types from "./ApiStruct";
import { DefAndMetas, EscDownEvent, modalHandleClose, modalStyle } from "./Common";
import {
  ColumnWidth,
  ContextMenuContext,
  dataGridStyle,
  DataSourceKind,
  extractFieldAndIDFromMouseEvent,
  handleCopyToClipboard,
  saveAndSetSize,
  secondConfirm,
  SettingContext,
} from "./helper";
import CancelIcon from "@mui/icons-material/Cancel";
import { NotificationContext } from "./Notification";
import { invoke } from "@tauri-apps/api/core";
import CheckCircleIcon from "@mui/icons-material/CheckCircle";
import { DataGrid, GridColDef, GridSlotsComponentsProps } from "@mui/x-data-grid";
import {
  Autocomplete,
  Box,
  Button,
  Checkbox,
  Chip,
  Divider,
  FormControlLabel,
  FormLabel,
  IconButton,
  InputAdornment,
  ListItemIcon,
  ListItemText,
  MenuItem,
  MenuList,
  Modal,
  Radio,
  RadioGroup,
  Stack,
  TextField,
  Tooltip,
  Typography,
} from "@mui/material";
import ContentCopyIcon from "@mui/icons-material/ContentCopy";
import EditIcon from "@mui/icons-material/Edit";
import DeleteForeverIcon from "@mui/icons-material/DeleteForever";
import AddIcon from "@mui/icons-material/Add";
import { BSON } from "bson";
import { MongoValueEditor } from "./MongoDB";
import { ModalHeader, SelectColumns, TrueOrFalse } from "./CommonComp";
import ClearIcon from "@mui/icons-material/Clear";
import { DefAndMetasView, TriggerDetail } from "./UdfAndMetaView";
import {
  createTableAndIndexSql,
  formatCreateTableAndIndexSql,
  showIndexes,
  showTableColumn,
  showTrigger,
} from "./handlers";

function ColumnMenu(props: {
  col: string;
  content: string;
  supportModifyColumn;
  handleDrop;
}) {
  const { showNotification } = useContext(NotificationContext);
  const { col, content, supportModifyColumn, handleDrop } = props;
  return (
    <MenuList>
      {content && (
        <MenuItem
          onClick={() => {
            handleCopyToClipboard(content, showNotification);
          }}
        >
          <ListItemIcon>
            <ContentCopyIcon />
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
            <DeleteForeverIcon />
          </ListItemIcon>
          <ListItemText> drop </ListItemText>
        </MenuItem>
      )}
    </MenuList>
  );
}

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
    DataSourceKind.SqlServer,
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

  const handleReload = useCallback(() => {
    setLoading(true);
    showTableColumn(uuid, table)
      .then(fs => {
        setFields(fs);
      })
      .catch(e => {
        showNotification("error", "list column failed", e);
      })
      .finally(() => {
        setLoading(false);
      });
  }, [showNotification, table, uuid]);

  const handleDrop = useCallback(
    async (col: string) => {
      const confirm = await secondConfirm({
        title: `Drop Column '${col}'`,
        content: "",
        confirmText: "DROP COLUMN",
        kind: "error",
      });
      if (!confirm) {
        return;
      }
      invoke("drop_table_column", {
        name: uuid,
        tableName: table,
        columnName: col,
      })
        .then(() => {
          showNotification("success", "Drop column OK");
          handleReload();
        })
        .catch(e => {
          showNotification("error", "Drop column Failure", e);
        });
    },
    [handleReload, showNotification, table, uuid],
  );

  const [showAdd, setShowAdd] = useState(false);

  useEffect(() => {
    handleReload();
  }, [handleReload]);
  const cols = useMemo(() => {
    return [
      { field: "field" },
      {
        field: "data_type",
        renderHeader: () => {
          return "type";
        },
      },
      {
        field: "nullable",
        renderCell: row => {
          return <TrueOrFalse value={row.row["nullable"]} />;
        },
      },
      {
        field: "is_primary",
        renderHeader: () => {
          return "primary key";
        },
        renderCell: row => {
          return <TrueOrFalse value={row.row["is_primary"]} />;
        },
      },
      {
        field: "is_auto_increment",
        renderHeader: () => {
          return "auto increment";
        },
        renderCell: row => {
          return <TrueOrFalse value={row.row["is_auto_increment"]} />;
        },
      },
      {
        field: "default_value",
        renderHeader: () => {
          return "default value";
        },
      },
      {
        field: "sortable",
        renderCell: row => {
          return <TrueOrFalse value={row.row["sortable"]} />;
        },
      },
    ] as GridColDef[];
  }, []);
  cols.forEach(e => {
    const width = ColumnWidth.instance.get(e.field);
    if (width) {
      e.width = width;
    }
  });
  const { setUpMenu } = useContext(ContextMenuContext);

  const slotProps = useMemo(() => {
    return {
      row: {
        onContextMenu: e => {
          const { target } = e;
          const { id, field } = extractFieldAndIDFromMouseEvent(e);
          if (!field) {
            return;
          }
          const content = (target as HTMLDivElement).innerHTML;
          setUpMenu({
            content: (
              <ColumnMenu
                supportModifyColumn={supportModifyColumn}
                col={id}
                content={content}
                handleDrop={handleDrop}
              />
            ),
            event: e,
          });
        },
        style: { cursor: "context-menu" },
      },
      toolbar: {
        csvOptions: {
          delimiter: setting.export.csvDelimiter,
          includeHeaders: setting.export.includeHeader,
        },
      },
    } as GridSlotsComponentsProps;
  }, [
    handleDrop,
    setUpMenu,
    setting.export.csvDelimiter,
    setting.export.includeHeader,
    supportModifyColumn,
  ]);

  const getRowId = useCallback(e => {
    return e["field"];
  }, []);

  const onRowDoubleClick = useCallback(row => {
    setRow(row.row);
  }, []);

  const [modalBox, setModalBox] = useState(null);

  useEffect(() => {
    if (modalBox) {
      const clean = saveAndSetSize(modalBox, "table-column", "width");
      return clean;
    }
  }, [modalBox]);
  useEffect(() => {
    if (row) {
      if ([DataSourceKind.Sqlite, DataSourceKind.LibSql].includes(kind)) {
        showNotification("warning", `${kind} not support modify column`);
      }
    }
  }, [kind, row, showNotification]);
  return (
    <Stack style={{ height: "100%" }}>
      <Stack direction="row" style={{ alignItems: "center" }}>
        <Button loading={loading} onClick={handleReload}>
          reload
        </Button>

        {supportModifyColumn && (
          <Button
            onClick={() => {
              setShowAdd(true);
            }}
          >
            <AddIcon />
          </Button>
        )}
        {supportModifyColumn && (
          <Tooltip title="double click a row to modify.">
            <Typography color="info">modify </Typography>
          </Tooltip>
        )}
      </Stack>

      <DataGrid
        showToolbar
        getRowId={getRowId}
        slotProps={slotProps}
        onColumnResize={onColumnResize}
        showCellVerticalBorder
        showColumnVerticalBorder
        rowHeight={setting.appearance.datagridRowHeight()}
        columnHeaderHeight={setting.appearance.datagridRowHeight()}
        columns={cols}
        rows={fields}
        onRowDoubleClick={onRowDoubleClick}
        style={dataGridStyle}
      />

      {showAdd && (
        <Modal
          open={showAdd}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setShowAdd(false);
          })}
        >
          <Box
            sx={{ resize: "horizontal", ...modalStyle }}
            width="60%"
            ref={setModalBox}
          >
            <ModalHeader
              handleClose={() => {
                setShowAdd(false);
              }}
              title="Add Column"
            />
            <TableAddColumn
              table={table}
              uuid={uuid}
              handleClose={() => {
                setShowAdd(false);
              }}
              reload={handleReload}
              kind={kind}
              fields={fields}
            />
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
              sx={{ resize: "horizontal", ...modalStyle }}
              width="60%"
              ref={setModalBox}
            >
              <ModalHeader
                title="Modify Column"
                handleClose={() => {
                  setRow(null);
                }}
              />
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
              />
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
      showNotification("warning", "name not change");
      return;
    }
    setLoading(true);

    invoke("rename_index", {
      name: uuid,
      tableName: table,
      oldName: indexName,
      newName: newName,
    })
      .then(() => {
        showNotification("success", "Rename OK");
        reload();
        handleClose();
      })
      .catch(e => {
        showNotification("error", "Rename Failure", e);
      })
      .finally(() => {
        setLoading(false);
      });
  };
  return (
    <Stack>
      <TextField
        label="new name"
        value={newName}
        onChange={e => {
          setNewname(e.target.value);
        }}
      />
      <Stack direction="row">
        <Button color="primary" loading={loading} onClick={handleRename}>
          rename
        </Button>
      </Stack>
    </Stack>
  );
}

function IndexMenu(props: {
  content: string;
  indexName: string;
  setRenameIndexName;
  handleDrop;
}) {
  const { content, indexName, setRenameIndexName, handleDrop } = props;
  const { showNotification } = useContext(NotificationContext);

  return (
    <MenuList>
      {content && (
        <MenuItem
          onClick={() => {
            handleCopyToClipboard(content, showNotification);
          }}
        >
          <ListItemIcon>
            <ContentCopyIcon />
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
          <EditIcon />
        </ListItemIcon>
        <ListItemText> rename </ListItemText>
      </MenuItem>
      <MenuItem
        onClick={() => {
          handleDrop(indexName);
        }}
      >
        <ListItemIcon>
          <DeleteForeverIcon />
        </ListItemIcon>
        <ListItemText> drop </ListItemText>
      </MenuItem>
    </MenuList>
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
  const [renameIndexName, setRenameIndexName] = useState("");
  const [showAdd, setShowAdd] = useState(false);

  const onColumnResize = useCallback(({ colDef, width }) => {
    ColumnWidth.instance.save(colDef.field, width);
  }, []);

  const handleReload = useCallback(() => {
    setLoading(true);
    showIndexes(uuid, table)
      .then(indexes => {
        setIndexes(indexes);
      })
      .catch(e => {
        showNotification("error", "list indexes failed", e);
      })
      .finally(() => {
        setLoading(false);
      });
  }, [showNotification, table, uuid]);

  const handleDrop = useCallback(
    async indexName => {
      const confirm = await secondConfirm({
        title: `drop index '${indexName}'`,
        content: "Are u sure to drop the index",
        confirmText: "Drop",
      });
      if (!confirm) {
        return;
      }
      invoke("drop_index", {
        name: uuid,
        tableName: table,
        index: indexName,
      })
        .then(() => {
          showNotification("success", "Drop OK");
          handleReload();
        })
        .catch(e => {
          showNotification("error", "Drop index failure", e);
        });
    },
    [handleReload, showNotification, table, uuid],
  );
  useEffect(() => {
    handleReload();
  }, [handleReload]);
  const cols = useMemo(() => {
    return [{ field: "name" }, { field: "kind" }, { field: "columns" }] as GridColDef[];
  }, []);
  cols.forEach(e => {
    const width = ColumnWidth.instance.get(e.field);
    if (width) {
      e.width = width;
    }
  });

  const { setUpMenu } = useContext(ContextMenuContext);

  const slotProps = useMemo(() => {
    return {
      row: {
        onContextMenu: e => {
          const { currentTarget, target } = e;
          {
            const { field } = extractFieldAndIDFromMouseEvent(e);
            if (!field) {
              return;
            }
          }
          const content = (target as HTMLDivElement).innerHTML;
          const indexName = (currentTarget as HTMLDivElement).getAttribute("data-id");
          setUpMenu({
            content: (
              <IndexMenu
                indexName={indexName}
                setRenameIndexName={setRenameIndexName}
                handleDrop={handleDrop}
                content={content}
              />
            ),
            event: e,
          });
        },
        style: { cursor: "context-menu" },
      },
      toolbar: {
        csvOptions: {
          delimiter: setting.export.csvDelimiter,
          includeHeaders: setting.export.includeHeader,
        },
      },
    } as GridSlotsComponentsProps;
  }, [
    handleDrop,
    setUpMenu,
    setting.export.csvDelimiter,
    setting.export.includeHeader,
  ]);

  const getRowId = useCallback(e => {
    return e["name"];
  }, []);

  const [modalBox, setModalBox] = useState(null);
  useEffect(() => {
    if (modalBox) {
      const c = saveAndSetSize(
        modalBox,
        "create-index" + (kind == DataSourceKind.Mongo ? "-mongo" : "-other"),
        kind == DataSourceKind.Mongo ? "both" : "width",
      );
      return c;
    }
  }, [kind, modalBox]);
  return (
    <Stack style={{ height: "100%" }}>
      <Stack direction="row">
        <Button loading={loading} onClick={handleReload}>
          reload
        </Button>

        <Button
          onClick={() => {
            setShowAdd(true);
          }}
        >
          <AddIcon />
        </Button>
      </Stack>

      <DataGrid
        showToolbar
        getRowId={getRowId}
        slotProps={slotProps}
        onColumnResize={onColumnResize}
        showCellVerticalBorder
        showColumnVerticalBorder
        rowHeight={setting.appearance.datagridRowHeight()}
        columnHeaderHeight={setting.appearance.datagridRowHeight()}
        columns={cols}
        rows={indexes}
        style={dataGridStyle}
      />

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
              resize: "both",
              ...modalStyle,
            }}
            width="60%"
            ref={setModalBox}
            height={kind == DataSourceKind.Mongo ? "40vh" : "auto"}
          >
            <ModalHeader
              title="create index"
              handleClose={() => {
                setShowAdd(false);
              }}
            />
            <MongoAddIndex
              uuid={uuid}
              table={table}
              handleClose={() => {
                setShowAdd(false);
              }}
              reload={handleReload}
            />
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
              resize: "horizontal",
              ...modalStyle,
            }}
            width="60%"
            ref={setModalBox}
          >
            <ModalHeader
              title="create index"
              handleClose={() => {
                setShowAdd(false);
              }}
            />
            <AddIndex
              uuid={uuid}
              table={table}
              handleClose={() => {
                setShowAdd(false);
              }}
              kind={kind}
              reload={handleReload}
            />
          </Box>
        </Modal>
      )}

      {renameIndexName && (
        <Modal
          open={!!renameIndexName}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setRenameIndexName("");
          })}
        >
          <Box sx={modalStyle} width="40%">
            <ModalHeader
              title={`Rename Index`}
              handleClose={() => {
                setRenameIndexName("");
              }}
            />
            <RenameIndex
              uuid={uuid}
              table={table}
              indexName={renameIndexName}
              handleClose={() => {
                setRenameIndexName("");
              }}
              reload={handleReload}
            />
          </Box>
        </Modal>
      )}
    </Stack>
  );
}

function MongoAddIndex(props: { uuid; table; handleClose; reload }) {
  const { uuid, table, handleClose, reload } = props;

  const indexTypes = ["normal", "unique"];
  const [indexType, setIndexType] = useState(indexTypes[0]);
  const [indexName, setIndexName] = useState("");
  const indexNameDom = useRef<HTMLInputElement>(null);

  const { showNotification } = useContext(NotificationContext);
  const handleAdd = async keys => {
    try {
      if (!indexName) {
        showNotification("warning", "index name can't be none");
        indexNameDom.current.focus();
        return;
      }
      await invoke("mongo_create_index", {
        name: uuid,
        tableName: table,
        keys: BSON.serialize(keys),
        unique: indexType == "unique",
        indexName: indexName ? indexName : null,
      });
      showNotification("success", "create index ok");
      handleClose();
      reload();
    } catch (e) {
      showNotification("error", "create index failed", e);
    }
  };

  return (
    <Stack flex={1}>
      <TextField
        value={indexName}
        onChange={e => {
          setIndexName(e.target.value);
        }}
        inputRef={indexNameDom}
        label="index name"
        required
      />

      <RadioGroup
        row
        onChange={e => {
          setIndexType(e.target.value);
        }}
        value={indexType}
      >
        {indexTypes.map(e => (
          <FormControlLabel key={e} value={e} control={<Radio />} label={e} />
        ))}
      </RadioGroup>
      <MongoValueEditor
        setOP={async doc => {
          handleAdd(doc);
        }}
        doc={{}}
        setName="create index"
      />
    </Stack>
  );
}

function AddIndex(props: { uuid; table; handleClose; reload; kind: DataSourceKind }) {
  const { uuid, table, handleClose, reload, kind } = props;

  const [fields, setFields] = useState<types.Field[]>([]);
  const [includes, setIncludes] = useState<string[]>([]);
  const { showNotification } = useContext(NotificationContext);
  const indexTypes = ["normal", "unique"];
  const [indexType, setIndexType] = useState(indexTypes[0]);
  const [indexName, setIndexName] = useState("");
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
            onChange={e => {
              setIndexType(e.target.value);
            }}
            value={indexType}
          >
            {indexTypes.map(e => (
              <FormControlLabel key={e} value={e} control={<Radio />} label={e} />
            ))}
          </RadioGroup>
        </>
      );
      break;
  }

  useEffect(() => {
    showTableColumn(uuid, table)
      .then(e => {
        setFields(e);
      })
      .catch(e => {
        showNotification("error", "list fields failed", e);
      });
  }, [showNotification, table, uuid]);
  const handleAdd = async () => {
    if (!indexName) {
      showNotification("warning", "index name can't be none");
      indexNameDom.current.focus();
      return;
    }
    if (includes.length == 0) {
      showNotification("warning", "select at least one column");
      return;
    }
    try {
      await invoke("create_index", {
        name: uuid,
        tableName: table,
        fields: includes,
        indexType,
        indexName: indexName ? indexName : null,
      });
      showNotification("success", "create index ok");
      handleClose();
      reload();
    } catch (e) {
      showNotification("error", "create index failed", e);
    }
  };

  return (
    <Stack>
      <TextField
        value={indexName}
        onChange={e => {
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
                    setIndexName("");
                  }}
                  edge="end"
                >
                  <ClearIcon fontSize="small" />
                </IconButton>
              </InputAdornment>
            ),
          },
        }}
      />
      <SelectColumns
        includeAll={false}
        includes={includes}
        setIncludes={setIncludes}
        fields={fields.map(e => e.field)}
      />
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

function TableAddColumn(props: {
  table: string;
  uuid;
  handleClose;
  reload;
  kind: DataSourceKind;
  fields: types.Field[];
}) {
  const { table, uuid, handleClose, reload, kind, fields } = props;
  const [name, setName] = useState("");
  const [nullness, setNullness] = useState("");
  const [typ, setTyp] = useState("");
  const { showNotification } = useContext(NotificationContext);
  const [after, setAfter] = useState("");
  const [defaultValue, setDefaultValue] = useState("");
  const supportAfter =
    kind == DataSourceKind.Mysql ||
    kind == DataSourceKind.MariaDB ||
    kind == DataSourceKind.TiDB;
  const nameDom = useRef(null);
  const [loading, setLoading] = useState(false);
  const handleAdd = () => {
    if (!name) {
      showNotification("error", "column name is empty");
      nameDom.current.focus();
      return;
    }
    if (!typ) {
      showNotification("warning", "type is empty");
      return;
    }
    setLoading(true);
    invoke("add_table_column", {
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
        showNotification("success", "Add Column OK");
        handleClose();
        reload();
      })
      .catch(e => {
        showNotification("error", "Add Column failure", e);
      })
      .finally(() => {
        setLoading(false);
      });
  };

  const supportNull = kindSupportNull(kind);
  const supportDefault = kindSupportDefault(kind);
  return (
    <Stack>
      <TextField
        value={name}
        required
        inputRef={nameDom}
        onChange={e => {
          setName(e.target.value.trim());
        }}
        slotProps={{
          input: {
            endAdornment: name && (
              <InputAdornment position="end">
                <IconButton
                  onClick={() => {
                    setName("");
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
      />
      <Stack direction="row" style={{ alignItems: "center" }}>
        <TypeSelect
          uuid={uuid}
          handleChange={e => {
            setTyp(e);
          }}
          style={{ flex: 1 }}
          type={typ}
        />
        {supportNull && (
          <ColumnNullness
            nullness={nullness}
            handleNullness={e => {
              setNullness(e);
            }}
          />
        )}
      </Stack>

      {supportDefault && (
        <SqlDefaultValue
          uuid={uuid}
          defaultValue={defaultValue}
          setDefaultValue={setDefaultValue}
        />
      )}
      {supportAfter && (
        <Autocomplete
          freeSolo={false}
          options={fields.map(e => e.field)}
          style={{ width: "22rem" }}
          value={after}
          renderInput={params => <TextField {...params} label="after column" />}
          onChange={(_, field) => {
            setAfter(field);
          }}
        />
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

function SqlDefaultValue(props: { uuid: string; defaultValue; setDefaultValue }) {
  const { uuid, defaultValue, setDefaultValue } = props;
  const [currentValue, setCurrentValue] = useState<string[]>([]);
  const { showNotification } = useContext(NotificationContext);
  useEffect(() => {
    invoke<string[]>("list_sql_current_values", { name: uuid })
      .then(ts => {
        setCurrentValue(ts);
      })
      .catch(e => {
        showNotification("success", "list sql current values failed", e);
      });
  }, [showNotification, uuid]);

  return (
    <Stack direction="row">
      <TextField
        label="default value"
        value={defaultValue}
        onChange={e => {
          setDefaultValue(e.target.value);
        }}
        style={{ flex: 1 }}
        slotProps={{
          input: {
            endAdornment: defaultValue && (
              <InputAdornment position="end">
                <IconButton
                  onClick={() => {
                    setDefaultValue("");
                  }}
                  edge="end"
                >
                  <ClearIcon fontSize="small" />
                </IconButton>
              </InputAdornment>
            ),
          },
        }}
      />
      <Autocomplete
        freeSolo
        options={currentValue}
        style={{ width: "22rem" }}
        value={defaultValue}
        renderInput={params => (
          <TextField {...params} label="some predefined value can pick" />
        )}
        onChange={(_, value) => {
          setDefaultValue(value);
        }}
      />
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
  const { uuid, table, col, handleClose, reload, typ, kind } = props;
  const [loading, setLoading] = useState(false);
  const [newName, setNewName] = useState(col);
  const [newTyp, setNewTyp] = useState(typ);
  const { showNotification } = useContext(NotificationContext);
  const [modifyType, setModifyType] = useState(false);
  const [nullness, setNullness] = useState("");
  const handleModify = () => {
    if (newName == col && !newTyp) {
      showNotification("warning", "nothing is changed");
      return;
    }
    if (!newName) {
      showNotification("error", "name can't empty");
      return;
    }
    setLoading(true);
    let typePara = null;
    if (typ != newTyp) {
      typePara = { typ: newTyp, null: nullness ? nullness : nullness };
    }
    invoke("modify_table_column", {
      name: uuid,
      tableName: table,
      oldName: col,
      newName: newName,
      typ: typePara,
    })
      .then(() => {
        showNotification("success", "Modify column OK");
        reload();
        handleClose();
      })
      .catch(e => {
        showNotification("error", "Modify column Failure", e);
      })
      .finally(() => {
        setLoading(false);
      });
  };
  const supportModifyType = kind != DataSourceKind.Cassandra;
  return (
    <Stack>
      <TextField
        fullWidth
        label="New Name"
        value={newName}
        onChange={e => {
          setNewName(e.target.value);
        }}
        required
        slotProps={{
          input: {
            endAdornment: newName && (
              <InputAdornment position="end">
                <IconButton
                  onClick={() => {
                    setNewName("");
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
      />
      {supportModifyType && !modifyType && (
        <FormControlLabel
          control={
            <Checkbox
              checked={modifyType}
              onChange={e => {
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
        <Stack direction="row" style={{ alignItems: "center" }}>
          <TypeSelect
            uuid={uuid}
            handleChange={e => {
              setNewTyp(e);
            }}
            style={{ flex: 1 }}
            type={typ}
          />
          {kindSupportNull(kind) && (
            <ColumnNullness nullness={nullness} handleNullness={setNullness} />
          )}
        </Stack>
      )}
      <Stack direction="row">
        <Button loading={loading} color="error" onClick={handleModify}>
          modify
        </Button>
      </Stack>
    </Stack>
  );
}

function TypeSelect(props: { handleChange; uuid; type; style?: CSSProperties }) {
  const { handleChange, uuid, type, style } = props;
  const [types, setTypes] = useState<string[]>([]);
  const { showNotification } = useContext(NotificationContext);
  useEffect(() => {
    invoke<string[]>("list_data_types", { name: uuid })
      .then(ts => {
        setTypes(ts);
      })
      .catch(e => {
        showNotification("error", "should not return a error", e);
      });
  }, [showNotification, uuid]);
  return (
    <Autocomplete
      freeSolo
      options={types}
      style={{ width: "22rem", ...style }}
      value={type}
      renderInput={params => (
        <TextField {...params} label={`type def may require size varchar(100)`} />
      )}
      onChange={(_, type) => {
        handleChange(type);
      }}
    />
  );
}

function ColumnNullness({ nullness, handleNullness }) {
  const xxx = [
    { name: "none", value: "" },
    { name: "null", value: "null" },
    {
      name: "not null",
      value: "not null",
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
      {xxx.map(e => (
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
  if (enable.kind == "postgres") {
    return enable.enable.toUpperCase() != "D";
  }
  if (enable.kind == "bool") {
    return enable.enable;
  }
}

function TriggerMenu(props: {
  content: string;
  detail: types.TriggerDetail;
  handleEnables;
  handleDrop;
}) {
  const { content, detail, handleEnables, handleDrop } = props;
  const { showNotification } = useContext(NotificationContext);
  let enable: ReactNode;
  if (detail.enable) {
    enable = triggerIsEnable(detail.enable) ? (
      <MenuItem
        sx={{ color: "error.main" }}
        onClick={() => {
          handleEnables("disable_trigger", detail.name, "Disable trigger");
        }}
      >
        <ListItemIcon>
          <CancelIcon />
        </ListItemIcon>
        <ListItemText>Disable</ListItemText>
      </MenuItem>
    ) : (
      <MenuItem
        onClick={() => {
          handleEnables("enable_trigger", detail.name, "Enable trigger");
        }}
      >
        <ListItemIcon>
          <CheckCircleIcon />
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
            <ContentCopyIcon />
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
          <DeleteForeverIcon />
        </ListItemIcon>
        <ListItemText> drop </ListItemText>
      </MenuItem>
    </MenuList>
  );
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

  const handleReload = useCallback(() => {
    setLoading(true);
    showTrigger(uuid, table)
      .then(ts => {
        setTriggers(ts);
      })
      .catch(e => {
        showNotification("error", "list indexes failed", e);
      })
      .finally(() => {
        setLoading(false);
      });
  }, [showNotification, table, uuid]);
  const callPara = useMemo(() => {
    return { name: uuid, tableName: table };
  }, [table, uuid]);

  const handleDrop = useCallback(
    async trigger => {
      const confirm = await secondConfirm({
        title: `Drop trigger '${trigger}'`,
        confirmText: "Drop",
        kind: "error",
      });
      if (!confirm) {
        return;
      }
      invoke("drop_trigger", { trigger, ...callPara })
        .then(() => {
          showNotification("success", "Drop Trigger OK");
          handleReload();
        })
        .catch(e => {
          showNotification("error", "Drop trigger failure", e);
        });
    },
    [callPara, handleReload, showNotification],
  );

  const handleEnables = useCallback(
    (invoke_name: string, trigger, operation: string) => {
      invoke(invoke_name, { trigger, ...callPara })
        .then(() => {
          showNotification("error", `${operation}  OK`);
          handleReload();
        })
        .catch(e => {
          showNotification("error", `${operation}  Failure`, e);
        });
    },
    [callPara, handleReload, showNotification],
  );

  useEffect(() => {
    handleReload();
  }, [handleReload]);
  const cols = useMemo(() => {
    return [
      { field: "name" },
      { field: "event" },
      { field: "sql" },
      {
        field: "enable",
        renderCell: r => {
          const enable = r.row.enable as types.TriggerEnable;
          if (!enable) {
            return;
          }
          switch (enable.kind) {
            case "postgres":
              return enable.enable;
            case "bool":
              return <TrueOrFalse value={enable.enable} />;
          }
        },
      },
    ] as GridColDef[];
  }, []);
  cols.forEach(e => {
    const width = ColumnWidth.instance.get(e.field);
    if (width) {
      e.width = width;
    }
  });
  const { setUpMenu } = useContext(ContextMenuContext);

  const slotProps = useMemo(() => {
    return {
      row: {
        onContextMenu: e => {
          const { currentTarget, target } = e;
          {
            const { field } = extractFieldAndIDFromMouseEvent(e);
            if (!field) {
              return;
            }
          }
          const content = (target as HTMLDivElement).innerHTML;
          const name = (currentTarget as HTMLDivElement).getAttribute("data-id");
          const detail = triggers.find(e => e.name == name);
          setUpMenu({
            content: (
              <TriggerMenu
                detail={detail}
                content={content}
                handleDrop={handleDrop}
                handleEnables={handleEnables}
              />
            ),
            event: e,
          });
        },
        style: { cursor: "context-menu" },
      },
      toolbar: {
        csvOptions: {
          delimiter: setting.export.csvDelimiter,
          includeHeaders: setting.export.includeHeader,
        },
      },
    } as GridSlotsComponentsProps;
  }, [
    handleDrop,
    handleEnables,
    setUpMenu,
    setting.export.csvDelimiter,
    setting.export.includeHeader,
    triggers,
  ]);

  const getRowId = useCallback(e => {
    return e["name"];
  }, []);
  const onRowDoubleClick = useCallback(row => {
    setRow(row.row);
  }, []);

  const [modalBox, setModalBox] = useState<HTMLDivElement | null>(null);
  useEffect(() => {
    if (modalBox) {
      const c = saveAndSetSize(modalBox, "redis-edit");
      return c;
    }
  }, [modalBox]);

  return (
    <Stack style={{ height: "100%" }}>
      <Stack direction="row" style={{ alignItems: "center" }}>
        <Button loading={loading} onClick={handleReload}>
          reload
        </Button>
        {triggers.length > 0 && (
          <Tooltip title="double click a row to see detail">
            <Typography color="info">detail?</Typography>
          </Tooltip>
        )}
      </Stack>

      <DataGrid
        showToolbar
        getRowId={getRowId}
        slotProps={slotProps}
        onColumnResize={onColumnResize}
        showCellVerticalBorder
        showColumnVerticalBorder
        rowHeight={setting.appearance.datagridRowHeight()}
        columnHeaderHeight={setting.appearance.datagridRowHeight()}
        columns={cols}
        rows={triggers}
        onRowDoubleClick={onRowDoubleClick}
        style={dataGridStyle}
      />

      {row && (
        <Modal
          open={!!row}
          onClose={modalHandleClose((e: EscDownEvent) => {
            e.preventDefault();
            setRow(null);
          })}
        >
          <Box
            sx={{ resize: "both", ...modalStyle }}
            width="80vw"
            height="80vh"
            ref={setModalBox}
          >
            <ModalHeader
              title={`Trigger Detail`}
              handleClose={() => {
                setRow(null);
              }}
            />
            <TriggerDetail
              reloadOP={async () => {
                return row;
              }}
              uuid={uuid}
              kind={kind}
            />
          </Box>
        </Modal>
      )}
    </Stack>
  );
}

function TableStructure(props: { table: string; uuid: string; kind: DataSourceKind }) {
  const { table, uuid, kind } = props;
  const [structure, setStructure] = useState("columns");
  const structures = [{ name: "columns" }];

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
    structures.push({ name: "statement" });
  }
  const supportIndex = [
    DataSourceKind.MariaDB,
    DataSourceKind.TiDB,
    DataSourceKind.Mysql,
    DataSourceKind.SqlServer,
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
    structures.push({ name: "indexes" });
  }
  const supportTrigger = [
    DataSourceKind.MariaDB,
    DataSourceKind.Mysql,
    DataSourceKind.Sqlite,
    DataSourceKind.LibSql,
    DataSourceKind.Oracle,
    DataSourceKind.SqlServer,
    DataSourceKind.Postgres,
  ].includes(kind);
  if (supportTrigger) {
    structures.push({ name: "triggers" });
  }

  return (
    <Stack style={{ height: "100%" }}>
      <Stack direction="row" style={{ justifyContent: "center" }}>
        {structures.map(e => (
          <Chip
            onClick={() => {
              setStructure(e.name);
            }}
            key={e.name}
            color={e.name == structure ? "success" : "default"}
            label={e.name}
            size="medium"
          />
        ))}
      </Stack>

      {structure == "columns" && (
        <ColumnsStructure table={table} kind={kind} uuid={uuid} />
      )}
      {structure == "indexes" && (
        <IndexesStructure table={table} kind={kind} uuid={uuid} />
      )}
      {structure == "triggers" && (
        <TriggersStructure table={table} kind={kind} uuid={uuid} />
      )}
      {structure == "statement" && (
        <StatementStructure table={table} kind={kind} uuid={uuid} />
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

  return (
    <Stack style={{ height: "100%" }}>
      <DefAndMetasView
        reloadOP={async () => {
          return createTableAndIndexSql(uuid, table).then(e => {
            return {
              def: formatCreateTableAndIndexSql(e),
              metas: [],
            } as DefAndMetas;
          });
        }}
        kind={kind}
        uuid={uuid}
      />
    </Stack>
  );
}

export { TableStructure };
